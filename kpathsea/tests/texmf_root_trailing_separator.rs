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
  // and the linked library resolve nothing. The same executable as above: its shared ls-R cache is keyed by the roots.
  assert_eq!(resolve(&Kpaths::new_subprocess().unwrap()), expected);
  let [trimmed] = trimmed_texmf_roots() else {
    panic!("{:?}", trimmed_texmf_roots());
  };
  assert_eq!(
    (trimmed.var, trimmed.given.as_str(), trimmed.used.as_str()),
    ("TEXMFROOT", slashed.as_str(), root.as_str())
  );
  assert_eq!(
    std::env::var("TEXMFROOT").unwrap(),
    slashed,
    "the subprocess backend writes no environment"
  );
  let kpse = Kpaths::new().unwrap();
  assert_eq!(resolve(&kpse), expected);
  if kpse.is_in_process() {
    assert_eq!(std::env::var("TEXMFROOT").unwrap(), root);
  }
}
