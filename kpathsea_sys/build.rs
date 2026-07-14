use std::env;
use std::path::PathBuf;

/// Locate `libkpathsea` and emit link directives when it is available.
///
/// Probe order:
///  1. `KPATHSEA_NO_LINK` env override — build WITHOUT linking even if the
///     library could be found (forces the high-level crate onto its
///     subprocess backend; used by CI to test that path on hosts that
///     also have the library).
///  2. `KPATHSEA_LIB_DIR` env override — link against the given directory
///     unconditionally (for TeX trees that ship the library without a
///     `kpathsea.pc`, or cross-compilation setups).
///
/// `KPATHSEA_STATIC` (orthogonal to the probe order): when set, link the
/// **static** archive (`libkpathsea.a`) instead of the shared library, baking
/// it into the binary. This yields a self-contained executable that does
/// in-process lookups with NO runtime `libkpathsea` dependency on the user's
/// system — it launches even where the shared library is absent (MacTeX, or a
/// machine with no TeX at all), degrading to empty lookups. The static archive
/// must be present at build time (`libkpathsea-dev` ships `libkpathsea.a` on
/// Debian/Ubuntu). The runtime backend is identical to a dynamic link — the
/// host's TeX tree is still resolved via the `kpsewhich`-anchored program name.
///  3. `pkg-config kpathsea` — the standard Unix route (Debian/Ubuntu
///     `libkpathsea-dev`, Homebrew `texlive`, vanilla TL source installs).
///  4. Windows native builds: the kpathsea DLL TeX Live ships next to
///     `kpsewhich.exe` (`kpathsealibw64.dll`) — see [`try_windows_dll`].
///  5. Nothing found: build WITHOUT linking. This is graceful by design —
///     MacTeX/BasicTeX, for example, ship no `libkpathsea` at all (no
///     header, no dylib, no .pc), so there is nothing to link against.
///     The high-level `kpathsea` crate reads the `linked` metadata below
///     and falls back to its subprocess-`kpsewhich` backend automatically.
fn main() {
  println!("cargo:rerun-if-env-changed=KPATHSEA_NO_LINK");
  println!("cargo:rerun-if-env-changed=KPATHSEA_STATIC");
  println!("cargo:rerun-if-env-changed=KPATHSEA_LIB_DIR");
  println!("cargo:rerun-if-env-changed=KPSEWHICH");
  println!("cargo:rustc-check-cfg=cfg(kpathsea_linked)");

  if env::var_os("KPATHSEA_NO_LINK").is_some() {
    println!(
      "cargo:warning=kpathsea_sys: KPATHSEA_NO_LINK is set; building \
       without linking. The `kpathsea` crate will use its \
       subprocess-`kpsewhich` backend."
    );
    println!("cargo:linked=0");
    return;
  }

  // `build_from_source` feature: compile a static libkpathsea from source,
  // ahead of the system-library probes so opting in wins over them. Inert
  // (returns false) on unsupported targets, which then keep the probe order.
  // Gated on the Cargo-set CARGO_FEATURE_* env var, not `cfg!(feature=…)`
  // (whose availability in build scripts is not contractual).
  if env::var_os("CARGO_FEATURE_BUILD_FROM_SOURCE").is_some() && try_build_from_source() {
    emit_linked();
    return;
  }

  // Static vs. shared link mode. `static=` bakes `libkpathsea.a` into the
  // binary (self-contained, no runtime libkpathsea dependency); the default
  // links the shared library at load time.
  let want_static = env::var_os("KPATHSEA_STATIC").is_some();
  let link_kind = if want_static { "static=" } else { "" };

  if let Ok(dir) = env::var("KPATHSEA_LIB_DIR") {
    println!("cargo:rustc-link-search=native={dir}");
    println!("cargo:rustc-link-lib={link_kind}kpathsea");
    emit_linked();
    return;
  }

  // pkg-config locates the library. For a dynamic link we let it emit the
  // directives directly. For a STATIC link we suppress its emission and emit
  // them ourselves: pkg-config deliberately refuses to statically link a
  // library found in a default system path (a guard against statically linking
  // system libraries), which would silently leave us DYNAMICALLY linked — so we
  // force `static=kpathsea` from the probe's search paths.
  if want_static {
    if let Ok(lib) = pkg_config::Config::new()
      .statik(true)
      .cargo_metadata(false)
      .probe("kpathsea")
    {
      for path in &lib.link_paths {
        println!("cargo:rustc-link-search=native={}", path.display());
      }
      println!("cargo:rustc-link-lib=static=kpathsea");
      // libkpathsea is self-contained (`pkg-config --static --libs kpathsea`
      // lists only `-lkpathsea`); emit any future private deps AFTER it so the
      // static archive's references resolve.
      for l in &lib.libs {
        if l != "kpathsea" {
          println!("cargo:rustc-link-lib={l}");
        }
      }
      emit_linked();
      return;
    }
  } else if pkg_config::Config::new().probe("kpathsea").is_ok() {
    emit_linked();
    return;
  }

  if try_windows_dll() {
    // Link directives emitted by the probe.
    emit_linked();
    return;
  }

  println!(
    "cargo:warning=kpathsea_sys: libkpathsea not found (no pkg-config entry, \
         no KPATHSEA_LIB_DIR, no TeX Live kpathsea DLL); building without \
         linking. In-process kpathsea calls are unavailable - the `kpathsea` \
         crate will use its subprocess-`kpsewhich` backend instead."
  );
  println!("cargo:linked=0");
}

