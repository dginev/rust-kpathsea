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
  if line.is_empty() { None } else { Some(line.to_string()) }
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
