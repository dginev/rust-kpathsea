use pkg_config::find_library;
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

  if let Ok(dir) = env::var("KPATHSEA_LIB_DIR") {
    println!("cargo:rustc-link-search=native={dir}");
    println!("cargo:rustc-link-lib=kpathsea");
    emit_linked();
    return;
  }

  if find_library("kpathsea").is_ok() {
    // pkg-config has already emitted the link-search/link-lib directives.
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
