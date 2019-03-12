use kpathsea::Kpaths;

#[test]
fn find_latex() {
  let kpse = Kpaths::new();
  match kpse.find_file("article.cls") {
   Some(path) => assert!(path.ends_with("article.cls"), "Successfully found the full path of article.cls"),
   None => assert!(false, "article.cls wasn't detected on this system. Either your TeX/texlive installation or your kpathsea installation are missing/not visible.")
  };
}

#[test]
fn it_finds_multiple_kinds_of_files() {
  let kpse = Kpaths::new();

  assert!(kpse.find_file("plain.tex").unwrap().ends_with("plain.tex"));
  assert!(kpse.find_file("cmr10.tfm").unwrap().ends_with("cmr10.tfm"));
  assert!(kpse.find_file("latex.ltx").unwrap().ends_with("latex.ltx"));
  assert!(kpse.find_file("plain.mf").unwrap().ends_with("plain.mf"));
}
