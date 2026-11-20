//! TEXMF tree roots without a trailing directory separator.
//!
//! texmf.cnf joins the roots with `/` into the search paths (`TEXMFDIST = $TEXMFROOT/texmf-dist`,
//! `TEXMF = {…,!!$TEXMFDIST}`, `TEXINPUTS = …$TEXMF/tex/…//`). A root set in the environment with a trailing `/`
//! (`TEXMFROOT=/usr/local/texlive/2024/`) joins into `/usr/local/texlive/2024//texmf-dist`, which kpathsea reads as
//! its `//` recursive search (elt-dirs.c): every lookup walks the tree on disk, and the `!!` ls-R-only elements no
//! longer prefix-match their `ls-R` directory (db.c), so nothing resolves. Real `kpsewhich` and pdflatex fail the same
//! way (pdflatex cannot find its format). [`crate::Kpaths::new`] hands both backends the roots without it
//! (dginev/latexml-oxide#950).

use std::sync::OnceLock;

/// The TEXMF tree roots that texmf.cnf joins with `/` into the search paths. Search-path specs (`TEXINPUTS`,
/// `TEXMFCNF`, `TEXMF`, …) are not roots: their `//` is kpathsea's recursive search, written on purpose.
pub const TEXMF_ROOT_VARS: [&str; 9] = [
  "TEXMFROOT",
  "TEXMFDIST",
  "TEXMFMAIN",
  "TEXMFLOCAL",
  "TEXMFSYSVAR",
  "TEXMFSYSCONFIG",
  "TEXMFHOME",
  "TEXMFVAR",
  "TEXMFCONFIG",
];

/// A root variable [`crate::Kpaths::new`] trimmed: the variable, its value as given, and the value used.
pub type TrimmedRoot = (&'static str, String, String);

/// A TEXMF root `value` without its trailing directory separators, or `None` when there is none to trim.
///
/// A value that is a path list or a brace expansion (holding the path-list separator, `,` or `{`) is a search spec
/// and stays as written; a file-system or drive root (`/`, `C:/`, `C:\`) keeps one separator; only the end is trimmed
/// (a `!!` prefix or a `//` inside the path stays). The separators are `/`, and on Windows also `\`.
pub fn trim_texmf_root(value: &str) -> Option<String> {
  trim_root_separators(value, cfg!(windows))
}

/// [`trim_texmf_root`] for the separators of Windows (`/`, `\`, list separator `;`) or of Unix (`/`, `:`).
fn trim_root_separators(value: &str, windows: bool) -> Option<String> {
  let (separators, list_separator): (&[char], char) = if windows {
    (&['/', '\\'], ';')
  } else {
    (&['/'], ':')
  };
  if value.contains([list_separator, ',', '{']) {
    return None;
  }
  let stem = value.trim_end_matches(separators);
  let is_drive = windows && stem.len() == 2 && stem.ends_with(':');
  // A root (`/`, `C:/`) keeps one separator.
  let keep = if stem.is_empty() || is_drive {
    stem.len() + 1
  } else {
    stem.len()
  };
  (keep < value.len()).then(|| value[..keep].to_string())
}

/// The root variables set in the environment now whose value has a trailing separator to trim. A value that is not
/// valid Unicode is left as written.
pub(crate) fn texmf_root_trims() -> Vec<TrimmedRoot> {
  TEXMF_ROOT_VARS
    .iter()
    .filter_map(|&var| {
      let given = std::env::var(var).ok()?;
      let used = trim_texmf_root(&given)?;
      Some((var, given, used))
    })
    .collect()
}

/// The first non-empty trim of this process (see [`trimmed_texmf_roots`]).
static TRIMMED_TEXMF_ROOTS: OnceLock<Vec<TrimmedRoot>> = OnceLock::new();

/// Remember `trims` for [`trimmed_texmf_roots`], unless empty or a construction already did.
pub(crate) fn record_trims(trims: &[TrimmedRoot]) {
  if !trims.is_empty() {
    let _ = TRIMMED_TEXMF_ROOTS.set(trims.to_vec());
  }
}

/// The root variables (see [`TEXMF_ROOT_VARS`]) whose trailing separator a [`crate::Kpaths`] construction of this
/// process trimmed, each as (variable, value as given, value used); empty when none had one (the usual case). For a
/// caller's log, next to [`crate::appended_texmfcnf`].
///
/// The linked library is handed the trimmed values in the environment (a later construction then finds nothing to
/// trim), the subprocess backend passes them to each `kpsewhich` it spawns.
pub fn trimmed_texmf_roots() -> &'static [TrimmedRoot] {
  TRIMMED_TEXMF_ROOTS.get().map_or(&[], Vec::as_slice)
}

