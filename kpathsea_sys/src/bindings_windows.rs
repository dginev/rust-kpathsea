// Hand-curated bindings for Windows builds, linking the kpathsea DLL that
// TeX Live's Windows distribution ships next to its binaries
// (`bin/windows/kpathsealibw64.dll` — TL builds its own tools against it).
//
// Deliberately OPAQUE: no struct layouts, no layout self-tests. The
// bindgen-generated `bindings.rs` was produced on Linux against glibc
// headers; its struct layouts (LP64 type sizes, glibc `stat`/`FILE`, and
// the `#ifdef WIN32` members of `kpathsea_instance` itself) do not hold
// under MSVC's LLP64 — its own layout self-tests fail there by the
// dozens. Only the opaque-pointer ABI is stable across that boundary:
// everything declared here takes and returns pointers and integers, and
// `kpathsea_instance` internals are never dereferenced on Windows (the
// high-level `kpathsea` crate guesses formats from a Rust-side suffix
// table instead of walking `format_info`).

use std::os::raw::{c_char, c_int, c_uint};

/// Opaque kpathsea instance — never dereferenced on Windows (see module
/// docs).
#[repr(C)]
pub struct kpathsea_instance {
  _opaque: [u8; 0],
}

pub type kpathsea = *mut kpathsea_instance;
pub type boolean = c_int;
pub type string = *mut c_char;
pub type const_string = *const c_char;
pub type kpse_file_format_type = c_uint;

unsafe extern "C" {
  pub fn kpathsea_new() -> kpathsea;
  pub fn kpathsea_set_program_name(kpse: kpathsea, argv0: const_string, progname: const_string);
  pub fn kpathsea_find_file(
    kpse: kpathsea,
    name: const_string,
    format: kpse_file_format_type,
    must_exist: boolean,
  ) -> string;
  pub fn kpathsea_finish(kpse: kpathsea);
}
