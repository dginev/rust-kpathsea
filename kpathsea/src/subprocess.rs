//! Subprocess-`kpsewhich` backend.
//!
//! Used when `libkpathsea` is not linked (e.g. MacTeX/BasicTeX, which ship
//! no library at all — no header, no dylib, no `kpathsea.pc`), or when a
//! caller explicitly requests it via [`crate::Kpaths::new_subprocess`].
//!
//! This is a faithful port of the kpse machinery in Perl LaTeXML's
//! `LaTeXML/Util/Pathname.pm` (`pathname_kpsewhich` + `build_kpse_cache`),
//! the original large-scale consumer of this strategy. Perl LaTeXML never
//! links `libkpathsea`: it resolves through the host distribution's own
//! `kpsewhich` executable, fronted by a one-shot cache built from the TeX
//! tree's `ls-R` databases. Delegating to the host's resolver binary keeps
//! behavior in sync with the ambient TeX distribution by construction —
//! including distributions that reimplement kpathsea entirely (MiKTeX).
//!
//! Resolution order per lookup:
//!  1. the `ls-R` cache — built lazily on first use: one `kpsewhich`
//!     invocation + reading each `ls-R` file in `$TEXMF`;
//!  2. a memo of earlier direct-call outcomes — TeX frontends re-probe
//!     the same absent names constantly, and each repeat would otherwise
//!     cost a process spawn;
//!  3. a direct `kpsewhich <candidates...>` subprocess call (also covers
//!     distributions without `ls-R` databases, e.g. MiKTeX — same comment
//!     as in the Perl original), whose outcome feeds the memo.
//!
//! Both the cache and the memo live in one [`SharedKpse`] per `kpsewhich`
//! executable, process-global (Perl's `$kpse_cache` is likewise a process
//! global): instances are thin handles, and a name resolved — or proven
//! absent — by one instance is known to all of them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, OnceLock, PoisonError};

/// The kpathsea path-list separator (Perl `$KPATHSEP`).
const KPATHSEP: char = if cfg!(windows) { ';' } else { ':' };

/// On Windows, kpathsea lowercases DOS drive letters in every path it
/// *resolves* (`d:/texlive/...`), but `--expand-var`/`--show-path` output
/// keeps whatever case the installation configured (`D:/texlive/...`).
/// Cache entries are built from the latter; normalize them the way
/// kpathsea would, so a cache hit and a direct `kpsewhich` call return
/// byte-identical strings for the same file. (Divergence caught by the
/// Windows CI agreement tests.) No-op off Windows and on driveless paths.
fn normalize_drive_letter(path: &mut String) {
  let bytes = path.as_bytes();
  if cfg!(windows) && bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_uppercase() {
    let lower = (bytes[0] as char).to_ascii_lowercase().to_string();
    path.replace_range(..1, &lower);
  }
}

/// Basename → first-wins path, from the TeX tree's `ls-R` databases.
type LsRCache = HashMap<String, String>;

/// All mutable backend state for one `kpsewhich` executable, shared by
/// every instance for the lifetime of the process.
struct SharedKpse {
  /// The `ls-R` cache: immutable after build, so reads are lock-free.
  /// Without sharing, every instance would pay the build (~100ms) and
  /// hold its own copy — ~50MB on a full TeX Live, multiplied by every
  /// live instance (gigabytes in a 100-thread smoke test).
  lsr: LsRCache,
  /// Outcomes of direct `kpsewhich` calls, hits and misses alike, keyed
  /// by the full argument vector (see [`SubprocessKpse::run_kpsewhich`]).
  /// Locked only on the `lsr`-miss path, where the alternative is a
  /// ~150ms spawn.
  memo: Mutex<HashMap<String, Option<String>>>,
}

/// One [`SharedKpse`] per `kpsewhich` executable — Perl's `$kpse_cache`
/// is likewise a process global.
static KPSE_REGISTRY: LazyLock<Mutex<HashMap<PathBuf, Arc<SharedKpse>>>> =
  LazyLock::new(|| Mutex::new(HashMap::new()));