/// Mark this build as linked: the in-crate cfg, and — via `links =
/// "kpathsea"` — the `DEP_KPATHSEA_LINKED` metadata read by dependents.
fn emit_linked() {
  println!("cargo:rustc-cfg=kpathsea_linked");
  println!("cargo:linked=1");
}

/// `build_from_source` feature: compile a static libkpathsea from the kpathsea
/// C sources with `cc` and link it in-process — a self-contained binary with no
/// runtime libkpathsea (and no `kpathsealibw64.dll` on Windows).
///
/// The sources are LGPL, so they are **not** bundled in this (MIT OR Apache-2.0)
/// crate: they come from `KPATHSEA_SRC_DIR`, else a build-time fetch at a pinned
/// commit ([`fetch_kpathsea_src`]). Only our own config headers ship in-tree
/// (`common/` + the per-OS `msvc/` and `unix/`).
///
/// Supported targets: windows-msvc, and Unix (verified on Linux/glibc,
/// best-effort elsewhere). Any other target returns `false` (and warns) so the
/// caller falls back to the normal probe order; a fetch or compile failure is a
/// hard error — the feature is an explicit opt-in. See `common/README.md`.
fn try_build_from_source() -> bool {
  // Pick the per-OS build leg. windows-msvc is verified; the Unix leg is
  // verified on Linux/glibc and best-effort on other Unix. MinGW/other targets
  // are unsupported and fall through to the probe order.
  let is_msvc = env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
    && env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
  let leg = if is_msvc {
    Leg {
      cfg_dir: "msvc",
      sources: &["win32lib.c", "knj.c"],
      // NO_KPSE_DLL keeps KPSEDLL empty (no `__declspec`, right for a static
      // link); the rest silence MSVC's CRT deprecation warnings.
      defines: &[
        "NO_KPSE_DLL",
        "_CRT_SECURE_NO_WARNINGS",
        "_CRT_NONSTDC_NO_WARNINGS",
        // Drop the UCRT's POSIX-name compat wrappers (`stat`, `fstat`, `open`,
        // …). win32lib.h already `#define`s those POSIX names to the `_`-prefixed
        // CRT entry points, so the wrappers are unused here — and worse,
        // win32lib.h's `#define stat _stat` combined with the UCRT's `#define
        // _stat _stat64i32` rewrites the wrapper's NAME to `_stat64i32`, so
        // win32lib.o emits a strong `_stat64i32` that collides with the real
        // (static-CRT) one under `+crt-static` + fat-LTO (LNK2005 — surfaces only
        // in a downstream maxperf/LTO release link, not a plain `cargo build`).
        // Setting this to 0 forces corecrt.h's `_CRT_INTERNAL_NONSTDC_NAMES` to
        // 0, which gates those wrappers off. Fixes dginev/latexml-oxide's Windows
        // release .exe.
        "_CRT_DECLARE_NONSTDC_NAMES=0",
      ],
      // OS imports: shell32 (CommandLineToArgvW), user32 (CharLowerA),
      // advapi32 (GetUserNameA), pulled in by win32lib.c / knj.c / hash.c.
      libs: &["shell32", "user32", "advapi32"],
    }
  } else if env::var_os("CARGO_CFG_UNIX").is_some() {
    // _GNU_SOURCE exposes the glibc extensions the Unix c-auto.h assumes
    // (fseeko, mkstemp, `struct stat` st_mtim, …); no extra link libs.
    Leg {
      cfg_dir: "unix",
      sources: &["xfseeko.c", "xftello.c"],
      defines: &["_GNU_SOURCE"],
      libs: &[],
    }
  } else {
    println!(
      "cargo:warning=kpathsea_sys: `build_from_source` supports only windows-msvc \
       and Unix targets; ignoring it and using the normal probe order."
    );
    return false;
  };

  let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));
  let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
  let common = manifest.join("common");
  let cfg = manifest.join(leg.cfg_dir);
  println!("cargo:rerun-if-changed={}", common.display());
  println!("cargo:rerun-if-changed={}", cfg.display());
  println!("cargo:rerun-if-env-changed=KPATHSEA_SRC_DIR");

  // kpathsea C source (`texk/kpathsea`): explicit `KPATHSEA_SRC_DIR`
  // (offline / pre-fetched), else fetch it.
  let src = match env::var_os("KPATHSEA_SRC_DIR") {
    Some(dir) => PathBuf::from(dir),
    None => match fetch_kpathsea_src(&out_dir) {
      Some(dir) => dir,
      None => {
        println!(
          "cargo:warning=kpathsea_sys: build_from_source could not obtain the \
           kpathsea source (git fetch failed and KPATHSEA_SRC_DIR is unset); \
           set KPATHSEA_SRC_DIR to a texk/kpathsea tree."
        );
        return false;
      }
    },
  };
  if !src.join("tex-file.c").is_file() {
    println!(
      "cargo:warning=kpathsea_sys: {} is not a kpathsea source dir (no tex-file.c).",
      src.display()
    );
    return false;
  }

  let mut build = cc::Build::new();
  build
    // Include order: the leg's `kpathsea/c-auto.h`; `common/` for
    // `kpathsea/paths.h` and the bare `config.h` shim; the source's parent for
    // `<kpathsea/*.h>`; the source dir for sibling bare includes (getopt.h).
    .include(&cfg)
    .include(&common)
    .include(src.parent().unwrap_or(&src))
    .include(&src)
    // MAKE_KPSE_DLL exposes libkpathsea's internal declarations, needed to
    // compile its own units (on Unix KPSEDLL stays empty regardless).
    .define("MAKE_KPSE_DLL", None)
    .warnings(false);
  for d in leg.defines {
    // Support `NAME=VALUE` entries (e.g. `_CRT_DECLARE_NONSTDC_NAMES=0`), not
    // only valueless `-DNAME`.
    match d.split_once('=') {
      Some((name, value)) => build.define(name, value),
      None => build.define(d, None),
    };
  }
  for f in KPATHSEA_COMMON_SOURCES.iter().chain(leg.sources) {
    build.file(src.join(f));
  }
  // Emits `cargo:rustc-link-search` + `cargo:rustc-link-lib=static=kpathsea`.
  build.compile("kpathsea");
  for l in leg.libs {
    println!("cargo:rustc-link-lib={l}");
  }
  println!(
    "cargo:warning=kpathsea_sys: built static libkpathsea from source \
     ({} .c files) — in-process, self-contained.",
    KPATHSEA_COMMON_SOURCES.len() + leg.sources.len()
  );
  true
}

