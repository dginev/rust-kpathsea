use kpathsea::Kpaths;

#[test]
fn find_latex() {
  let kpse = Kpaths::new();
  match kpse.find_file("article.cls") {
   Some(path) => assert!(path.ends_with("article.cls"), "Successfully found the full path of article.cls"),
   None => assert!(false, "article.cls wasn't detected on this system. Either your TeX/texlive installation or your kpathsea installation are missing/not visible.")
  };
}