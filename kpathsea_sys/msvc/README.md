# MSVC static build config (`build_from_source` feature)

These are the **only** in-tree files needed to build a static libkpathsea on
`*-pc-windows-msvc` — and they are all **original** to this crate. The kpathsea
C sources themselves are **not** here (see Licensing below); `build.rs`
(`try_build_from_source`) fetches them at build time.

## What's here

- `kpathsea/c-auto.h` — hand-written stand-in for kpathsea's autoconf config
  header (there is no `./configure` step on MSVC). Encodes the MSVC/UCRT feature
  set; maintained by hand. Revisit on a `KPSE_REF` bump.
- `kpathsea/paths.h` — stub `DEFAULT_*` path strings; the host's `texmf.cnf`
  overrides all of them at runtime.
- `config.h` — one-line shim (`#include <kpathsea/config.h>`) for the few
  utility files that include bare `config.h` (autotools puts the generated
  header at the build root).

## Source acquisition (fetch, not vendor)

`build.rs` obtains the `texk/kpathsea` C sources from:
1. `KPATHSEA_SRC_DIR` if set (offline / pre-fetched builds), else
2. a sparse, shallow `git` fetch from the TeX Live source mirror at the pinned
   commit `KPSE_REF` (default = kpathsea **6.4.1 / TL2025**, matching
   `bindings_windows.rs` and latexml-oxide's `build_static_kpathsea.sh`).

It then compiles the Windows/MSVC compile set (`KPATHSEA_MSVC_SOURCES` in
`build.rs`) with these headers → a static libkpathsea → in-process, self-contained
link (no runtime `kpathsealibw64.dll`). **Zero source patches** — the MSVC shims
live in kpathsea's own `win32lib.h`.

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
