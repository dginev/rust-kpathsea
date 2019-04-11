[![Build Status](https://secure.travis-ci.org/dginev/rust-kpathsea.png?branch=master)](http://travis-ci.org/dginev/rust-kpathsea)
[![API Documentation](https://img.shields.io/badge/docs-API-blue.svg)](https://docs.rs/crate/kpathsea)
[![License](http://img.shields.io/badge/license-MIT-blue.svg)](https://raw.githubusercontent.com/dginev/rust-kpathsea/master/LICENSE)
[![crates.io](https://img.shields.io/crates/v/kpathsea.svg)](https://crates.io/crates/kpathsea)

Rust interface and wrapper for the [kpathsea library](https://ctan.org/pkg/kpathsea)

**Note:** Currently there are no safety guarantees and the wrapper is not thread-safe (see #2)

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
