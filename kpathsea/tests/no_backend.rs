//! With neither a linked `libkpathsea` nor a `kpsewhich` to spawn, construction
//! must fail explicitly — never hand back a handle that silently resolves
//! nothing. Only meaningful in an unlinked build; skipped entirely otherwise.
#![cfg(not(kpathsea_linked))]

use kpathsea::Kpaths;

#[test]
fn construction_fails_when_no_backend_exists() {
  // SAFETY: the only test in this integration binary, so nothing else is
  // mutating the environment concurrently.
  unsafe { std::env::set_var("KPSEWHICH", "/nonexistent/definitely-not-kpsewhich") };

  // `Kpaths` is not `Debug`, so match rather than `expect_err`.
  let err = match Kpaths::new() {
    Ok(_) => panic!("must not succeed with no library and no kpsewhich"),
    Err(err) => err,
  };
  assert!(
    err.contains("kpsewhich") && err.contains("libkpathsea"),
    "the error must name both missing backends, got: {err}"
  );
}
