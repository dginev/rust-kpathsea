# Change Log

## [0.3.0] (in development) — portable backends; kpathsea_sys 0.2.0

**The crate now works on Linux, macOS, and Windows, with or without
`libkpathsea` present.** Every platform × backend configuration is
verified in CI. (Motivated by `dginev/latexml-oxide#217`: MacTeX/BasicTeX
ship no library at all.)

Backends — selected at build time; `Kpaths::is_in_process()` reports the
selection:

* **in-process** (`libkpathsea` FFI): the unchanged fast path. Now also
  available on Windows, linking TeX Live's `kpathsealibw64.dll` through
  opaque-pointer bindings; format guessing there uses a Rust-side suffix
  table, drift-checked against the C library on Linux and macOS CI.
* **subprocess**: delegates to the host's `kpsewhich`, fronted by a
  process-global `ls-R` cache — a port of Perl LaTeXML's
  `pathname_kpsewhich`/`build_kpse_cache`. Selected when no library is
  found, or on request.

New API: `new_subprocess()`, `with_kpsewhich(path)`,
`find_first(&[candidates])` (one spawn for a whole candidate list),
`is_in_process()`. The `KPSEWHICH` environment variable overrides the
executable both backends anchor on. `Format` is now a crate-owned `u32`
alias with source-compatible constants, identical with or without the C
library.

The build fails at install time when neither backend is possible (no
library, no `kpsewhich`), with the remedies stated; the check is skipped
on docs.rs and under cross-compilation, or explicitly via
`KPATHSEA_SKIP_TOOLCHAIN_CHECK=1`.

Subprocess backend behavior:

* `find_file_with_format` consults the cache first, like `find_file`;
  `--format=NAME` shapes only the fallback call on a miss.
* Candidate names beginning with `-` resolve to `None` rather than being
  passed to `kpsewhich` as options.
* Ambiguous `ls-R` basenames (one name under several directories) are
  evicted and resolved through `kpsewhich` directly — no single-pass
  tie-break reproduces kpathsea's ranking (witnesses: TeX Live's
  duplicate `fonttext.cfg` and `hyphen.cfg`). `-dev` trees are skipped.
* The `ls-R` cache and all direct-call outcomes (hits and misses) are
  process-global per executable: one ~50MB cache in total, and a
  repeated miss costs ~1µs rather than a process spawn.
* On Windows, drive letters in cache results are normalized to
  lowercase, matching `kpsewhich` output byte for byte.

kpathsea_sys 0.2.0:

* The build script no longer panics when `libkpathsea` is missing.
  Probe order: `KPATHSEA_NO_LINK` (force unlinked) → `KPATHSEA_LIB_DIR`
  → pkg-config → on Windows, TeX Live's kpathsea DLL, with the import
  library synthesized from its export table (no headers, `.lib`, or
  developer shell required) → unlinked build. Dependents read
  `DEP_KPATHSEA_LINKED`; new `LINKED: bool` constant.
* Bindings exist only in linked builds; unlinked builds export `LINKED`
  alone. Windows uses hand-curated opaque-pointer bindings
  (`bindings_windows.rs`): the Linux-generated layouts do not hold under
  MSVC, and struct internals are never dereferenced there.
* The `kpathsea_docs_rs` cfg hack is removed — docs.rs builds work as
  unlinked builds.

Fixes:

* Bare-extension lookups (`.sty` with an empty stem) no longer panic in
  `guess_format_from_filename` (debug-build overflow).
* Concurrent `Kpaths::new()` calls no longer crash the process:
  construction and teardown are serialized (libkpathsea mutates process
  globals via `putenv` and static buffers).
* Names containing an interior NUL byte resolve to `None` rather than
  panicking.

## [0.2.6] (skipped — superseded by 0.3.0)

## [0.2.5] 2026-05-17

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