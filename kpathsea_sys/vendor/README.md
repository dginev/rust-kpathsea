# Vendored kpathsea (for the `vendored` feature)

This tree lets `kpathsea_sys` build a **static** libkpathsea from source with
`cc` on `*-pc-windows-msvc`, so downstream binaries link kpathsea **in-process**
and stay self-contained (no runtime `kpathsealibw64.dll`). Enabled by the
`vendored` cargo feature; see `../build.rs` (`try_vendored`) and
`../../docs/MSVC_STATIC_LINK_SCOPE.md` for the full rationale.

## Provenance

- Upstream: `texk/kpathsea` from <https://github.com/TeX-Live/texlive-source>
- Commit: `217a915b4c7c8439647454b533927dbc1b7de3f4`
- kpathsea version: `6.4.3/dev`
- License: LGPL v2.1 (`kpathsea/COPYING.LESSERv2`)

> **Pre-publish TODO:** re-pin to a tagged **released** kpathsea (e.g. 6.4.2, as
> shipped by TeX Live 2025/2026) instead of a `/dev` snapshot. This is cosmetic,
> not a correctness issue: the library version is decoupled from the host TeX
> tree (it anchors on the host `kpsewhich` and reads the host `texmf.cnf`), so a
> vendored 6.4.x serves any host year — see the "Version skew" section of the
> scope doc.

## Layout

- `kpathsea/` — pristine upstream sources. The `.c` files are exactly the
  Windows/MSVC compile set (`Makefile.am` base list + `getopt.c`/`getopt1.c` +
  `win32lib.c` + `knj.c`); all `.h` are copied verbatim. The `win32/` `mktex*`
  on-the-fly *generation* helpers are intentionally omitted — this is a
  lookup-only build (`MAKE_TEX_*_BY_DEFAULT 0`).
- `generated/kpathsea/c-auto.h` — hand-written stand-in for the autoconf config
  header, for the MSVC feature set.
- `generated/kpathsea/paths.h` — stub `DEFAULT_*` path strings; the host's
  `texmf.cnf` overrides all of them at runtime.
- `generated/config.h` — one-line shim (`#include <kpathsea/config.h>`) for the
  three utility files that include bare `config.h` (autotools puts the generated
  header at the build root).

## Build recipe (mirrored in `build.rs::try_vendored`)

- Defines: `MAKE_KPSE_DLL` (marks "compiling libkpathsea" — exposes internal
  `static inline` helpers gated behind it) **and** `NO_KPSE_DLL` (keeps
  `KPSEDLL` empty, so no `__declspec` — correct for a static link).
- Includes (in order): `generated/`, `kpathsea/`... plus `generated/kpathsea`
  wins for `c-auto.h`/`paths.h`; `kpathsea/` provides the rest and the
  sibling-style bare includes.
- Link libs: `shell32` (`CommandLineToArgvW`), `user32` (`CharLowerA`),
  `advapi32` (`GetUserNameA`).
- **Zero source patches** to the upstream `.c`/`.h` — the MSVC shims live in
  kpathsea's own `win32lib.h`.

To refresh the vendored sources, re-copy from the same upstream paths at the new
commit and re-run the crate's tests on windows-msvc.
