# Change Log

## [0.3.0] (in development) — subprocess fallback; kpathsea_sys 0.2.0

**The crate now works on TeX distributions that ship no `libkpathsea`**
(e.g. MacTeX/BasicTeX). Diagnosed in `dginev/latexml-oxide#217`.

Backends — selected at build time; `Kpaths::is_in_process()` reports the
choice:

* **in-process** (`libkpathsea` FFI): the unchanged fast path. Now also
  on Windows, linking TeX Live's `kpathsealibw64.dll` through
  opaque-pointer bindings; format guessing there uses a Rust-side suffix
  table, drift-checked against the C walk on Linux CI.
* **subprocess**: delegates to the host's `kpsewhich`, fronted by a
  process-global `ls-R` cache — a port of Perl LaTeXML's
  `pathname_kpsewhich`/`build_kpse_cache`. Selected when no library was
  found, or explicitly.

New API: `new_subprocess()`, `with_kpsewhich(path)`,
`find_first(&[candidates])` (one spawn for a whole candidate list),
`is_in_process()`. The `KPSEWHICH` env var overrides the executable both
backends anchor on. `Format` is now a crate-owned `u32` alias —
source-compatible constants, identical API with or without the C library.

The build fails at install time when *neither* backend is possible (no
library, no `kpsewhich`), with the remedies spelled out; skipped on
docs.rs and cross-compiles, or via `KPATHSEA_SKIP_TOOLCHAIN_CHECK=1`.

Subprocess backend behavior:

* `find_file_with_format` is cache-first like `find_file`;
  `--format=NAME` only shapes the fallback call on a miss.
* candidate names starting with `-` resolve to `None` instead of being
  parsed as `kpsewhich` options.
* ambiguous `ls-R` basenames (same name under several directories) are
  evicted and resolve via `kpsewhich` directly — no single-pass
  tie-break matches kpathsea's ranking (witnesses: TL's duplicate
  `fonttext.cfg` and `hyphen.cfg`). `-dev` trees are skipped first.
* the `ls-R` cache and all direct-call outcomes (hits AND misses) are
  process-global per executable: one ~50MB cache total instead of one
  per instance, and a repeated miss costs ~1µs instead of a respawn.
* Windows: drive letters in cache results are lowercased, matching
  `kpsewhich`'s own output byte-for-byte.

kpathsea_sys 0.2.0:

* The build script **no longer panics** when `libkpathsea` is missing.
  Probe order: `KPATHSEA_NO_LINK` (force unlinked) → `KPATHSEA_LIB_DIR`
  → pkg-config → Windows: TeX Live's kpathsea DLL, with the import
  library synthesized from its export table (no headers, `.lib`, or dev
  shell needed) → graceful unlinked. Dependents read
  `DEP_KPATHSEA_LINKED`; new `LINKED: bool` constant.
* Bindings exist **only in linked builds** — unlinked builds export just
  `LINKED`. Windows uses hand-curated opaque-pointer bindings
  (`bindings_windows.rs`): the Linux-generated layouts do not hold under
  MSVC, and struct internals are never dereferenced there.
* The `kpathsea_docs_rs` cfg hack is gone — docs.rs builds work out of
  the box as unlinked builds.

Fixes:

* bare-extension lookups (`.sty` with an empty stem) no longer panic
  with a debug-build overflow in `guess_format_from_filename`.
* concurrent `Kpaths::new()` calls no longer crash the process:
  construction/teardown is serialized (libkpathsea mutates process
  globals via `putenv` and static buffers).
* names containing an interior NUL byte return `None` instead of
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