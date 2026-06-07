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
//!  1. the `ls-R` cache (built lazily on first use, one `kpsewhich`
//!     invocation + reading each `ls-R` file in `$TEXMF`),
//!  2. a direct `kpsewhich <candidates...>` subprocess call (also covers
//!     distributions without `ls-R` databases, e.g. MiKTeX — same comment
//!     as in the Perl original).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

/// The kpathsea path-list separator (Perl `$KPATHSEP`).
const KPATHSEP: char = if cfg!(windows) { ';' } else { ':' };

pub(crate) struct SubprocessKpse {
  kpsewhich: PathBuf,
  /// `None` until the first lookup builds it (Perl: `$kpse_cache`).
  cache: Mutex<Option<HashMap<String, String>>>,
}

impl SubprocessKpse {
  /// Locate `kpsewhich`: the `KPSEWHICH` env var (resolved through PATH if
  /// it is a bare name, mirroring Perl's `which($ENV{...} || 'kpsewhich')`),
  /// then PATH.
  pub(crate) fn new() -> crate::Result<Self> {
    let name = std::env::var("KPSEWHICH").unwrap_or_else(|_| "kpsewhich".to_string());
    let kpsewhich = which::which(&name).map_err(|_| "Error finding kpsewhich executable")?;
    Ok(SubprocessKpse { kpsewhich, cache: Mutex::new(None) })
  }

  /// Use an explicit `kpsewhich` executable path, bypassing PATH lookup.
  pub(crate) fn with_kpsewhich(path: PathBuf) -> Self {
    SubprocessKpse { kpsewhich: path, cache: Mutex::new(None) }
  }

  /// Port of Perl `pathname_kpsewhich(@candidates)`: consult the `ls-R`
  /// cache first; on a full miss, issue ONE direct `kpsewhich` call with
  /// all candidates and take the first result line.
  pub(crate) fn find_first(&self, candidates: &[&str]) -> Option<String> {
    if candidates.is_empty() {
      return None;
    }
    {
      let mut guard = self.cache.lock().unwrap();
      let cache = guard.get_or_insert_with(|| build_kpse_cache(&self.kpsewhich));
      for candidate in candidates {
        if let Some(hit) = cache.get(*candidate) {
          return Some(hit.clone());
        }
      }
    }
    // "If we've failed to read the cache, try directly calling kpsewhich.
    //  For multiple calls, this is slower in general. But MiKTeX, eg.,
    //  doesn't use texmf ls-R files!" (Pathname.pm)
    self.run_kpsewhich(candidates)
  }

  /// Find with an explicit `kpsewhich --format=NAME`, when the caller's
  /// kpse format constant has a known CLI name; otherwise fall back to a
  /// plain lookup (kpsewhich guesses from the suffix, like `find_file`).
  pub(crate) fn find_with_format_name(
    &self,
    name: &str,
    format_name: Option<&str>,
  ) -> Option<String> {
    match format_name {
      Some(fmt) => self.run_kpsewhich(&[&format!("--format={fmt}"), name]),
      None => self.find_first(&[name]),
    }
  }

  /// One direct `kpsewhich` invocation. First stdout line wins; the exit
  /// status is deliberately ignored (kpsewhich exits non-zero when ANY
  /// candidate is missing — usually only one of them exists).
  fn run_kpsewhich(&self, args: &[&str]) -> Option<String> {
    let out = Command::new(&self.kpsewhich).args(args).output().ok()?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let first = stdout.lines().map(str::trim).find(|l| !l.is_empty())?;
    Some(first.to_string())
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
