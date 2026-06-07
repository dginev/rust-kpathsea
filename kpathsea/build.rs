use std::env;

/// `kpathsea_sys` declares `links = "kpathsea"` and publishes
/// `cargo:linked={0,1}` from its build script, which Cargo exposes to us as
/// `DEP_KPATHSEA_LINKED`. Mirror it as a `cfg` so the library can select
/// the in-process backend at compile time, and fall back to the
/// subprocess-`kpsewhich` backend when the C library was not found.
fn main() {
  println!("cargo:rustc-check-cfg=cfg(kpathsea_linked)");
  if env::var("DEP_KPATHSEA_LINKED").as_deref() == Ok("1") {
    println!("cargo:rustc-cfg=kpathsea_linked");
  }
}