/// Fetch (or build) the shared state for this executable.
fn shared_kpse(kpsewhich: &Path) -> Arc<SharedKpse> {
  if let Some(shared) = KPSE_REGISTRY
    .lock()
    .unwrap_or_else(PoisonError::into_inner)
    .get(kpsewhich)
  {
    return Arc::clone(shared);
  }
  // Built OUTSIDE the registry lock — it spawns kpsewhich and reads the
  // ls-R files. A concurrent duplicate build is benign: first insert wins.
  let built = Arc::new(SharedKpse {
    lsr: build_kpse_cache(kpsewhich),
    memo: Mutex::new(HashMap::new()),
  });
  let mut registry = KPSE_REGISTRY.lock().unwrap_or_else(PoisonError::into_inner);
  Arc::clone(registry.entry(kpsewhich.to_path_buf()).or_insert(built))
}

pub(crate) struct SubprocessKpse {
  kpsewhich: PathBuf,
  /// This instance's handle on the shared state (see [`KPSE_REGISTRY`]);
  /// resolved lazily by the first lookup, after which cache reads are
  /// lock-free.
  shared: OnceLock<Arc<SharedKpse>>,
}

impl SubprocessKpse {
  /// Locate `kpsewhich`: the `KPSEWHICH` env var (resolved through PATH if
  /// it is a bare name, mirroring Perl's `which($ENV{...} || 'kpsewhich')`),
  /// then PATH.
  pub(crate) fn new() -> crate::Result<Self> {
    Ok(Self::with_kpsewhich(crate::kpsewhich_executable()?))
  }

  /// Use an explicit `kpsewhich` executable path, bypassing PATH lookup.
  pub(crate) fn with_kpsewhich(path: PathBuf) -> Self {
    SubprocessKpse {
      kpsewhich: path,
      shared: OnceLock::new(),
    }
  }

  /// The shared state for this instance's executable, resolved on first
  /// use (see [`KPSE_REGISTRY`]).
  fn shared(&self) -> &SharedKpse {
    self.shared.get_or_init(|| shared_kpse(&self.kpsewhich))
  }

  /// The first candidate with an `ls-R` cache entry, if any.
  fn cache_lookup(&self, candidates: &[&str]) -> Option<String> {
    let cache = &self.shared().lsr;
    candidates.iter().find_map(|c| cache.get(*c).cloned())
  }

  /// Port of Perl `pathname_kpsewhich(@candidates)`: consult the `ls-R`
  /// cache first; on a full miss, issue ONE direct `kpsewhich` call with
  /// all candidates and take the first result line.
  pub(crate) fn find_first(&self, candidates: &[&str]) -> Option<String> {
    if let Some(hit) = self.cache_lookup(candidates) {
      return Some(hit);
    }
    // "If we've failed to read the cache, try directly calling kpsewhich.
    //  For multiple calls, this is slower in general. But MiKTeX, eg.,
    //  doesn't use texmf ls-R files!" (Pathname.pm)
    self.run_kpsewhich(&[], candidates)
  }

  /// Format-typed lookup. The `ls-R` cache is consulted first, exactly like
  /// [`Self::find_first`] — an exact-basename hit is what `kpsewhich` would
  /// return for any format that can match the name's suffix. The format only
  /// shapes the fallback: on a cache miss it is passed as
  /// `kpsewhich --format=NAME` (enabling kpsewhich's suffix auto-completion
  /// for that format); with no known CLI name the fallback is a plain lookup
  /// (kpsewhich then guesses from the suffix, like `find_file`).
  pub(crate) fn find_with_format_name(
    &self,
    name: &str,
    format_name: Option<&str>,
  ) -> Option<String> {
    if let Some(hit) = self.cache_lookup(&[name]) {
      return Some(hit);
    }
    match format_name {
      Some(fmt) => self.run_kpsewhich(&[&format!("--format={fmt}")], &[name]),
      None => self.run_kpsewhich(&[], &[name]),
    }
  }

