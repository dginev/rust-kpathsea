# Scope: static, in-process libkpathsea on Windows/MSVC

**Status:** scoped + spike done → GO → **implemented as the opt-in
`build_from_source` feature** (2026-07-13/14). Branch `msvc-static-scope`.

> **Update (2026-07-14): fetch, don't vendor.** kpathsea is LGPL-2.1 and this
> crate is MIT/Apache, so the LGPL sources are NOT bundled — `build.rs` fetches
> them at build time (pinned `KPSE_REF` = 6.4.1, matching the bindings + Linux/
> macOS `build_static_kpathsea.sh`) or takes them from `KPATHSEA_SRC_DIR`. Only
> the original config headers ship in-tree (`common/`, `msvc/`, `unix/`). The
> feature was renamed `vendored` → `build_from_source`. Everything below about
> the *compile* recipe still holds; only the source *acquisition* changed from
> vendored-tree to fetch. A statically-linked binary still carries LGPL §6
> obligations (same as the existing Linux/macOS static legs) — see
> `common/README.md`.
>
> **Update (2026-07-14): also on Unix.** `build_from_source` was generalized past
> windows-msvc to a Unix leg (its own `unix/kpathsea/c-auto.h`, the base compile
> set with `xfseeko`/`xftello` swapped in for `win32lib`/`knj`), verified
> in-process on Linux/glibc. On Unix the packaged `libkpathsea-dev` is still the
> normal route; this is the portable, pinned fallback. The recipe below stays
> Windows/MSVC-specific.
**Goal:** let a Windows/MSVC build link libkpathsea **statically and
in-process** (`KPATHSEA_STATIC`), so downstream binaries (e.g. latexml-oxide's
Windows release `.exe`) get fast in-process file lookups **and** stay
self-contained — no runtime `kpathsealibw64.dll` dependency.

## Phase-1 spike result (2026-07-13): GO — no source patches needed

Ran the de-risking compile spike against **kpathsea 6.4.3/dev** (TeX Live source
mirror; host is 6.4.2, structurally identical) with `cc` → cl.exe (VS2022
14.44). Outcome: **the full library compiles, links statically into a Rust
binary, and does correct in-process lookups — with ZERO edits to kpathsea's
`.c`/`.h` sources.** The MSVC shim surface I flagged as the top risk turned out
to be *already provided* by kpathsea's own `win32lib.h` (it remaps
`getcwd→_getcwd`, `stat→_stat`, `putenv→_putenv`, `inline→__inline`, `isascii`,
`strcasecmp`, `S_ISDIR`, and the WIN32 `dirent`/`opendir` shims). The entire
port reduced to build configuration:

- **Compile set:** the `Makefile.am` base list + `getopt.c`/`getopt1.c` +
  `win32lib.c` + `knj.c` (55 `.c`). The `win32/` `mktex*` subdir is **dropped**
  by setting `MAKE_TEX_*_BY_DEFAULT 0` (lookup-only; no on-the-fly generation).
- **Two `-D` flags:** `MAKE_KPSE_DLL` (the "compiling the library itself" marker
  that exposes internal `static inline` helpers like `str_list_init` — gated
  behind `#ifdef MAKE_KPSE_DLL`, this was the one non-obvious find) **and**
  `NO_KPSE_DLL` (keeps `KPSEDLL` expanding to empty, so no `__declspec` — correct
  for a static link).
- **Include path:** `vendor/include` (for `<kpathsea/*.h>`) + `vendor/include/kpathsea`
  (sibling-style bare `getopt.h`), plus a one-line `config.h` shim at the root
  (`#include <kpathsea/config.h>`) for the 3 utility files that include bare
  `config.h` (autotools puts the generated header at the build root).
- **Two synthesized headers** (the only hand-written artifacts): `c-auto.h`
  (~35 `HAVE_*`/`SIZEOF_*`/`PACKAGE_*`/`KPSEVERSION` lines for the MSVC feature
  set) and a `paths.h` stub (the ~58 `DEFAULT_*` path strings, runtime-overridden
  by `texmf.cnf`).
- **Link libs:** `shell32` (`CommandLineToArgvW`), `user32` (`CharLowerA`),
  `advapi32` (`GetUserNameA`) — three OS import libs.

**Smoke test** (Rust bin, static-linked, `kpathsea_set_program_name` anchored on
the host `C:\texlive\2026\bin\windows\kpsewhich.exe`, then `kpathsea_find_file`):

