#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(missing_docs)]
// Stable rustc (>=1.98, Aug 2026) added `suspicious_runtime_symbol_definitions`, which under
// `cargo clippy -- -D warnings` reddens the bindgen `strlen` FFI declaration in `bindings.rs`
// (generated `-> c_ulong` where std expects `-> usize`). `bindings.rs` is `include!`d only for
// linked, non-Windows builds (see below), i.e. LP64/ILP32 targets where `c_ulong == usize`, so
// the signature is ABI-compatible and the lint masks nothing (rustc's own help: "allow this lint
// if the signature is compatible"); Windows LLP64 uses the hand-curated `bindings_windows.rs`.
// Drop this once the vendored bindings are regenerated blocklisting the libc `strlen` shim.
#![allow(suspicious_runtime_symbol_definitions)]

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