/// A per-OS `build_from_source` leg: the config-header subdir, the OS-specific
/// source units, extra `cc` defines, and extra system libraries to link.
struct Leg {
  cfg_dir: &'static str,
  sources: &'static [&'static str],
  defines: &'static [&'static str],
  libs: &'static [&'static str],
}

/// Sources every leg compiles: kpathsea's base `libkpathsea_la_SOURCES` plus
/// `getopt`/`getopt1`. Each [`Leg`] adds its OS units (Windows `win32lib`/`knj`,
/// Unix `xfseeko`/`xftello`). The `win32/` mktex* *generation* helpers are
/// omitted — this is a lookup-only build.
const KPATHSEA_COMMON_SOURCES: &[&str] = &[
  "tex-file.c",
  "absolute.c",
  "atou.c",
  "cnf.c",
  "concat.c",
  "concat3.c",
  "concatn.c",
  "db.c",
  "debug.c",
  "dir.c",
  "elt-dirs.c",
  "expand.c",
  "extend-fname.c",
  "file-p.c",
  "find-suffix.c",
  "fn.c",
  "fontmap.c",
  "hash.c",
  "kdefault.c",
  "kpathsea.c",
  "line.c",
  "magstep.c",
  "make-suffix.c",
  "path-elt.c",
  "pathsearch.c",
  "proginit.c",
  "progname.c",
  "readable.c",
  "rm-suffix.c",
  "str-list.c",
  "str-llist.c",
  "tex-glyph.c",
  "tex-hush.c",
  "tex-make.c",
  "tilde.c",
  "uppercasify.c",
  "variable.c",
  "version.c",
  "xbasename.c",
  "xcalloc.c",
  "xdirname.c",
  "xfopen.c",
  "xfseek.c",
  "xftell.c",
  "xgetcwd.c",
  "xmalloc.c",
  "xopendir.c",
  "xputenv.c",
  "xrealloc.c",
  "xstat.c",
  "xstrdup.c",
  "getopt.c",
  "getopt1.c",
];

