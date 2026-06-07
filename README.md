[![CI](https://github.com/dginev/rust-kpathsea/actions/workflows/ci.yml/badge.svg)](https://github.com/dginev/rust-kpathsea/actions/workflows/ci.yml)
[![API Documentation](https://img.shields.io/badge/docs-API-blue.svg)](https://docs.rs/crate/kpathsea)
[![License](http://img.shields.io/badge/license-MIT-blue.svg)](https://raw.githubusercontent.com/dginev/rust-kpathsea/master/LICENSE)
[![crates.io](https://img.shields.io/crates/v/kpathsea.svg)](https://crates.io/crates/kpathsea)

A Rust interface to the [kpathsea library](https://ctan.org/pkg/kpathsea),
the TeX ecosystem's file-search component. Portable across Linux, macOS,
and Windows; every supported configuration is verified in CI.

**Note:** a `Kpaths` instance is not `Sync` (see #2). Construction from
multiple threads is safe — it is serialized internally.

### Backends

Two backends, selected automatically at build time;
`Kpaths::is_in_process()` reports the selection.

* **in-process** — FFI into `libkpathsea`; microsecond lookups. Linked
  via `pkg-config` or `KPATHSEA_LIB_DIR`; on Windows, TeX Live's own
  `kpathsealibw64.dll` is located next to `kpsewhich.exe` and linked
  directly — no headers or import library required.
* **subprocess** — delegates to the host's `kpsewhich`, fronted by a
  process-global cache of the `ls-R` databases, following Perl LaTeXML's
  resolution strategy. Remains correct on distributions that reimplement
  kpathsea.

| Platform | TeX distribution | Backend |
|---|---|---|
| Linux | TeX Live with `libkpathsea-dev` | in-process |
| macOS | Homebrew `texlive` | in-process |
| macOS | MacTeX / BasicTeX | subprocess |
| Windows | TeX Live | in-process |
| Windows | MiKTeX | subprocess |
| any | `kpsewhich` on PATH, no library | subprocess |

The build fails only when neither backend is possible — no library and no
`kpsewhich` — with the remedies stated. `kpathsea_sys` exports its FFI
bindings only in linked builds; the high-level API is identical in either
configuration.

Environment variables:

| Variable | Effect |
|---|---|
| `KPSEWHICH` | the `kpsewhich` executable both backends anchor on |
| `KPATHSEA_LIB_DIR` | link `libkpathsea` from this directory |
| `KPATHSEA_NO_LINK=1` | force the subprocess backend at build time |
| `KPATHSEA_SKIP_TOOLCHAIN_CHECK=1` | permit building with no TeX installed (docs.rs and cross-compilation skip the check automatically) |

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
