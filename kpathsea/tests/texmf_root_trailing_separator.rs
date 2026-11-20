//! A TEXMF root with a trailing `/` resolves as without it, through both backends (dginev/latexml-oxide#950:
//! `TEXMFROOT=/usr/local/texlive/2024/` made texmf.cnf's `$TEXMFROOT/texmf-dist` kpathsea's `//` recursive search).
//!
//! One test in its own binary: it sets the process environment.

use kpathsea::{Kpaths, trimmed_texmf_roots};
use std::process::Command;

/// From the subprocess backend's ls-R cache, and `hyphen.cfg` (ambiguous in ls-R, evicted) from a spawned `kpsewhich`.
const NAMES: [&str; 4] = ["article.cls", "cmr10.tfm", "latex.ltx", "hyphen.cfg"];

/// `kpsewhich -var-value=<var>` as this process's environment resolves it.
fn var_value(var: &str) -> String {
  Command::new("kpsewhich")
    .arg(format!("-var-value={var}"))
    .output()
    .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
    .unwrap_or_default()
}

fn resolve(kpse: &Kpaths) -> Vec<Option<String>> {
  NAMES.iter().map(|name| kpse.find_file(name)).collect()
}

#[test]
#[ignore = "needs a TeX Live whose texmf.cnf derives TEXMFDIST from TEXMFROOT, not portable; run locally with `cargo test -- --ignored`"]
fn trailing_separator_on_texmf_root_resolves_as_without() {
  let root = var_value("TEXMFROOT");
  assert!(!root.is_empty(), "kpsewhich names no TEXMFROOT");
  let expected = resolve(&Kpaths::new_subprocess().expect("kpsewhich on PATH"));
  assert!(expected.iter().all(Option::is_some), "{expected:?}");
  assert!(trimmed_texmf_roots().is_empty());

  let slashed = format!("{root}/");
  // SAFETY: the only test of this binary; no other thread reads the environment.
  unsafe {
    std::env::set_var("TEXMFROOT", &slashed);
    std::env::remove_var("TEXMFDIST");
  }
  // Without the trim the subprocess backend's ls-R cache returns `…//texmf-dist/…` paths, its spawned `kpsewhich`
  // and the linked library resolve nothing. A bare executable name: the ls-R cache is shared per executable path.
  assert_eq!(resolve(&Kpaths::with_kpsewhich("kpsewhich")), expected);
  assert_eq!(
    trimmed_texmf_roots(),
    [("TEXMFROOT", slashed.clone(), root.clone())]
  );
  assert_eq!(
    std::env::var("TEXMFROOT").unwrap(),
    slashed,
    "the subprocess backend writes no environment"
  );
  let kpse = Kpaths::new().unwrap();
  assert_eq!(resolve(&kpse), expected);
  if kpse.is_in_process() && !cfg!(windows) {
    assert_eq!(std::env::var("TEXMFROOT").unwrap(), root);
  }
}