/// kpathsea source pin: the TeX Live source-mirror commit whose kpathsea (6.4.1,
/// TL2025) matches `bindings_windows.rs` — the same commit latexml-oxide's
/// `build_static_kpathsea.sh` uses on Linux/macOS. Overridable via `KPSE_REF`.
const KPSE_REF: &str = "def12ffd4d6e46bae03b3e5c7ff6f5f14dced3ab";

/// Fetch `texk/kpathsea` from the TeX Live source mirror (sparse, shallow) at
/// [`KPSE_REF`] into `<out>/kpathsea-src`, returning the `texk/kpathsea` path.
/// Returns `None` on any git failure (no git, no network, bad ref). Idempotent:
/// a prior fetch in the same `OUT_DIR` is reused.
fn fetch_kpathsea_src(out: &std::path::Path) -> Option<PathBuf> {
  let kpse_ref = env::var("KPSE_REF").unwrap_or_else(|_| KPSE_REF.to_string());
  println!("cargo:rerun-if-env-changed=KPSE_REF");
  let dir = out.join("kpathsea-src");
  let src = dir.join("texk").join("kpathsea");
  if src.join("tex-file.c").is_file() {
    return Some(src); // reuse a prior fetch
  }
  let _ = std::fs::remove_dir_all(&dir);
  std::fs::create_dir_all(&dir).ok()?;
  println!(
    "cargo:warning=kpathsea_sys: fetching kpathsea source @ {kpse_ref} \
     (sparse, shallow) — set KPATHSEA_SRC_DIR to build offline."
  );
  let git = |args: &[&str]| -> bool {
    std::process::Command::new("git")
      .current_dir(&dir)
      .args(args)
      .status()
      .map(|s| s.success())
      .unwrap_or(false)
  };
  let ok = git(&["init", "-q"])
    && git(&[
      "remote",
      "add",
      "origin",
      "https://github.com/TeX-Live/texlive-source.git",
    ])
    && git(&["sparse-checkout", "init", "--cone"])
    && git(&["sparse-checkout", "set", "texk/kpathsea"])
    && git(&[
      "fetch",
      "--depth",
      "1",
      "--filter=blob:none",
      "origin",
      &kpse_ref,
    ])
    && git(&["checkout", "-q", "FETCH_HEAD"]);
  (ok && src.join("tex-file.c").is_file()).then_some(src)
}

