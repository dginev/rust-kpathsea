#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(missing_docs)]

include!("bindings.rs");

/// `true` when this build links the system `libkpathsea`, `false` when the
/// library was not found at build time (see `build.rs`). When `false`, the
/// types and constants above are still usable, but calling any of the
/// `extern "C"` functions will fail at link time of the final artifact —
/// use the high-level `kpathsea` crate, which falls back to a
/// subprocess-`kpsewhich` backend in that situation.
pub const LINKED: bool = cfg!(kpathsea_linked);
