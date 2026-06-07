use kpathsea::Kpaths;

#[test]
fn find_latex() {
  let kpse = Kpaths::new()
    .expect("You need a properly setup tex toolchain (texlive/MikTeX/...) and kpathsea headers, to use this wrapper.");
  if let Some(path) = kpse.find_file("article.cls") {
    assert!(
      path.ends_with("article.cls"),
      "Successfully found the full path of article.cls"
    );
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
  // Interior NUL bytes used to panic in CString::new.
  let _ = kpse.find_file("arti\0cle.cls");
}

#[test]
fn concurrent_construction_is_safe() {
  // Regression: libkpathsea's kpse_set_program_name mutates process-global
  // state; without the crate's construction lock, concurrent Kpaths::new()
  // calls interleave its path buffers and crash the process outright
  // ("Can't get directory of program name", with garbled paths).
  let handles: Vec<_> = (0..8)
    .map(|_| std::thread::spawn(|| Kpaths::new().map(|kpse| kpse.is_in_process())))
    .collect();
  for handle in handles {
    handle
      .join()
      .expect("construction thread panicked")
      .expect("Kpaths::new failed in a thread");
  }
}