  /// One direct `kpsewhich` invocation: `flags` first, then candidate
  /// `names`. Names beginning with `-` are dropped rather than relying on
  /// `--` end-of-options support across kpsewhich reimplementations —
  /// kpsewhich would otherwise parse them as options. First stdout line
  /// wins; the exit status is deliberately ignored (kpsewhich exits
  /// non-zero when ANY candidate is missing — usually only one of them
  /// exists).
  ///
  /// Outcomes — hits AND misses — are memoized process-wide per
  /// executable ([`SharedKpse::memo`]), keyed by the full argument
  /// vector: TeX frontends re-probe the same absent names constantly
  /// (and hosts without `ls-R` databases, e.g. MiKTeX, reach this path
  /// on every lookup), and each repeat — including from another thread's
  /// instance — would otherwise cost a fresh process spawn. This
  /// deliberately diverges from the Perl original, which re-spawns every
  /// time; the staleness it introduces — a file added to the TeX tree
  /// mid-process stays invisible once a lookup missed it — matches the
  /// one-shot `ls-R` cache's existing semantics.
  fn run_kpsewhich(&self, flags: &[&str], names: &[&str]) -> Option<String> {
    let names: Vec<&str> = names
      .iter()
      .copied()
      .filter(|n| !n.starts_with('-'))
      .collect();
    if names.is_empty() {
      return None;
    }
    let key = flags
      .iter()
      .chain(names.iter())
      .copied()
      .collect::<Vec<_>>()
      .join("\u{1f}");
    if let Some(outcome) = self.memo().get(&key) {
      return outcome.clone();
    }
    // The memo lock is NOT held during the spawn: concurrent lookups of
    // the same unmemoized query may each spawn once (benign — identical
    // results, last insert wins).
    let result = Command::new(&self.kpsewhich)
      .args(flags)
      .args(&names)
      .output()
      .ok()
      .and_then(|out| {
        let stdout = String::from_utf8_lossy(&out.stdout);
        stdout
          .lines()
          .map(str::trim)
          .find(|l| !l.is_empty())
          .map(str::to_string)
      });
    self.memo().insert(key, result.clone());
    result
  }

  /// The direct-call memo, tolerating lock poisoning (no code panics while
  /// holding it, but a poisoned memo would only ever repeat spawns).
  fn memo(&self) -> MutexGuard<'_, HashMap<String, Option<String>>> {
    self
      .shared()
      .memo
      .lock()
      .unwrap_or_else(PoisonError::into_inner)
  }
}

