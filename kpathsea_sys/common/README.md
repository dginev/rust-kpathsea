# From-source build config (`build_from_source` feature)

These are the **only** in-tree files needed to build a static libkpathsea from
source — and they are all **original** to this crate. The kpathsea C sources
themselves are **not** here (see Licensing below); `build.rs`
(`try_build_from_source`) fetches them at build time.

## Layout

Hand-written stand-ins for kpathsea's autoconf output (the `cc` build has no
`./configure` step). `build.rs` picks the per-OS `c-auto.h` and always adds
`common/`; revisit the headers on a `KPSE_REF` bump.

- `common/config.h` — one-line shim (`#include <kpathsea/config.h>`) for the few
  units that include bare `config.h` (autotools puts the generated header at the
  build root).
- `common/kpathsea/paths.h` — stub `DEFAULT_*` path strings; the host's
  `texmf.cnf` overrides all of them at runtime.
- `msvc/kpathsea/c-auto.h` — the MSVC/UCRT feature set (windows-msvc leg).
- `unix/kpathsea/c-auto.h` — the POSIX/glibc feature set (Unix leg; verified on
  Linux, best-effort elsewhere).

## Source acquisition (fetch, not vendor)

`build.rs` obtains the `texk/kpathsea` C sources from:
1. `KPATHSEA_SRC_DIR` if set (offline / pre-fetched builds), else
2. a sparse, shallow `git` fetch from the TeX Live source mirror at the pinned
   commit `KPSE_REF` (default = kpathsea **6.4.1 / TL2025**, matching
   `bindings_windows.rs` and latexml-oxide's `build_static_kpathsea.sh`).

It then compiles the per-OS source set (`KPATHSEA_COMMON_SOURCES` plus the leg's
units, in `build.rs`) with these headers → a static libkpathsea → in-process,
self-contained link (on Windows, no runtime `kpathsealibw64.dll`). **Zero source
patches.**

## When to use it

On Unix the packaged library is the normal route: the default probe finds
`libkpathsea` via pkg-config (Debian/Ubuntu `libkpathsea-dev`, Homebrew
`texlive`, …), and `KPATHSEA_STATIC=1` statically links the system
`libkpathsea.a`. `build_from_source` is the portable fallback — a binary pinned
to exactly `KPSE_REF`, independent of any system install (minimal containers,
musl, parity with the Windows build). Only Windows/MSVC truly needs it.

## Licensing (why the source is fetched, not bundled)

kpathsea is **LGPL-2.1**; this crate is **MIT OR Apache-2.0**. To keep the crate
free of LGPL-licensed files, the LGPL sources are fetched at build time rather
than committed here. Only the original config headers above ship in-tree.

Note that a binary which **statically links** the fetched libkpathsea contains
LGPL code and so carries LGPL §6 obligations (source availability + a relink
provision). The `build_from_source` feature is opt-in and off by default; the
crate's own default builds link nothing of kpathsea's. Downstreams that enable it
for distribution must satisfy §6 (e.g. ship this crate + the `KPSE_REF` pin as
the "scripts used to control compilation").
