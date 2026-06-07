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
//!  1. the `ls-R` cache — process-global, one per `kpsewhich` executable
//!     (Perl's `$kpse_cache` is likewise a process global), built lazily
//!     on first use: one `kpsewhich` invocation + reading each `ls-R`
//!     file in `$TEXMF`;
//!  2. a per-instance memo of earlier direct-call outcomes — TeX
//!     frontends re-probe the same absent names constantly, and each
//!     repeat would otherwise cost a process spawn;
//!  3. a direct `kpsewhich <candidates...>` subprocess call (also covers
//!     distributions without `ls-R` databases, e.g. MiKTeX — same comment
//!     as in the Perl original), whose outcome feeds the memo.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, LazyLock, Mutex, OnceLock, PoisonError};

/// The kpathsea path-list separator (Perl `$KPATHSEP`).
const KPATHSEP: char = if cfg!(windows) { ';' } else { ':' };

/// Basename → first-wins path, from the TeX tree's `ls-R` databases.
type LsRCache = HashMap<String, String>;

/// The `ls-R` caches, one per `kpsewhich` executable, shared by ALL
/// instances for the lifetime of the process — Perl's `$kpse_cache` is
/// likewise a process global. Without sharing, every instance pays the
/// build (~100ms) and holds its own copy: ~50MB on a full TeX Live,
/// multiplied by every live instance (gigabytes in a 100-thread smoke
/// test that constructed one instance per thread).
static CACHE_REGISTRY: LazyLock<Mutex<HashMap<PathBuf, Arc<LsRCache>>>> =
  LazyLock::new(|| Mutex::new(HashMap::new()));

/// Fetch (or build) the shared `ls-R` cache for this executable.
fn shared_kpse_cache(kpsewhich: &Path) -> Arc<LsRCache> {
  if let Some(cache) = CACHE_REGISTRY
    .lock()
    .unwrap_or_else(PoisonError::into_inner)
    .get(kpsewhich)
  {
    return Arc::clone(cache);
  }
  // Built OUTSIDE the registry lock — it spawns kpsewhich and reads the
  // ls-R files. A concurrent duplicate build is benign: first insert wins.
  let built = Arc::new(build_kpse_cache(kpsewhich));
  let mut registry = CACHE_REGISTRY
    .lock()
    .unwrap_or_else(PoisonError::into_inner);
  Arc::clone(registry.entry(kpsewhich.to_path_buf()).or_insert(built))
}

pub(crate) struct SubprocessKpse {
  kpsewhich: PathBuf,
  /// This instance's handle on the shared `ls-R` cache (see
  /// [`CACHE_REGISTRY`]); resolved lazily by the first lookup, reads are
  /// lock-free afterwards.
  cache: OnceLock<Arc<LsRCache>>,
  /// Outcomes of direct `kpsewhich` calls, hits and misses alike, keyed by
  /// the full argument vector (see [`Self::run_kpsewhich`]).
  memo: Mutex<HashMap<String, Option<String>>>,
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
      cache: OnceLock::new(),
      memo: Mutex::new(HashMap::new()),
    }
  }

  /// The shared `ls-R` cache for this instance's executable, resolved on
  /// first use (see [`CACHE_REGISTRY`]).
  fn cache(&self) -> &LsRCache {
    self
      .cache
      .get_or_init(|| shared_kpse_cache(&self.kpsewhich))
  }

  /// The first candidate with an `ls-R` cache entry, if any.
  fn cache_lookup(&self, candidates: &[&str]) -> Option<String> {
    let cache = self.cache();
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
  /// Outcomes — hits AND misses — are memoized per instance, keyed by the
  /// full argument vector: TeX frontends re-probe the same absent names
  /// constantly (and hosts without `ls-R` databases, e.g. MiKTeX, reach
  /// this path on every lookup), and each repeat would otherwise cost a
  /// fresh process spawn. This deliberately diverges from the Perl
  /// original, which re-spawns every time; the staleness it introduces —
  /// a file added to the TeX tree mid-process stays invisible to an
  /// instance that already missed it — matches the one-shot `ls-R`
  /// cache's existing semantics.
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
  fn memo(&self) -> std::sync::MutexGuard<'_, HashMap<String, Option<String>>> {
    self.memo.lock().unwrap_or_else(PoisonError::into_inner)
  }
}

/// Port of Perl `build_kpse_cache`: one `kpsewhich` call obtains both the
/// `$TEXMF` root list and the `tex` search path (the filter), then every
/// `ls-R` database under a `$TEXMF` root is read into a basename → path map.
///
/// Divergences from the Perl original, both deliberate:
///  * **first-wins** instead of Perl's last-wins overwrite: `$TEXMF` lists
///    trees in descending priority (TEXMFHOME before TEXMFDIST), and
///    `kpsewhich` resolves in that order; Perl's `$$kpse_cache{$_} = ...`
///    lets later (lower-priority) trees shadow earlier ones.
///  * `-dev` subdirectories are skipped unconditionally (Perl gates this on
///    its `latex-dev` debug flag). This is required for first-wins
///    correctness: `tex/latex-dev/base/article.cls` sorts before
///    `tex/latex/base/article.cls` in `ls-R`, and would otherwise shadow
///    the release file.
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

  for dir in texmf.split(',') {
    let dir = dir.trim().trim_start_matches("!!");
    let lsr_path = Path::new(dir).join("ls-R");
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
        // -dev releases shadow their release twins under first-wins (see
        // module docs); skip them like Perl does.
        skip = skip || d.contains("-dev/") || d.ends_with("-dev");
      } else if !skip {
        cache
          .entry(line.to_string())
          .or_insert_with(|| format!("{dir}/{subdir}/{line}"));
      }
    }
  }
  cache
}