/// Windows: TeX Live ships its kpathsea as a DLL next to `kpsewhich.exe`
/// (`bin/windows/kpathsealibw64.dll`) — no headers, no import library.
/// Synthesize the import library from the DLL's own export table
/// (`dumpbin -exports` → `.def` → `lib.exe`), with the MSVC tools located
/// through the registry (no developer shell needed), and link against it.
/// The DLL resolves at run time through PATH — the same PATH entry that
/// made `kpsewhich.exe` findable.
///
/// Every failure path returns `false` and the build degrades to the
/// subprocess backend, exactly as if the DLL were absent. MiKTeX's
/// reimplementation DLLs (`miktex-kpathsea*.dll`) are deliberately not
/// matched: only TL's own build is known ABI-compatible with the
/// declarations in `bindings_windows.rs`.
fn try_windows_dll() -> bool {
  if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
    return false;
  }
  // Synthesizing the import library needs native MSVC tools.
  if env::var("HOST") != env::var("TARGET") {
    return false;
  }
  let kpsewhich = env::var("KPSEWHICH").unwrap_or_else(|_| "kpsewhich".to_string());
  let Ok(kpsewhich) = which::which(&kpsewhich) else {
    return false;
  };
  let Some(bin_dir) = kpsewhich.parent() else {
    return false;
  };
  let Ok(entries) = std::fs::read_dir(bin_dir) else {
    return false;
  };
  let dll = entries.flatten().map(|e| e.path()).find(|p| {
    p.file_name().is_some_and(|n| {
      let n = n.to_string_lossy().to_lowercase();
      n.starts_with("kpathsea") && n.ends_with(".dll")
    })
  });
  let Some(dll) = dll else {
    return false;
  };
  let stem = dll
    .file_stem()
    .unwrap_or_default()
    .to_string_lossy()
    .into_owned();

  let target = env::var("TARGET").unwrap_or_default();
  let Some(dumpbin) = cc::windows_registry::find_tool(&target, "dumpbin.exe") else {
    return false;
  };
  let Some(libexe) = cc::windows_registry::find_tool(&target, "lib.exe") else {
    return false;
  };
  let machine = match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
    Ok("x86_64") => "x64",
    Ok("aarch64") => "arm64",
    Ok("x86") => "x86",
    _ => return false,
  };

  // Export table → module-definition file. Export lines are
  // `ordinal hint RVA name`; anything else (headers, summary) is skipped.
  let Ok(out) = dumpbin.to_command().arg("-exports").arg(&dll).output() else {
    return false;
  };
  if !out.status.success() {
    return false;
  }
  let mut def = format!("LIBRARY {stem}\nEXPORTS\n");
  for line in String::from_utf8_lossy(&out.stdout).lines() {
    let mut fields = line.split_whitespace();
    let (Some(ordinal), Some(_hint), Some(_rva), Some(symbol)) =
      (fields.next(), fields.next(), fields.next(), fields.next())
    else {
      continue;
    };
    if ordinal.bytes().all(|b| b.is_ascii_digit()) {
      def.push_str("  ");
      def.push_str(symbol);
      def.push('\n');
    }
  }

  let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
  let def_path = out_dir.join(format!("{stem}.def"));
  let lib_path = out_dir.join(format!("{stem}.lib"));
  if std::fs::write(&def_path, def).is_err() {
    return false;
  }
  let synthesized = libexe
    .to_command()
    .arg("-nologo")
    .arg(format!("-def:{}", def_path.display()))
    .arg(format!("-out:{}", lib_path.display()))
    .arg(format!("-machine:{machine}"))
    .status()
    .map(|s| s.success())
    .unwrap_or(false);
  if !synthesized {
    return false;
  }

  println!(
    "cargo:warning=kpathsea_sys: Windows: linking TeX Live's {} \
     (import library synthesized from its export table)",
    dll.display()
  );
  println!("cargo:rustc-link-search=native={}", out_dir.display());
  println!("cargo:rustc-link-lib=dylib={stem}");
  true
}
