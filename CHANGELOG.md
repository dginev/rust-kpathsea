# Change Log

## [0.3.3] 2026-07-14 — build_from_source: fix `_stat64i32` LNK2005 under static CRT + LTO

* **`kpathsea_sys` 0.2.3 — the `build_from_source` static link now works under
  `+crt-static` + fat-LTO.** The MSVC leg compiles `win32lib.c` with
  `-D_CRT_DECLARE_NONSTDC_NAMES=0`. Without it, `win32lib.h`'s `#define stat
  _stat` combined with the UCRT's `#define _stat _stat64i32` (`sys/stat.h`)
  rewrote the NAME of the UCRT's POSIX `stat()` compatibility wrapper to
  `_stat64i32`, so `win32lib.o` emitted its own `_stat64i32` definition — which
  collides with the real (static-CRT) `_stat64i32` (`LNK2005`) in a downstream
  `+crt-static` + fat-LTO release link. It surfaces ONLY in such a release link
  (not a plain `cargo build`) and only where the SDK makes the wrapper external
  (`_CRT_NONSTANDARD_STATIC`). Disabling the UCRT's POSIX-name compat wrappers —
  which `win32lib.h` already re-provides via its own `_`-prefixed remapping —
  drops the mangled definition entirely (`win32lib.o` now merely *references* the
  real `_stat64i32`, 0 definitions). Fixes `dginev/latexml-oxide`'s Windows
  release `.exe`. No API change; only affects the opt-in Windows-MSVC
  `build_from_source` leg.

## [0.3.2] 2026-07-14 — subprocess backend: never block on MiKTeX's on-the-fly installer

* **The subprocess backend no longer deadlocks on MiKTeX package installation.**
  MiKTeX's `kpsewhich` triggers its on-the-fly package installer when asked for a
  file whose package is known to the distribution but not installed; with the
  default `[MPM]AutoInstall = Ask` this raises a blocking (often interactive)
  prompt that hangs a non-interactive caller until an outer timeout kills it. The
  subprocess backend now prepends `--miktex-disable-installer` to every
  `kpsewhich` call on MiKTeX, so a missing package resolves to "not found"
  immediately (graceful degradation) instead of prompting. The option is detected
  once per executable by **capability probe** — `kpsewhich
  --miktex-disable-installer --version`, which does no file lookup and so cannot
  itself trigger the installer; a zero exit means the flag is understood. TeX Live
  (and any distro that rejects the option) is unaffected — the prepended prefix is
  empty. No API change; installed-file resolution is unchanged. Motivated by
  `dginev/latexml-oxide`'s Windows release on MiKTeX hosts.

## [0.3.1] 2026-07-14 — opt-in `build_from_source`; kpathsea_sys 0.2.2

* **`kpathsea_sys` 0.2.2 — new opt-in `build_from_source` feature.** Builds a
  static libkpathsea from source with `cc` instead of locating a system/DLL
  library, for an in-process, self-contained link. Supported on
  `*-pc-windows-msvc` and Unix (verified on Linux/glibc). On Windows this removes
  the runtime `kpathsealibw64.dll` dependency, so the binary launches on any
  Windows regardless of TeX distribution (unlike the default Windows path, which
  dynamically links TeX Live's DLL); on Unix it is a portable fallback pinned to a
  known kpathsea version, for cases where the default `libkpathsea-dev` probe
  isn't wanted (minimal containers, musl, Windows parity). The kpathsea sources
  (LGPL-2.1) are **not bundled** — the crate stays MIT/Apache: they are fetched at
  build time from the TeX Live source mirror at a pinned commit (kpathsea 6.4.1,
  matching the bindings + latexml-oxide's `build_static_kpathsea.sh`), or taken
  from `KPATHSEA_SRC_DIR` for offline builds. Only original config headers ship
  in-tree (`kpathsea_sys/common/`, `msvc/`, `unix/`). Off by default; no effect on
  unsupported targets. Motivated by `dginev/latexml-oxide`'s Windows release,
  where the subprocess backend adds a fixed ~0.5 s/conversion (in-process removes
  it). See `kpathsea_sys/common/README.md` and `docs/MSVC_STATIC_LINK_SCOPE.md`.

## [0.3.0] 2026-06-07 — portable backends; kpathsea_sys 0.2.0

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