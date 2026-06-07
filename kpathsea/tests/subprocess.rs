//! Subprocess-backend tests. These run on any host with a TeX distribution
//! on PATH (`kpsewhich` available), independent of whether `libkpathsea`
//! was linked — `Kpaths::new_subprocess()` forces the subprocess backend.

use kpathsea::Kpaths;
use std::process::Command;

fn subprocess_kpse() -> Kpaths {
  Kpaths::new_subprocess()
    .expect("You need a TeX toolchain with kpsewhich on PATH to run these tests.")
}

/// Ground truth: what the actual CLI says.
fn kpsewhich_cli(name: &str) -> Option<String> {
  let out = Command::new("kpsewhich").arg(name).output().ok()?;
  let s = String::from_utf8_lossy(&out.stdout);
  let line = s.lines().next()?.trim();
  if line.is_empty() {
    None
  } else {
    Some(line.to_string())
  }
}

#[test]
fn backend_is_reported() {
  let kpse = subprocess_kpse();
  assert!(!kpse.is_in_process());
}

#[test]
fn find_latex_via_subprocess() {
  let kpse = subprocess_kpse();
  let path = kpse
    .find_file("article.cls")
    .expect("subprocess backend failed to find article.cls");
  assert!(path.ends_with("article.cls"));
  // The ls-R cache (or direct call) must agree with the real resolver on
  // the basename; the full path may legitimately differ only if a TEXMF
  // tree shadows another, which the -dev/first-wins rules are designed to
  // prevent for release files like article.cls.
  if let Some(cli) = kpsewhich_cli("article.cls") {
    assert_eq!(path, cli, "cache result diverges from kpsewhich CLI");
  }
}

#[test]
fn finds_multiple_kinds_of_files_via_subprocess() {
  let kpse = subprocess_kpse();
  assert!(kpse.find_file("cmr10.tfm").unwrap().ends_with("cmr10.tfm"));
  assert!(kpse.find_file("plain.tex").unwrap().ends_with("plain.tex"));
  assert!(kpse.find_file("latex.ltx").unwrap().ends_with("latex.ltx"));
}

#[test]
fn find_first_returns_earliest_candidate() {
  let kpse = subprocess_kpse();
  let path = kpse
    .find_first(&["definitely-not-a-real-file.qqq", "article.cls"])
    .expect("find_first failed");
  assert!(path.ends_with("article.cls"));
}

#[test]
fn missing_file_is_none() {
  let kpse = subprocess_kpse();
  assert_eq!(kpse.find_first(&["definitely-not-a-real-file.qqq"]), None);
}

#[test]
fn format_typed_lookup_via_subprocess() {
  let kpse = subprocess_kpse();
  let path = kpse
    .find_file_with_format("article.cls", kpathsea::formats::TEX)
    .expect("format-typed subprocess lookup failed");
  assert!(path.ends_with("article.cls"));
}

#[test]
fn degenerate_names_do_not_panic() {
  let kpse = subprocess_kpse();
  // A bare extension with an empty stem; the in-process path used to
  // panic on these in guess_format_from_filename (debug overflow).
  let _ = kpse.find_file(".sty");
  let _ = kpse.find_file("");
}

#[test]
fn option_like_names_are_not_passed_to_kpsewhich() {
  // Candidate names beginning with `-` would be parsed as kpsewhich
  // options; the backend drops them instead.
  let kpse = subprocess_kpse();
  assert_eq!(kpse.find_file("--version"), None);
  assert_eq!(kpse.find_first(&["-progname=latex", "--help"]), None);
  // ... while remaining candidates are still resolved.
  let path = kpse
    .find_first(&["--help", "article.cls"])
    .expect("find_first failed");
  assert!(path.ends_with("article.cls"));
}

#[test]
fn bogus_kpsewhich_path_degrades_to_none() {
  // An explicit executable path is not validated up front; lookups just
  // come back empty (both the ls-R cache build and the direct call fail).
  let kpse = Kpaths::with_kpsewhich("/definitely/not/a/real/kpsewhich");
  assert!(!kpse.is_in_process());
  assert_eq!(kpse.find_file("article.cls"), None);
  // Second lookup exercises the direct-call memo (hit on a recorded miss).
  assert_eq!(kpse.find_file("article.cls"), None);
}

#[test]
fn instances_agree_through_the_shared_cache() {
  // The ls-R cache is process-global per kpsewhich executable; a second
  // instance resolves through the same shared cache and must agree.
  let (a, b) = (subprocess_kpse(), subprocess_kpse());
  assert_eq!(a.find_file("article.cls"), b.find_file("article.cls"));
  // Misses are memoized process-wide alongside the cache: a's spawn
  // outcome is visible to b, and all repeats agree on None.
  assert_eq!(a.find_file("zz-not-a-file.qqq"), None);
  assert_eq!(a.find_file("zz-not-a-file.qqq"), None);
  assert_eq!(b.find_file("zz-not-a-file.qqq"), None);
}

#[test]
fn lsr_cache_agrees_with_cli_on_shadowed_basenames() {
  // Regression witness for the ls-R cache tie-break (2026-06-07): TeX Live
  // ships TWO `fonttext.cfg`s (`tex/cslatex/base/` and `tex/latex/base/`).
  // In raw ls-R order cslatex comes first; kpathsea's path-spec order
  // resolves the latex/base one. A first-wins cache returned the csLaTeX
  // config, silently switching LaTeX's text encoding to IL2 during
  // latexml-oxide format-dump generation. Perl's last-wins overwrite gets
  // this right; the cache must agree with the real resolver.
  let kpse = subprocess_kpse();
  for name in ["fonttext.cfg", "fontmath.cfg", "hyphen.cfg", "article.cls"] {
    if let Some(cli) = kpsewhich_cli(name) {
      assert_eq!(
        kpse.find_file(name).as_deref(),
        Some(cli.as_str()),
        "cache diverges from kpsewhich CLI for shadowed basename {name}"
      );
    }
  }
}
