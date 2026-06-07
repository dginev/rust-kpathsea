use kpathsea::Kpaths;

#[test]
fn find_latex() {
  let kpse = Kpaths::new()
    .expect("You need a properly setup tex toolchain (texlive/MikTeX/...) and kpathsea headers, to use this wrapper.");
  if let Some(path) = kpse.find_file("article.cls") {
    assert!(path.ends_with("article.cls"), "Successfully found the full path of article.cls");
  } else {
    panic!("A tex toolchain was found, but the search failed to detect a class file.");
  }
}

#[test]
fn it_finds_multiple_kinds_of_files() {
  let kpse = Kpaths::new()
    .expect("You need a properly setup tex toolchain (texlive/MikTeX/...) and kpathsea headers, to use this wrapper.");

  assert!(kpse.find_file("cmr10.tfm").unwrap().ends_with("cmr10.tfm"));
  assert!(kpse.find_file("plain.tex").unwrap().ends_with("plain.tex"));
  assert!(kpse.find_file("latex.ltx").unwrap().ends_with("latex.ltx"));
  assert!(kpse.find_file("plain.mf").unwrap().ends_with("plain.mf"));
}

#[test]
fn degenerate_names_do_not_panic_default_backend() {
  let kpse = Kpaths::new()
    .expect("You need a properly setup tex toolchain (texlive/MikTeX/...) to use this wrapper.");
  // Bare-extension names (empty stem) used to underflow the alt-suffix
  // comparison in guess_format_from_filename and panic in debug builds.
  let _ = kpse.find_file(".sty");
  let _ = kpse.find_file(".bib");
  let _ = kpse.find_file("");
}