```
article.cls -> c:/texlive/2026/texmf-dist/tex/latex/base/article.cls
cmr10.tfm   -> c:/texlive/2026/texmf-dist/fonts/tfm/public/cm/cmr10.tfm
plain.tex   -> c:/texlive/2026/texmf-dist/tex/plain/base/plain.tex
latex.ltx   -> c:/texlive/2026/texmf-dist/tex/latex/base/latex.ltx
```

The **6.4.3/dev** library read the host **6.4.2** tree correctly — a live
confirmation of the version-skew analysis below. `dumpbin /DEPENDENTS` on the
`.exe` showed **only OS/CRT DLLs, no `kpathsealibw64.dll`** — self-contained,
launches on any Windows regardless of TeX distro. This is the whole thesis,
demonstrated end-to-end.

Remaining work is now *productization*, not feasibility: fold the recipe into
`kpathsea_sys/build.rs`, vendor a pinned 6.4.2 tarball, generate `c-auto.h`
programmatically, confirm `bindings_windows.rs` matches, and flip latexml-oxide's
release leg. (Spike artifacts were in the session scratchpad — ephemeral; this
recipe is the durable output.)

## Why (motivation)

Today `kpathsea_sys` has three Windows outcomes, none of which is
"static + portable":

| Mode | How | Perf | Portability of the shipped binary |
|---|---|---|---|
| **Dynamic DLL** (`try_windows_dll`) | find `kpathsea*.dll` next to `kpsewhich`, synthesize an import lib, link it | in-process (fast) | **poor** — runtime dep on TL's `kpathsealibw64.dll`; won't *launch* on a MiKTeX-only or no-TeX host |
| **Subprocess** (`KPATHSEA_NO_LINK`) | no link; the high-level crate shells out to `kpsewhich` | slower (a `kpsewhich` spawn per uncached lookup; process spawn is ~10× costlier on Windows) | **good** — launches everywhere, any TeX distro |
| **Static** (`KPATHSEA_STATIC`) | link `libkpathsea.a` | in-process (fast) | **good** — self-contained | **← not available on MSVC (no static lib exists)** |

Linux/macOS releases already use the static mode (`tools/build_static_kpathsea.sh`
→ `KPATHSEA_LIB_DIR` + `KPATHSEA_STATIC`), getting in-process speed *and* a
self-contained binary. Windows can't, only because there is **no
MSVC-compatible static `kpathsea.lib`** — kpathsea's autotools build is
Unix/MinGW-oriented, there is no vcpkg port, and a MinGW `.a` will not link
cleanly into an MSVC binary (CRT/ABI mismatch). So the Windows release currently
ships the subprocess backend (`KPATHSEA_NO_LINK=1`) for portability, trading
away the in-process speed.

This scope is the missing third mode for Windows.

## Target end-state

- `kpathsea_sys` can **build a static libkpathsea from vendored C sources with
  `cc`** (cl.exe on windows-msvc), the same pattern `dginev/marpa`'s
  `libmarpa-sys` now uses for libmarpa.
- Downstream flips the Windows release leg from `KPATHSEA_NO_LINK=1` to
  `KPATHSEA_STATIC=1` (or the cc-build becomes the default when no system lib is
  found on Windows). Subprocess remains the fallback when the build is opted out.
- Result: Windows release `.exe` does in-process lookups, no DLL dependency,
  launches on TL / MiKTeX / no-TeX alike.

## Approach options

1. **`cc`-crate port in `kpathsea_sys` (RECOMMENDED).** Vendor a pinned
   kpathsea source tree, compile the needed `.c` with `cc::Build`, synthesize
   the autoconf-generated headers. Full control; matches the libmarpa-sys
   precedent already in this ecosystem; no external toolchain beyond MSVC.
   Risk concentrated in MSVC C-portability + header synthesis (below).
2. **vcpkg port.** Write a vcpkg portfile so the release does
   `vcpkg install kpathsea` next to libxml2/libxslt. But there is no upstream
   vcpkg port, kpathsea is autotools (vcpkg's `vcpkg_configure_make` under MSVC
   is finicky), so this front-loads the *same* MSVC-build problem behind more
   machinery. Only attractive if a portfile can be upstreamed.
3. **MinGW static `.a` linked into MSVC** — rejected: ABI/CRT mismatch makes
   this fragile-to-broken.

Recommendation: **(1)**, with a de-risking spike first (see Phasing).

## `cc`-port plan (option 1)

### 1. Vendor the source
Pin a kpathsea release from TeX Live's `texk/kpathsea` (kpathsea has its own
version, e.g. 6.4.x matching TL2026's `kpathsea version 6.4.2`). Vendor as a
tarball in the crate (as `libmarpa-sys` vendors `libmarpa-8.6.2.tar.gz`) and
extract in `build.rs`. Prefer the standalone kpathsea dist over the full TL
tree.