/// Port of Perl `build_kpse_cache`: one `kpsewhich` call obtains both the
/// `$TEXMF` root list and the `tex` search path (the filter), then every
/// `ls-R` database under a `$TEXMF` root is read into a basename → path map.
///
/// **Ambiguous basenames are evicted** — a name listed under more than one
/// subdirectory is removed from the cache, so its lookups fall through to
/// a direct `kpsewhich` call (memoized), which is ground truth by
/// construction. This deliberately diverges from Perl's unconditional
/// `$$kpse_cache{$_} = ...` overwrite, because NO single-pass tie-break
/// can reproduce kpathsea's path-spec ranking from raw `ls-R` order.
/// Two live witnesses (latexml-oxide format-dump validation, 2026-06-07):
///
///  * `fonttext.cfg`: TL ships `tex/cslatex/base/` (ISO Latin 2) and
///    `tex/latex/base/`. kpathsea resolves the latter; first-wins on raw
///    `ls-R` order picks csLaTeX's and silently turns LaTeX's text
///    encoding Czech.
///  * `hyphen.cfg`: TL ships `tex/generic/babel/` and
///    `tex/lambda/antomega/`. kpathsea resolves babel's; LAST-wins (Perl's
///    tie-break) picks antomega's.
///
/// `-dev` pre-release subdirectories are skipped before ambiguity
/// detection (Perl gates the same skip on its `latex-dev` debug flag) —
/// otherwise every kernel file would be "ambiguous" against its
/// `latex-dev` twin and the eviction would gut the cache; dev-only files
/// resolve through the direct call instead.
///
/// On any failure the cache is simply left empty ("At least we've tried") —
/// every lookup then falls through to a direct `kpsewhich` call.
fn build_kpse_cache(kpsewhich: &Path) -> HashMap<String, String> {
  let mut cache = HashMap::new();
  // Get 2 bits of data from kpsewhich (with 1 call!)
  // texmf: ALL the directories used for any purposes, including docs, fonts, etc
  // texpaths: the directories which contain the TeX related files we're
  //   interested in (but they're typically below where the ls-R indexes are!)
  let Ok(out) = Command::new(kpsewhich)
    .args(["--expand-var", "$TEXMF", "--show-path", "tex"])
    .output()
  else {
    return cache;
  };
  let stdout = String::from_utf8_lossy(&out.stdout);
  let mut lines = stdout.lines();
  let texmf = lines.next().unwrap_or("").trim().to_string();
  let texpaths = lines.next().unwrap_or("").trim().to_string();

  // The filter set: existing directories on the `tex` search path. A single
  // trailing `/` is preserved (Perl: s|//+$|/|) — it both marks the end of
  // the directory name for the substring filter below (`.../tex/` must not
  // match `.../texmf-dist`) and collapses kpathsea's `//` recursion marker.
  let mut filters: Vec<String> = Vec::new();
  for path in texpaths.split(KPATHSEP) {
    let mut path = path.trim().trim_start_matches("!!").to_string();
    while path.ends_with("//") {
      path.pop();
    }
    normalize_drive_letter(&mut path);
    if !path.is_empty() && Path::new(&path).is_dir() {
      filters.push(path);
    }
  }
  if filters.is_empty() {
    // "Really shouldn't end up empty" — but if it is, Perl's regex guard
    // skips every subdirectory; an empty cache expresses the same thing.
    return cache;
  }

  // The $TEXMF root list: strip quoting and the outer brace expansion,
  // then split on commas.
  let mut texmf = texmf
    .trim()
    .trim_matches(|c| c == '"' || c == '\'')
    .trim_start_matches('\\')
    .to_string();
  if texmf.starts_with('{') && texmf.ends_with('}') {
    texmf = texmf[1..texmf.len() - 1].to_string();
  }
  texmf = texmf.replace("{}", "");

  // Names seen under more than one subdirectory; evicted below (see
  // module docs — no `ls-R`-order tie-break matches kpathsea's ranking).
  let mut ambiguous: std::collections::HashSet<String> = std::collections::HashSet::new();
  for dir in texmf.split(',') {
    let mut dir = dir.trim().trim_start_matches("!!").to_string();
    normalize_drive_letter(&mut dir);
    let lsr_path = Path::new(&dir).join("ls-R");
    // Presumably if no ls-R, we can ignore the directory?
    let Ok(lsr) = std::fs::read_to_string(&lsr_path) else {
      continue;
    };
    let mut subdir = String::new();
    let mut skip = true; // whether to skip entries in the current subdirectory
    for line in lsr.lines() {
      if line.is_empty() || line.starts_with('%') {
        continue;
      }
      if let Some(sub) = line.strip_suffix(':') {
        subdir = sub.strip_prefix("./").unwrap_or(sub).to_string();
        let d = format!("{dir}/{subdir}");
        skip = !filters.iter().any(|f| d.contains(f.as_str()));
        // Skip -dev pre-release trees BEFORE ambiguity detection (see
        // module docs); their files resolve via the direct fallback.
        skip = skip || d.contains("-dev/") || d.ends_with("-dev");
      } else if !skip {
        match cache.entry(line.to_string()) {
          std::collections::hash_map::Entry::Vacant(slot) => {
            slot.insert(format!("{dir}/{subdir}/{line}"));
          }
          std::collections::hash_map::Entry::Occupied(_) => {
            ambiguous.insert(line.to_string());
          }
        }
      }
    }
  }
  for name in &ambiguous {
    cache.remove(name);
  }
  cache
}

#[cfg(all(test, windows))]
mod tests {
  #[test]
  fn drive_letters_normalize_to_lowercase() {
    let mut p = String::from("D:/texlive/2026/texmf-dist");
    super::normalize_drive_letter(&mut p);
    assert_eq!(p, "d:/texlive/2026/texmf-dist");
    // Driveless paths are untouched.
    let mut rel = String::from("texmf-dist");
    super::normalize_drive_letter(&mut rel);
    assert_eq!(rel, "texmf-dist");
  }
}
