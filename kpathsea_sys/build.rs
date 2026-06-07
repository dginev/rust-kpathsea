use pkg_config::find_library;
use std::env;

/// Locate `libkpathsea` and emit link directives when it is available.
///
/// Probe order:
///  1. `KPATHSEA_LIB_DIR` env override — link against the given directory
///     unconditionally (for TeX trees that ship the library without a
///     `kpathsea.pc`, or cross-compilation setups).
///  2. `pkg-config kpathsea` — the standard route (Debian/Ubuntu
///     `libkpathsea-dev`, Homebrew `texlive`, vanilla TL source installs).
///  3. Neither found: build WITHOUT linking. This is graceful by design —
///     MacTeX/BasicTeX, for example, ship no `libkpathsea` at all (no
///     header, no dylib, no .pc), so there is nothing to link against.
///     The bindings still compile (types, constants, extern declarations);
///     the in-process functions simply must not be *referenced*. The
///     high-level `kpathsea` crate reads the `linked` metadata below and
///     falls back to its subprocess-`kpsewhich` backend automatically.
fn main() {
    println!("cargo:rerun-if-env-changed=KPATHSEA_LIB_DIR");
    println!("cargo:rustc-check-cfg=cfg(kpathsea_linked)");

    if let Ok(dir) = env::var("KPATHSEA_LIB_DIR") {
        println!("cargo:rustc-link-search=native={dir}");
        println!("cargo:rustc-link-lib=kpathsea");
        println!("cargo:rustc-cfg=kpathsea_linked");
        // `links = "kpathsea"` exports this to dependents as DEP_KPATHSEA_LINKED.
        println!("cargo:linked=1");
        return;
    }

    if find_library("kpathsea").is_ok() {
        // pkg-config has already emitted the link-search/link-lib directives.
        println!("cargo:rustc-cfg=kpathsea_linked");
        println!("cargo:linked=1");
        return;
    }

    println!(
        "cargo:warning=kpathsea_sys: libkpathsea not found (no pkg-config entry, \
         no KPATHSEA_LIB_DIR); building without linking. In-process kpathsea \
         calls are unavailable - the `kpathsea` crate will use its \
         subprocess-`kpsewhich` backend instead."
    );
    println!("cargo:linked=0");
}
