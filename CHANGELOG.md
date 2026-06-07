# Change Log

## [0.3.0] (in development) — subprocess fallback; kpathsea_sys 0.2.0

The headline: **the crate now works on TeX distributions that ship no
`libkpathsea` at all** (e.g. MacTeX/BasicTeX on macOS — no header, no
dylib, no `kpathsea.pc`). Diagnosed in latexml-oxide's macOS portability
probe (`dginev/latexml-oxide#217`).

Backends:

* `Kpaths` is now backend-dispatched:
  * **in-process** (`libkpathsea` FFI) — unchanged fast path, selected
    automatically when the library is found at build time;
  * **subprocess** — delegates to the host's `kpsewhich` executable,
    fronted by a one-shot cache of the TeX tree's `ls-R` databases
    (a faithful port of Perl LaTeXML's `pathname_kpsewhich` +
    `build_kpse_cache`, which never link libkpathsea). Selected
    automatically when the library was NOT found at build time, or
    explicitly via the new constructors.
* New API: `Kpaths::new_subprocess()`, `Kpaths::with_kpsewhich(path)`,
  `Kpaths::find_first(&[candidates])` (one subprocess call for a whole
  candidate list on cache miss), `Kpaths::is_in_process()`.
* `KPSEWHICH` env var overrides the `kpsewhich` executable both backends
  anchor on (resolved through PATH when a bare name): the subprocess
  backend invokes it, the in-process backend hands it to
  `kpathsea_set_program_name`.
* The build **fails at install time when neither backend is possible** —
  no `libkpathsea` to link against AND no `kpsewhich` to delegate to —
  with the remedies spelled out, instead of compiling a crate that can
  never resolve a file. The probe uses the same per-platform PATH rules
  as the runtime (`which` crate: PATHEXT/`.exe` on Windows). Skipped
  automatically on docs.rs and when cross-compiling; skipped explicitly
  via `KPATHSEA_SKIP_TOOLCHAIN_CHECK=1` (build-here-deploy-there setups).
* Subprocess-backend behavior notes:
  * `find_file_with_format` consults the `ls-R` cache before shelling out,
    exactly like `find_file`; `--format=NAME` only shapes the fallback
    `kpsewhich` call on a cache miss — so the "fast path" guidance from
    0.2.5 holds on both backends.
  * candidate names beginning with `-` are never passed to `kpsewhich`
    (they would be parsed as options) and simply resolve to `None`.
  * the `ls-R` cache **evicts ambiguous basenames** (a name listed under
    more than one subdirectory): no single-pass tie-break can reproduce
    kpathsea's path-spec ranking from raw `ls-R` order — TL ships two
    `fonttext.cfg`s (first-wins picks csLaTeX's IL2 one) AND two
    `hyphen.cfg`s (Perl's last-wins picks antomega's over babel's).
    Evicted names resolve through the direct (memoized) `kpsewhich`
    call — ground truth by construction. `-dev` pre-release
    subdirectories are skipped before ambiguity detection (otherwise
    every kernel file would be ambiguous against its latex-dev twin).
  * the `ls-R` cache is process-global, shared across instances per
    `kpsewhich` executable — as Perl's `$kpse_cache` always was.
    (Per-instance copies were ~50MB each on a full TeX Live, multiplied
    by every live instance; shared, it is one ~50MB cache total.)
  * direct-call outcomes (hits and misses) are memoized alongside the
    cache, process-wide per executable: re-probing the same absent name
    never costs a second process spawn, even from another thread's
    instance — the dominant lookup cost for TeX frontends, and the only
    lookup path on hosts without `ls-R` databases (MiKTeX). Divergence
    from the Perl original, which re-spawns; staleness matches the
    one-shot `ls-R` cache semantics.

kpathsea_sys 0.2.0:

* The build script **no longer panics** when `libkpathsea` is missing:
  probe order is `KPATHSEA_LIB_DIR` env override → pkg-config → graceful
  unlinked build (types/constants still available; a `cargo:warning`
  explains the fallback). Dependents can read `DEP_KPATHSEA_LINKED`
  (`links = "kpathsea"` metadata) — the high-level crate uses it to
  select its backend at compile time.
* New `kpathsea_sys::LINKED: bool` constant.
* The `kpathsea_docs_rs` cfg hack is gone — docs.rs builds (no TeX) now
  work out of the box as unlinked builds.

Fixes:

* `guess_format_from_filename`: the alt-suffix loop now carries the same
  length guard as the suffix loop; bare-extension lookups (a `.sty` with
  an empty stem) no longer panic with a subtraction overflow in debug
  builds. (Previously tracked downstream in latexml-oxide as a
  catch_unwind workaround. The two suffix loops are now one helper.)
* Concurrent `Kpaths::new()` calls no longer crash the process:
  libkpathsea's `kpse_set_program_name` mutates process-global state
  (static path buffers, `putenv`), so the in-process backend serializes
  construction/teardown behind a static lock. (Observed as garbled
  `lstat(...) failed` aborts under parallel `cargo test`.)
* Lookups of names containing an interior NUL byte return `None` instead
  of panicking in `CString::new`.

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