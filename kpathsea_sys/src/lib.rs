#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(missing_docs)]

// The bindgen surface exists only when `libkpathsea` was actually found:
// without a library there is no ABI to describe, the `extern "C"`
// declarations would be link-time landmines, and the generated layout
// self-tests assert the layouts of the platform the bindings were
// generated on. Unlinked builds (docs.rs, MacTeX, TeX-less CI) export
// nothing but [`LINKED`]; the high-level `kpathsea` crate provides the
// portable API on top.
#[cfg(all(kpathsea_linked, not(windows)))]
include!("bindings.rs");

// Windows linked builds bind TeX Live's `kpathsealibw64.dll` through
// hand-curated opaque-pointer declarations — the Linux-generated
// `bindings.rs` does not transfer to MSVC's LLP64 (see the module docs).
#[cfg(all(kpathsea_linked, windows))]
include!("bindings_windows.rs");

/// `true` when this build links the system `libkpathsea` (Unix: via
/// pkg-config or `KPATHSEA_LIB_DIR`; Windows: TeX Live's kpathsea DLL),
/// `false` when no library was found at build time (see `build.rs`). When
/// `false`, this crate exports nothing else — use the high-level
/// `kpathsea` crate, which falls back to a subprocess-`kpsewhich` backend
/// in that situation.
pub const LINKED: bool = cfg!(kpathsea_linked);
