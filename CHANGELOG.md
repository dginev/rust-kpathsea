# Change Log

## [0.2.5] (next target)

API additions:

* Add `Kpaths::find_file_with_format(name, format)` for callers that already
  know the target kpathsea format. Avoids the `guess_format_from_filename`
  walk, which lazily initializes every format type via `kpathsea_init_format`
  and dominates profiles for LaTeX-frontend-style callers that only need
  `kpse_tex_format`.
* Re-export `kpse_file_format_type` as `Format` and expose common format
  constants (`TEX`, `BIB`, `BST`, `CNF`, `FONTMAP`, `TYPE1`, `TRUETYPE`) in
  the `formats` module.

Maintenance refresh:

* Bump `which` 5 → 8. Removes the `which → rustix 0.38 → linux-raw-sys
  0.4` carrier — newer `which` is dependency-free for the helper paths
  this crate uses, which lets downstream consumers stay on a single
  modern rustix version.
* Bump `kpathsea_sys` 0.1.2 → 0.1.3 with `edition = "2024"` and an
  explicit `unexpected_cfgs` allowlist for the `kpathsea_docs_rs` cfg.
* Regenerate the auto-bindings file (`kpathsea_sys/src/bindings.rs`)
  with `unsafe extern "C"` blocks required by Rust 2024.
* Modernize the workspace layout: `resolver = "3"`, SPDX license
  expressions (`MIT OR Apache-2.0` instead of the deprecated
  `MIT/Apache-2.0` slash form), and pkg-config / dep version ranges
  trimmed to their major versions.

## [0.2.3] 2021-11-29

* Patch `guess_format_from_filename` for names shorter than suffixes, thanks @Jazzpirate
* Update to 2021 rust edition, minor cleanup

## [0.2.2] 2019-04-19

 * `Drop` and `Send` traits implemented for `Kpaths`
 * Welcome to @xymostech to the authors/owners team!

## [0.2.1] 2019-04-14

 * Fix bug in `guess_format_from_filename`

## [0.2.0] 2019-04-11

* More robust detection of the tex toolchain, as expected by the `kpathsea` C library
   * making the wrapper more reliable to build and use cross-platform.
   * Thanks @xymostech for tracking down and upgrading.

## [0.1.3] 2019-03-12

### Added

* `find_file` can now discover the full range of `kpathsea`-supported types, via `guess_format_from_filename`. Thank you @xymostech !