### 2. Compile set
kpathsea is ~40 `.c` files (path search, `tex-file`, `elt-dirs`, `expand`,
`hash`, `cnf`, `db`, `variable`, `str-list`, `tilde`, `xmalloc`, `readable`,
`absolute`, …). The exact list is the library's `Makefile.am`
`libkpathsea_la_SOURCES` (mirror it verbatim, as the libmarpa port mirrored
`libmarpa_la_SOURCES`). Windows-specific units to include: `knj.c` (kanji /
Windows path handling) and the `win32`/`w32lib` helpers guarded by `#ifdef
WIN32`.

### 3. Generated headers (the hard part)
Autoconf normally produces:
- **`c-auto.h`** — `HAVE_*` feature macros + `SIZEOF_*` + `KPSEVERSION`. Must be
  hand-synthesized for the MSVC feature set (like the 3-line `config.h` the
  libmarpa port derives from `LIB_VERSION`, but **larger** — kpathsea probes
  many features). This is the single biggest risk item: getting the MSVC
  `HAVE_*` set right (`HAVE_UNISTD_H`=0, `HAVE_DIRENT_H`=0 → use kpathsea's
  own dir shims, `HAVE_GETCWD`, `HAVE__STAT`/`_getcwd` MSVC spellings, etc.).
- **`paths.h`** — default texmf paths from `paths.h.in`. Can be synthesized
  with the stock defaults; **runtime `texmf.cnf` overrides them anyway**, and
  the shipped binary reads the host's texmf tree via `kpsewhich`-anchored
  self-location, so the baked defaults are near-irrelevant.
- **`kpathsea/version.h`** — from the pinned version string.

### 4. MSVC portability shims
kpathsea's Windows code path historically targets **MinGW**, not MSVC (TL-Win is
MinGW-built; MiKTeX *reimplements* kpathsea rather than MSVC-building TL's — a
signal this path is under-trodden). Expect to shim MSVC gaps: `<unistd.h>`,
`<dirent.h>`, `strcasecmp`/`strncasecmp` → `_stricmp`, `popen`→`_popen`,
`getcwd`→`_getcwd`, `S_ISDIR`/mode macros, `ssize_t`, `PATH_MAX`. kpathsea ships
`c-*.h` compatibility headers that cover some; the residual MSVC set is the spike
deliverable.

### 5. build.rs integration
Add a `cc`-static branch to the existing probe order (build.rs). Proposed
precedence on Windows:
`KPATHSEA_NO_LINK` (opt-out → subprocess) → `KPATHSEA_LIB_DIR` (explicit) →
**vendored `cc` static build** (new; the default when nothing else is found and
opt-out is unset) → `try_windows_dll` (legacy dynamic; demote to last resort).
Emit `cargo:linked=1` + `cargo:rustc-cfg=kpathsea_linked` on success so the
high-level crate uses the in-process backend. Keep `KPATHSEA_STATIC` semantics
consistent with Unix. Gate the vendored build behind a cargo feature (e.g.
`vendored`) if we don't want it unconditional.

### 6. Bindings
`bindings_windows.rs` is pregenerated; confirm it matches the statically-built
ABI (same kpathsea version → same struct layout). Regenerate via bindgen against
the vendored headers if the pinned version differs from what the current
bindings were cut against.

## Version skew: does a vendored kpathsea drift from the host TeX Live?

The natural objection: **kpathsea's version moves with each TeX Live release, so
a vendored copy could go out of sync with the user's tree.** Real question,
benign answer for our use — and, crucially, it is the *same* contract the
Linux/macOS static release legs already run under in production.

Separate the two things that both "move with TL":

1. **The dumps** (`{plain,latex}.YYYY.dump.txt`) — genuinely TL-year-specific.
   Already handled, independently of this work: the release embeds a 5-year
   moving window and the runtime picks the right year via
   `kpsewhich -var-value=SELFAUTOPARENT` / `pdflatex --version`. Orthogonal to
   the kpathsea library.
