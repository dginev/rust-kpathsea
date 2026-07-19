//! Regression: with no usable `kpsewhich`, the in-process backend must still
//! initialize and honor `TEXINPUTS`, instead of `Kpaths::new` giving up and
//! every lookup returning `None`.
//!
//! In-process only — a subprocess build has nothing to shell out to — so the
//! backend is detected at runtime and other configurations skip cleanly.

use kpathsea::Kpaths;
use std::io::Write;

/// Path comparison is normalized rather than byte for byte.
#[cfg(not(windows))]
fn normalized(path: &str) -> String {
  path.replace('\\', "/").to_lowercase()
}

#[test]
fn texinputs_honored_without_kpsewhich() {
  // Detect the active backend from a normal construction (real environment,
  // before we perturb it below). Skip where the in-process backend is not the
  // one in play — the current_exe fallback only applies there.
  let in_process = match Kpaths::new() {
    Ok(kpse) => kpse.is_in_process(),
    // No usable backend in this environment — nothing this test can assert.
    Err(_) => return,
  };
  if !in_process {
    return;
  }

  // A file reachable ONLY via TEXINPUTS: a unique name in a fresh temp dir,
  // nowhere near a texmf tree or the current directory.
  let dir = std::env::temp_dir().join(format!("kpse_texinputs_probe_{}", std::process::id()));
  std::fs::create_dir_all(&dir).unwrap();
  let file = dir.join("lxo_texinputs_probe.tex");
  writeln!(std::fs::File::create(&file).unwrap(), "% probe").unwrap();

  // TEXINPUTS is read lazily on the first lookup, so set it before constructing.
  // The trailing separator appends kpathsea's own default path; it is `;` on
  // Windows, where `:` belongs to the drive letter and would split the entry.
  // SAFETY: this is the only test in this integration binary, so no other
  // thread mutates the environment concurrently.
  let sep = if cfg!(windows) { ';' } else { ':' };
  unsafe { std::env::set_var("TEXINPUTS", format!("{}{sep}", dir.display())) };

  // Every way `kpsewhich_executable()` can fail: a nonexistent absolute path,
  // a bare name that is nowhere on PATH, an empty override, and a real file
  // that is not executable. In each case a linked `Kpaths::new()` must still
  // succeed, stay in-process, and honor TEXINPUTS — before the fallback anchor
  // it returned `Err`, leaving libkpathsea uninitialized and every lookup `None`.
  let not_executable = dir.join("not_executable_kpsewhich");
  std::fs::write(&not_executable, b"#!/bin/sh\n").unwrap();
  for bogus in [
    "/nonexistent/definitely-not-kpsewhich",
    "definitely-not-a-command-on-path",
    "",
    not_executable.to_string_lossy().as_ref(),
  ] {
    // SAFETY: as above — single-test binary, no concurrent env mutation.
    unsafe { std::env::set_var("KPSEWHICH", bogus) };
    let kpse = Kpaths::new()
      .unwrap_or_else(|e| panic!("linked Kpaths::new must not fail for KPSEWHICH={bogus:?}: {e}"));
    assert!(
      kpse.is_in_process(),
      "expected the in-process backend to survive KPSEWHICH={bogus:?}"
    );
    // Resolution through the degraded anchor is asserted off Windows only.
    // Windows kpathsea locates `texmf.cnf` relative to the program name, so an
    // anchor outside the distribution finds no config and resolves nothing; the
    // library still initializes, which is strictly better than the old `Err`,
    // but the TEXINPUTS guarantee needs a `kpsewhich` there.
    #[cfg(not(windows))]
    assert_eq!(
      kpse
        .find_file("lxo_texinputs_probe.tex")
        .as_deref()
        .map(normalized),
      Some(normalized(&file.to_string_lossy())),
      "a TEXINPUTS-only file must resolve through the fallback-anchored libkpathsea \
       with KPSEWHICH={bogus:?}"
    );
  }

  let _ = std::fs::remove_dir_all(&dir);
}
