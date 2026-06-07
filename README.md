[![Build Status](https://secure.travis-ci.org/dginev/rust-kpathsea.png?branch=master)](http://travis-ci.org/dginev/rust-kpathsea)
[![API Documentation](https://img.shields.io/badge/docs-API-blue.svg)](https://docs.rs/crate/kpathsea)
[![License](http://img.shields.io/badge/license-MIT-blue.svg)](https://raw.githubusercontent.com/dginev/rust-kpathsea/master/LICENSE)
[![crates.io](https://img.shields.io/crates/v/kpathsea.svg)](https://crates.io/crates/kpathsea)

Rust interface and wrapper for the [kpathsea library](https://ctan.org/pkg/kpathsea)

**Note:** Currently there are no safety guarantees for the in-process backend, and a `Kpaths` instance is not `Sync` (see #2). Constructing instances from multiple threads is safe — construction is serialized internally.

### Backends and portability

Two backends, selected automatically at build time:

* **in-process** — FFI into the system `libkpathsea` (microseconds per
  lookup). Used when the library is found via `pkg-config kpathsea` or the
  `KPATHSEA_LIB_DIR` env override (Debian/Ubuntu: `libkpathsea-dev`;
  macOS: `brew install texlive`).
* **subprocess** — shells out to the host TeX distribution's own
  `kpsewhich`, fronted by a one-shot cache of the `ls-R` databases. Used
  when `libkpathsea` is absent at build time — e.g. **MacTeX/BasicTeX
  ship no library at all** — or on request via `Kpaths::new_subprocess()`
  / `Kpaths::with_kpsewhich(path)`. This mirrors how Perl LaTeXML resolves
  TeX files, and stays correct on distributions that reimplement kpathsea
  (MiKTeX).

The `KPSEWHICH` env var overrides the `kpsewhich` executable both backends
anchor on (the subprocess backend invokes it; the in-process backend uses
its location to find the right TeX distribution).

The build never fails for lack of `libkpathsea`; `Kpaths::is_in_process()`
reports which backend you got.

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
