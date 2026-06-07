[![CI](https://github.com/dginev/rust-kpathsea/actions/workflows/ci.yml/badge.svg)](https://github.com/dginev/rust-kpathsea/actions/workflows/ci.yml)
[![API Documentation](https://img.shields.io/badge/docs-API-blue.svg)](https://docs.rs/crate/kpathsea)
[![License](http://img.shields.io/badge/license-MIT-blue.svg)](https://raw.githubusercontent.com/dginev/rust-kpathsea/master/LICENSE)
[![crates.io](https://img.shields.io/crates/v/kpathsea.svg)](https://crates.io/crates/kpathsea)

Rust interface and wrapper for the [kpathsea library](https://ctan.org/pkg/kpathsea)

**Note:** a `Kpaths` instance is not `Sync` (see #2). Constructing
instances from multiple threads is safe — construction is serialized
internally.

### Backends

Selected automatically at build time; `Kpaths::is_in_process()` reports
the choice.

* **in-process** — FFI into `libkpathsea`; microsecond lookups. Linked
  when found via `pkg-config` or `KPATHSEA_LIB_DIR` (Debian/Ubuntu:
  `libkpathsea-dev`; macOS: `brew install texlive`). On **Windows**, TeX
  Live's own `kpathsealibw64.dll` (next to `kpsewhich.exe`) is linked
  automatically — no headers or import library needed.
* **subprocess** — delegates to the host's `kpsewhich`, fronted by a
  process-global `ls-R` cache (how Perl LaTeXML resolves files). Used
  when no library is available — MacTeX/BasicTeX ship none — or via
  `Kpaths::new_subprocess()` / `Kpaths::with_kpsewhich(path)`. Stays
  correct on distributions that reimplement kpathsea (MiKTeX).

The build fails only when *neither* backend is possible — no library, no
`kpsewhich` — with the remedies spelled out. (`kpathsea_sys` exports its
FFI bindings only in linked builds; the high-level API is identical
either way.)

Environment variables:

| Variable | Effect |
|---|---|
| `KPSEWHICH` | the `kpsewhich` executable both backends anchor on |
| `KPATHSEA_LIB_DIR` | link `libkpathsea` from this directory |
| `KPATHSEA_NO_LINK=1` | force the subprocess backend at build time |
| `KPATHSEA_SKIP_TOOLCHAIN_CHECK=1` | allow building with no TeX at all (docs.rs and cross-compiles skip the check automatically) |

### Example

```rust
  let kpse = Kpaths::new()
    .expect("You need a properly setup tex toolchain (texlive/MikTeX/...) and kpathsea headers, to use this wrapper.");
  if let Some(path) = kpse.find_file("article.cls") {
    assert!(path.ends_with("article.cls"), "Successfully found the full path of article.cls");
  } else {
    panic!("A tex toolchain was found, but the search failed to detect a class file.");
  }
```