2. **The kpathsea library** (the C path-search engine) — what we'd vendor. This
   does **not** need to match the host tree's year, because:

   - **It reads the *host's* config, not a baked-in one.** The high-level crate
     anchors the linked library on the host's `kpsewhich` path
     (`kpathsea_set_program_name(kpse, <host kpsewhich>, …)`, kpathsea 0.3.0
     lib.rs:133-341). So the vendored library self-locates *as if it were the
     host's kpsewhich* and reads the host's `texmf.cnf` + `ls-R` + tree. The
     baked `paths.h` defaults are overridden by the host `texmf.cnf` and are
     near-irrelevant (see §3 above).
   - **The formats it consumes are decades-stable and version-tolerant.**
     `texmf.cnf` (unknown directives ignored) and `ls-R` have not changed shape
     across the 2022–2026 window; a vendored engine reads an older-or-newer
     host tree fine.
   - **latexml-oxide only resolves long-stable file types** (`.sty`, `.cls`,
     `.tfm`, `.cnf`, `.enc`, `.map`, `.pfb`). New per-year `kpse_*_format`
     enums don't affect these lookups.
   - **The vendored C + our bindgen bindings compile together** → self-
     consistent ABI regardless of the host version.

   This is precisely how the **Linux/macOS static legs already ship**: one
   pinned static kpathsea running against whatever TL (2022–2026) or MacTeX the
   user has. It works because the runtime backend is identical to a dynamic
   link — only the *code's location* changes, not *which tree it reads*.

**So we pin one recent version (e.g. 6.4.x) and it serves the whole window.**
Bumping the vendored version is occasional hygiene (upstream bug fixes, a
genuinely new format need) — low-frequency and decoupled from per-year
correctness, **not** a per-TL-release chore.

**Zero-skew escape hatch:** the subprocess backend runs the host's *exact*
`kpsewhich`, so it is inherently version-perfect. It stays as the fallback — if
a future kpathsea/TL ever introduced a breaking search behavior the vendored
engine couldn't match, a build could opt back to subprocess with no code change
(`KPATHSEA_NO_LINK=1`). That safety valve is why vendoring carries no lock-in
risk.

## What does NOT change
- Runtime behavior: the host's texmf tree is still resolved via kpathsea's
  self-location / `texmf.cnf` — static linking changes *where the code lives*,
  not *which files it finds*. `ls-R`, `texmf.cnf`, and per-year dump selection
  are unaffected.
- Non-Windows platforms: untouched (they already have the static path).
- The subprocess backend stays as the fallback, so nothing regresses if the
  vendored build is disabled or fails to compile on a given toolchain.

## Phasing (de-risk first)
1. **Spike — ✅ DONE (2026-07-13), GO.** Compiled all 55 `.c` on windows-msvc,
   linked static, in-process lookups verified, `.exe` self-contained. Zero
   source patches; the shim surface was already in `win32lib.h`. Full recipe in
   the "Phase-1 spike result" section above. It was a few build flags, not a
   swamp.
2. **Link + smoke:** get `kpathsea_sys` to emit `linked=1`; build the high-level
   `kpathsea` crate; run its test suite on Windows; confirm `find_file` resolves
   a real texmf file in-process (no `kpsewhich` spawn).
3. **Downstream flip:** in latexml-oxide's `release.yml` Windows leg, swap
   `KPATHSEA_NO_LINK=1` → the static/vendored build; `dumpbin /DEPENDENTS` must
   show **no** `kpathsealibw64.dll`; launch smoke on a MiKTeX-only and a no-TeX
   VM; A/B a conversion's wall-clock vs the subprocess backend to quantify the
   win.
4. **Publish** kpathsea_sys/kpathsea point releases; latexml-oxide bumps + flips
   the leg.

## Effort & risk
- **Effort:** low-to-medium (revised down after the spike). The compile set +
  build.rs wiring is mechanical (libmarpa precedent) and now has a proven
  recipe. The `c-auto.h` synthesis is done in draft; no MSVC shims are needed.
- **Top risk: RESOLVED.** The feared MSVC-vs-MinGW shim surface didn't
  materialize — `win32lib.h` already provides it, and the spike compiled with
  zero source patches. Residual risks are minor: keeping the vendored `c-auto.h`
  correct across a version bump, and confirming `bindings_windows.rs` matches
  the pinned ABI. The subprocess backend remains the fallback (no regression —
  it works today).
- **Value:** in-process lookups on Windows (meaningful, since Windows process
  spawn is expensive and conversions do many lookups), plus a self-contained
  `.exe`. Quantify in Phase 3 before committing to publish.

## Decision points for the maintainer
- Go/no-go after the Phase-1 spike (MSVC shim surface).
- `vendored` as a default-on Windows behavior vs an opt-in cargo feature.
- Whether to keep `try_windows_dll` (dynamic) at all once static works, or
  retire it (its shipped-binary portability problem is why we're here).