#[cfg(test)]
mod tests {
  use super::*;

  /// A root loses its trailing separators, so `$TEXMFROOT/texmf-dist` never becomes kpathsea's `//` recursive search;
  /// roots keep theirs, and a spec's deliberate `//` is left as written. Pure strings: portable.
  #[test]
  fn trims_roots_not_specs() {
    for windows in [false, true] {
      let trim = |value: &str| trim_root_separators(value, windows);
      assert_eq!(
        trim("/usr/local/texlive/2024/").as_deref(),
        Some("/usr/local/texlive/2024")
      );
      assert_eq!(
        trim("/usr/local/texlive/2024//").as_deref(),
        Some("/usr/local/texlive/2024")
      );
      assert_eq!(trim("~/texmf/").as_deref(), Some("~/texmf"));
      assert_eq!(
        trim("!!/usr/local/texlive/2024/").as_deref(),
        Some("!!/usr/local/texlive/2024")
      );
      assert_eq!(trim("/usr/local/texlive/2024"), None, "nothing to trim");
      assert_eq!(trim("/usr/local//texlive/2024"), None, "only the end");
      assert_eq!(trim(""), None);
      assert_eq!(trim("/"), None, "the file-system root keeps its separator");
      assert_eq!(trim("//").as_deref(), Some("/"));
      // A path list or a brace expansion is a search spec: its `//` is kpathsea's recursive search.
      assert_eq!(trim("{/a/,/b/}"), None);
      assert_eq!(trim("/a/,/b/"), None);
      let list = if windows { ".//;C:/x//" } else { ".//:/x//" };
      assert_eq!(trim(list), None);
    }
    let windows = |value: &str| trim_root_separators(value, true);
    assert_eq!(
      windows(r"C:\texlive\2024\").as_deref(),
      Some(r"C:\texlive\2024")
    );
    assert_eq!(
      windows(r"C:/texlive/2024/\").as_deref(),
      Some("C:/texlive/2024")
    );
    assert_eq!(
      windows(r"\\server\share\").as_deref(),
      Some(r"\\server\share")
    );
    assert_eq!(windows("C:/"), None, "a drive root keeps its separator");
    assert_eq!(windows(r"C:\"), None);
    assert_eq!(windows(r"C:\\").as_deref(), Some(r"C:\"));
    assert_eq!(windows("C:"), None);
    // On Unix `\` is a file-name character, and `:` separates a path list.
    assert_eq!(trim_root_separators(r"/x\", false), None);
    assert_eq!(trim_root_separators("C:/", false), None);
    // Only roots are trimmed: the search-path specs are not among them.
    for spec in ["TEXINPUTS", "TEXMFCNF", "TEXMF", "TEXMFDBS", "TFMFONTS"] {
      assert!(
        !TEXMF_ROOT_VARS.contains(&spec),
        "{spec} is a search-path spec"
      );
    }
  }

  /// The public function uses the host's separators.
  #[test]
  fn trim_texmf_root_uses_the_host_separators() {
    assert_eq!(trim_texmf_root("/a/b/").as_deref(), Some("/a/b"));
    let backslash = trim_texmf_root(r"C:\a\");
    assert_eq!(backslash.as_deref(), cfg!(windows).then_some(r"C:\a"));
  }
}
