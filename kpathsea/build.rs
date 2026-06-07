use std::env;

/// `kpathsea_sys` declares `links = "kpathsea"` and publishes
/// `cargo:linked={0,1}` from its build script, which Cargo exposes to us as
/// `DEP_KPATHSEA_LINKED`. Mirror it as a `cfg` so the library can select
/// the in-process backend at compile time, and fall back to the
/// subprocess-`kpsewhich` backend when the C library was not found.
///
/// A build where NEITHER precondition holds — no `libkpathsea` to link
/// against, no `kpsewhich` executable to delegate to — could never resolve
/// a file at runtime, so it fails here, at install time, with the remedies
/// spelled out. See [`require_kpsewhich`] for the escape hatches.
fn main() {
  println!("cargo:rustc-check-cfg=cfg(kpathsea_linked)");
  println!("cargo:rerun-if-env-changed=KPSEWHICH");
  println!("cargo:rerun-if-env-changed=KPATHSEA_SKIP_TOOLCHAIN_CHECK");
  // PATH is deliberately NOT a rerun trigger: it differs between shells and
  // IDEs, which would cause spurious whole-crate rebuilds. A FAILED check is
  // never cached by Cargo, so installs that hit the error below re-probe on
  // every attempt; a passing result only goes stale in the harmless
  // direction (kpsewhich later removed → runtime lookups return errors).

  if env::var("DEP_KPATHSEA_LINKED").as_deref() == Ok("1") {
    // Precondition 1: libkpathsea found — the in-process backend links.
    println!("cargo:rustc-cfg=kpathsea_linked");
    return;
  }
  require_kpsewhich();
}

/// Precondition 2: the subprocess backend needs a `kpsewhich` executable —
/// the `KPSEWHICH` env var when set, otherwise `kpsewhich` on PATH,
/// mirroring the runtime resolver in lib.rs. The `which` crate applies the
/// same per-platform lookup rules as the runtime (PATHEXT/`.exe` on
/// Windows, plain PATH on Linux/macOS), so check and runtime agree by
/// construction.
fn require_kpsewhich() {
  // Build-here-deploy-there setups (Docker build stages, CI images without
  // TeX) can opt out explicitly.
  if env::var_os("KPATHSEA_SKIP_TOOLCHAIN_CHECK").is_some() {
    println!(
      "cargo:warning=kpathsea: skipping the TeX toolchain check \
       (KPATHSEA_SKIP_TOOLCHAIN_CHECK is set); lookups will fail at \
       runtime unless `kpsewhich` is available there."
    );
    return;
  }
  // docs.rs has no TeX; the unlinked build documents fine without one.
  if env::var_os("DOCS_RS").is_some() {
    return;
  }
  // Cross-compiling: the build host's PATH says nothing about the target.
  if env::var("HOST") != env::var("TARGET") {
    println!(
      "cargo:warning=kpathsea: cross-compiling; skipping the `kpsewhich` \
       presence check (the build host's PATH says nothing about the \
       target system)."
    );
    return;
  }

  let explicit = env::var("KPSEWHICH").ok();
  let name = explicit.as_deref().unwrap_or("kpsewhich");
  if which::which(name).is_err() {
    let detail = match &explicit {
      Some(name) => format!("KPSEWHICH is set to `{name}`, but no such executable was found."),
      None => "no `kpsewhich` executable was found on PATH.".to_string(),
    };
    eprintln!(
      "\n\
       kpathsea: no usable TeX backend.\n\
       \n\
       libkpathsea was not found at build time (see the kpathsea_sys \
       warning above),\nand {detail}\n\
       \n\
       This crate needs ONE of:\n\
       \n\
       1. libkpathsea to link against (in-process backend): install your\n\
       \u{20}  platform's kpathsea development package — `libkpathsea-dev` on\n\
       \u{20}  Debian/Ubuntu, `texlive` via Homebrew, or a TeX Live source\n\
       \u{20}  install — or point KPATHSEA_LIB_DIR at the library directory.\n\
       \n\
       2. a `kpsewhich` executable to delegate to (subprocess backend):\n\
       \u{20}  install any TeX distribution (TeX Live, MacTeX/BasicTeX, MiKTeX)\n\
       \u{20}  and ensure `kpsewhich` is on PATH, or set KPSEWHICH to its\n\
       \u{20}  location.\n\
       \n\
       Building on a machine without TeX for deployment to one with it?\n\
       Set KPATHSEA_SKIP_TOOLCHAIN_CHECK=1 to bypass this check.\n"
    );
    std::process::exit(1);
  }
}
