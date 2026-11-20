#![deny(missing_docs)]
//! High-level Rust API for working with the kpathsea file-searching library for TeX
//!
//! Two backends are provided:
//!
//! * **in-process** — FFI calls into the system `libkpathsea` (the fast
//!   path, microseconds per lookup). Selected automatically when the
//!   library was found at build time: pkg-config or the
//!   `KPATHSEA_LIB_DIR` override on Unix, TeX Live's own kpathsea DLL
//!   (found next to `kpsewhich.exe`) on Windows — see `kpathsea_sys`'s
//!   build script. `KPATHSEA_NO_LINK=1` at build time forces the
//!   subprocess backend even when a library is available.
//! * **subprocess** — delegates to the host TeX distribution's own
//!   `kpsewhich` executable, fronted by a one-shot cache of the TeX
//!   tree's `ls-R` databases. Selected automatically when `libkpathsea`
//!   was *not* found at build time (e.g. MacTeX/BasicTeX ship no library
//!   at all), or explicitly via [`Kpaths::new_subprocess`]. Because it
//!   asks the host's resolver binary, it stays in sync with the ambient
//!   distribution by construction — including MiKTeX, which reimplements
//!   kpathsea. (This mirrors how Perl LaTeXML has always resolved TeX
//!   files; see `src/subprocess.rs`.)
//!
//! Latency profile (measured on a full TeX Live): in-process lookups are
//! tens of µs, hit or miss. Subprocess lookups are *bimodal*: sub-µs on an
//! `ls-R` cache hit — faster than the FFI path — but a cache miss costs a
//! `kpsewhich` spawn (tens to hundreds of ms, memoized process-wide per
//! executable and TEXMF roots, so a repeated miss is only ever paid once,
//! regardless of which instance or thread asks).

#[cfg(kpathsea_linked)]
use kpathsea_sys::*;
#[cfg(kpathsea_linked)]
use std::ffi::{CStr, CString};
use std::path::PathBuf;

mod subprocess;
use subprocess::SubprocessKpse;
mod texmf_roots;
pub use texmf_roots::{TEXMF_ROOT_VARS, TrimmedRoot, trim_texmf_root, trimmed_texmf_roots};

/// External result type for handling library errors
pub type Result<T> = std::result::Result<T, &'static str>;

/// The one unrecoverable configuration: no linked `libkpathsea` to call and no
/// `kpsewhich` executable to spawn, so nothing can ever be resolved.
#[cfg(not(kpathsea_linked))]
const NO_BACKEND: &str = "kpathsea: no libkpathsea is linked and no `kpsewhich` executable is \
                          available — TeX file lookups cannot resolve. Install a TeX distribution, \
                          or point KPSEWHICH at its kpsewhich.";

/// Kpathsea file-format type, for callers of
/// [`Kpaths::find_file_with_format`] that want to pass a known format.
///
/// Values mirror the C `kpse_file_format_type` enum; the common ones are
/// named in [`formats`], and any other enum value is passed through
/// faithfully. Owned by this crate — not re-exported from `kpathsea_sys` —
/// so the API is identical whether or not `libkpathsea` was linked (the
/// `kpathsea_sys` surface only exists in linked builds).
pub type Format = u32;

/// Common kpathsea format constants. Values are the C
/// `kpse_file_format_type` enum's (drift-checked against `kpathsea_sys` in
/// linked test builds); other enum values can be passed as plain [`Format`]
/// numbers.
pub mod formats {
  use super::Format;
  /// `.tex`, `.sty`, `.cls`, `.def`, `.ltx` and related source formats.
  pub const TEX: Format = 26;
  /// `.bib` bibliography source
  pub const BIB: Format = 6;
  /// `.bst` bibliography style
  pub const BST: Format = 7;
  /// `.cnf` kpathsea config
  pub const CNF: Format = 8;
  /// Fontmap files
  pub const FONTMAP: Format = 11;
  /// Type 1 (`.pfa`/`.pfb`) fonts
  pub const TYPE1: Format = 32;
  /// TrueType fonts
  pub const TRUETYPE: Format = 36;
}

/// The wrapper's format constants must stay in lockstep with the C enum.
/// (Only checkable where the bindgen bindings exist: linked, non-Windows.)
#[cfg(all(test, kpathsea_linked, not(windows)))]
mod format_drift {
  #[test]
  fn formats_match_libkpathsea() {
    use kpathsea_sys::*;
    assert_eq!(crate::formats::TEX, kpse_file_format_type_kpse_tex_format);
    assert_eq!(crate::formats::BIB, kpse_file_format_type_kpse_bib_format);
    assert_eq!(crate::formats::BST, kpse_file_format_type_kpse_bst_format);
    assert_eq!(crate::formats::CNF, kpse_file_format_type_kpse_cnf_format);
    assert_eq!(
      crate::formats::FONTMAP,
      kpse_file_format_type_kpse_fontmap_format
    );
    assert_eq!(
      crate::formats::TYPE1,
      kpse_file_format_type_kpse_type1_format
    );
    assert_eq!(
      crate::formats::TRUETYPE,
      kpse_file_format_type_kpse_truetype_format
    );
  }
}

/// The `kpsewhich --format=NAME` spelling for the constants in [`formats`],
/// used by the subprocess backend. Formats without a mapping fall back to a
/// plain lookup (kpsewhich then guesses from the suffix, like
/// [`Kpaths::find_file`]).
fn kpsewhich_format_name(format: Format) -> Option<&'static str> {
  match format {
    formats::TEX => Some("tex"),
    formats::BIB => Some("bib"),
    formats::BST => Some("bst"),
    formats::CNF => Some("cnf"),
    formats::FONTMAP => Some("map"),
    formats::TYPE1 => Some("type1 fonts"),
    formats::TRUETYPE => Some("truetype fonts"),
    _ => None,
  }
}

enum Backend {
  #[cfg(kpathsea_linked)]
  InProcess(kpathsea),
  Subprocess(SubprocessKpse),
}

/// High-level interface struct for the kpathsea API
pub struct Kpaths(Backend);

// A kpathsea pointer is Send because it owns the data that it references. It
// is not Sync, because calling kpathsea functions on it is not thread-safe.
// (The subprocess backend is inherently Send.)
unsafe impl Send for Kpaths {}

/// Resolve the `kpsewhich` executable: the `KPSEWHICH` env var when set
/// (a bare name is looked up through PATH, an absolute path is taken as-is),
/// otherwise `kpsewhich` on PATH. Both backends anchor on this executable —
/// in-process as the program name handed to `kpathsea_set_program_name`,
/// subprocess as the resolver to invoke.
fn kpsewhich_executable() -> Result<PathBuf> {
  let name = std::env::var("KPSEWHICH").unwrap_or_else(|_| "kpsewhich".to_string());
  which::which(&name).map_err(|_| "Error finding kpsewhich executable")
}

/// A path as a `CString`, for `kpathsea_set_program_name`.
#[cfg(kpathsea_linked)]
fn path_to_cstring(path: PathBuf) -> Result<CString> {
  CString::new(path.to_string_lossy().into_owned()).map_err(|_| "path contains a NUL byte")
}

/// [`kpsewhich_executable`] as a `CString`, for `kpathsea_set_program_name`.
#[cfg(kpathsea_linked)]
fn get_kpsewhich_path() -> Result<CString> {
  path_to_cstring(kpsewhich_executable()?)
}

/// The running executable's path as a `CString` — the second-choice anchor.
#[cfg(kpathsea_linked)]
fn current_exe_program_name() -> Result<CString> {
  path_to_cstring(std::env::current_exe().map_err(|_| "current executable path is unavailable")?)
}

/// The program name to anchor libkpathsea on, degrading but never failing:
/// `kpsewhich` (which also locates the TeX distribution) → the running
/// executable → a literal.
///
/// Refusing to initialize is worse than a degraded anchor: an uninitialized
/// libkpathsea returns `None` for every lookup and ignores `TEXINPUTS` &c,
/// which need no TeX distribution at all. So a linked [`Kpaths::new`] can
/// always succeed.
///
/// How much a degraded anchor still resolves is platform-dependent. On Unix it
/// only costs TeX-*distribution* discovery, and env-var search paths keep
/// working. On Windows the anchor also governs where `texmf.cnf` is looked for,
/// so an anchor outside the distribution finds no config and resolves nothing —
/// initialized but inert, which is still better than the `Err` this replaces.
///
/// Both sources are injected so every tier is testable — `current_exe()` does
/// not fail on a live system.
#[cfg(kpathsea_linked)]
fn program_name_anchor(
  kpsewhich: Result<CString>,
  current_exe: impl FnOnce() -> Result<CString>,
) -> CString {
  kpsewhich
    .or_else(|_| current_exe())
    .unwrap_or_else(|_| CString::from(c"kpsewhich"))
}

/// Every tier degrades without failing — what makes a linked
/// [`Kpaths::new`] infallible.
#[cfg(all(test, kpathsea_linked))]
mod anchor_tiers {
  use super::*;

  #[test]
  fn prefers_kpsewhich_and_leaves_current_exe_unevaluated() {
    let mut consulted = false;
    let got = program_name_anchor(Ok(CString::new("/usr/bin/kpsewhich").unwrap()), || {
      consulted = true;
      Ok(CString::new("/proc/self/exe").unwrap())
    });
    assert_eq!(got.to_str().unwrap(), "/usr/bin/kpsewhich");
    assert!(
      !consulted,
      "current_exe must not be consulted when kpsewhich resolves"
    );
  }

  #[test]
  fn degrades_to_current_exe_when_kpsewhich_is_unresolvable() {
    let got = program_name_anchor(Err("no kpsewhich"), || {
      Ok(CString::new("/proc/self/exe").unwrap())
    });
    assert_eq!(got.to_str().unwrap(), "/proc/self/exe");
  }

  #[test]
  fn degrades_to_a_literal_when_every_source_fails() {
    // Previously propagated `Err`, leaving libkpathsea uninitialized.
    let got = program_name_anchor(Err("no kpsewhich"), || Err("no current_exe"));
    assert_eq!(got.to_str().unwrap(), "kpsewhich");
  }
}

/// The `TEXMFCNF` this process appended a TeX Live tree to, if any (see [`texmfcnf_extension`]).
#[cfg(kpathsea_linked)]
static APPENDED_TEXMFCNF: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// The `TEXMFCNF` value a linked [`Kpaths::new`] set so that the library also reads the anchor's TeX Live `texmf.cnf`,
/// or `None` when it set none (the usual case: the library's compiled-in directories already hold one, or the user
/// set `TEXMFCNF`). For a caller's log, next to [`Kpaths::is_in_process`].
pub fn appended_texmfcnf() -> Option<&'static str> {
  #[cfg(kpathsea_linked)]
  return APPENDED_TEXMFCNF.get().map(String::as_str);
  #[cfg(not(kpathsea_linked))]
  None
}

/// The `TEXMFCNF` that lets a linked library read the `texmf.cnf` of the install-tl tree it is anchored on, or `None`
/// to leave its configuration search alone.
///
/// A distro-built library (Debian's `libkpathsea6`: `/etc/texmf/web2c:…:$SELFAUTOPARENT/share/texmf/web2c`) does not
/// search install-tl's `$SELFAUTOPARENT/texmf-dist/web2c`; with no distro tree beside it, it finds no `texmf.cnf` and
/// resolves nothing (dginev/latexml-oxide#948, a Docker image). Only then, when `selfautoparent` (kpathsea's own, from
/// `kpathsea_set_program_name`) holds `texmf-dist/web2c/texmf.cnf` and the user set no `TEXMFCNF`, are TeX Live's
/// own default directories of an install-tl tree (the subset an install-tl tree populates) appended, in its order (`$SELFAUTOGRANDPARENT/texmf-local/web2c`,
/// `$SELFAUTOPARENT`, `$SELFAUTOPARENT/texmf-dist/web2c`). The leading `:` keeps the compiled-in default first.
/// `TEXMFROOT`/`TEXMFDIST` do not steer where `texmf.cnf` is looked for (`CNF_ENVS`, tex-file.c), so they block
/// nothing: the appended file then reads them as the anchor's own `kpsewhich` does. `default_finds_cnf` runs last and
/// only once the rest holds: where the compiled-in default already finds a `texmf.cnf`, nothing is appended, since
/// kpathsea would read a file found on both paths twice. Not on Windows, where the anchor already governs the search.
#[cfg(kpathsea_linked)]
#[cfg_attr(windows, allow(dead_code))]
fn texmfcnf_extension(
  selfautoparent: Option<&str>,
  user_set: impl Fn(&str) -> bool,
  is_file: impl Fn(&std::path::Path) -> bool,
  default_finds_cnf: impl FnOnce() -> bool,
) -> Option<String> {
  if cfg!(windows) || user_set("TEXMFCNF") {
    return None;
  }
  // Rebuilt from its components: a trailing or doubled `/` would join into `//`, kpathsea's recursive search.
  let root: PathBuf = std::path::Path::new(selfautoparent?).components().collect();
  let web2c = root.join("texmf-dist/web2c");
  if !root.is_absolute() || !is_file(&web2c.join("texmf.cnf")) || default_finds_cnf() {
    return None;
  }
  let local = root
    .parent()
    .map(|grandparent| grandparent.join("texmf-local/web2c"));
  let dirs = local.iter().chain([&root, &web2c]);
  let dirs: Vec<&str> = dirs.map(|dir| dir.to_str()).collect::<Option<_>>()?;
  Some(format!(":{}", dirs.join(":")))
}

#[cfg(all(test, kpathsea_linked))]
mod texmfcnf_extension_rule {
  use super::*;
  use std::path::Path;

  const ROOT: &str = "/usr/local/texlive/2025";
  const EXPECTED: &str = ":/usr/local/texlive/texmf-local/web2c:/usr/local/texlive/2025:\
                          /usr/local/texlive/2025/texmf-dist/web2c";

  /// The rule with a tree holding `texmf-dist/web2c/texmf.cnf` under [`ROOT`] and a compiled-in default that finds none.
  fn extension(root: Option<&str>, user_set: impl Fn(&str) -> bool) -> Option<String> {
    let cnf = Path::new(ROOT).join("texmf-dist/web2c/texmf.cnf");
    texmfcnf_extension(root, user_set, |path| path == cnf, || false)
  }

  #[test]
  #[ignore = "written against an install-tl TeX Live 2025 layout, not portable; run locally with `cargo test -- --ignored`"]
  fn appends_the_install_tl_directories_after_the_default() {
    let got = extension(Some(ROOT), |_| false);
    assert_eq!(got.as_deref(), (!cfg!(windows)).then_some(EXPECTED));
  }

  #[test]
  #[ignore = "written against an install-tl TeX Live 2025 layout, not portable; run locally with `cargo test -- --ignored`"]
  fn a_trailing_or_doubled_slash_never_joins_into_a_recursive_element() {
    for root in [
      "/usr/local/texlive/2025/",
      "/usr/local/texlive/2025//",
      "/usr/local//texlive/2025",
    ] {
      let got = extension(Some(root), |_| false);
      assert_eq!(
        got.as_deref(),
        (!cfg!(windows)).then_some(EXPECTED),
        "{root}"
      );
    }
  }

  #[test]
  #[ignore = "written against an install-tl TeX Live 2025 layout, not portable; run locally with `cargo test -- --ignored`"]
  fn a_tree_at_the_file_system_root_has_no_grandparent() {
    let got = texmfcnf_extension(Some("/"), |_| false, |_| true, || false);
    let expected = ":/:/texmf-dist/web2c";
    assert_eq!(got.as_deref(), (!cfg!(windows)).then_some(expected));
  }

  #[test]
  #[ignore = "written against an install-tl TeX Live 2025 layout, not portable; run locally with `cargo test -- --ignored`"]
  fn no_root_or_no_texmf_cnf_under_it_changes_nothing() {
    assert_eq!(extension(None, |_| false), None);
    assert_eq!(extension(Some("/usr"), |_| false), None);
    assert_eq!(extension(Some("relative/2025"), |_| false), None);
  }

  #[test]
  #[ignore = "written against an install-tl TeX Live 2025 layout, not portable; run locally with `cargo test -- --ignored`"]
  fn only_a_user_texmfcnf_blocks_it() {
    assert_eq!(extension(Some(ROOT), |var| var == "TEXMFCNF"), None);
    // TEXMFROOT/TEXMFDIST steer the variables, not where texmf.cnf is looked for (latexml-oxide sets TEXMFROOT itself).
    for var in ["TEXMFROOT", "TEXMFDIST", "TEXINPUTS"] {
      let got = extension(Some(ROOT), |set| set == var);
      assert_eq!(
        got.as_deref(),
        (!cfg!(windows)).then_some(EXPECTED),
        "{var}"
      );
    }
  }

  #[test]
  #[ignore = "written against an install-tl TeX Live 2025 layout, not portable; run locally with `cargo test -- --ignored`"]
  fn a_default_that_finds_texmf_cnf_keeps_it_and_is_asked_last() {
    let cnf = Path::new(ROOT).join("texmf-dist/web2c/texmf.cnf");
    let got = texmfcnf_extension(Some(ROOT), |_| false, |path| path == cnf, || true);
    assert_eq!(got, None);
    // Not consulted when the tree has no texmf.cnf or the user set TEXMFCNF.
    let probe = |user_set: bool, has_cnf: bool| {
      let mut asked = false;
      texmfcnf_extension(
        Some(ROOT),
        |_| user_set,
        |_| has_cnf,
        || {
          asked = true;
          false
        },
      );
      asked
    };
    assert!(!probe(true, true));
    assert!(!probe(false, false));
    assert_eq!(probe(false, true), !cfg!(windows));
  }
}

/// Whether the library, as configured now, finds a `texmf.cnf` along its configuration path — the search
/// `read_all_cnf` (cnf.c) makes when a first lookup reads the configuration, here without reading the file.
///
/// # Safety
/// `kpse` must be a live instance after `kpathsea_set_program_name` whose configuration has not been read, used under
/// [`KPSE_GLOBAL_LOCK`]. Afterwards it reads its configuration as it would have at construction: the cnf path it caches
/// is the one `read_all_cnf` would compute from the environment now, and `followup_search` (the flag pathsearch.c's
/// `search()` sets after the first, unlogged search) is reset, so that first search stays unlogged.
#[cfg(all(kpathsea_linked, not(windows)))]
unsafe fn cnf_path_finds_texmf_cnf(kpse: kpathsea) -> bool {
  unsafe {
    // As `kpathsea_cnf_get` around `read_all_cnf`: no variable is looked up in a configuration not yet read.
    (*kpse).doing_cnf_init = 1;
    let path = kpathsea_init_format(kpse, kpse_file_format_type_kpse_cnf_format);
    let found = kpathsea_path_search(kpse, path, c"texmf.cnf".as_ptr(), 0);
    (*kpse).doing_cnf_init = 0;
    (*kpse).followup_search = 0;
    if found.is_null() {
      return false;
    }
    free(found.cast());
    true
  }
}

/// The probe on a live instance. CI's distro `kpsewhich` anchors on `/`, so [`Kpaths::new`] never reaches the probe
/// there; this runs it directly: anchored on the `kpsewhich` found, the library's own configuration path finds a
/// `texmf.cnf`, and the instance still resolves after the probe.
#[cfg(all(test, kpathsea_linked, not(windows)))]
mod cnf_probe_on_a_live_instance {
  use super::*;

  #[test]
  #[ignore = "written against an install-tl TeX Live 2025 layout, not portable; run locally with `cargo test -- --ignored`"]
  fn probe_finds_the_default_texmf_cnf_and_lookups_still_resolve() {
    let Ok(program_name) = get_kpsewhich_path() else {
      eprintln!(
        "SKIP probe_finds_the_default_texmf_cnf_and_lookups_still_resolve: no kpsewhich on PATH"
      );
      return;
    };
    let guard = KPSE_GLOBAL_LOCK
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner);
    let kpse = unsafe { kpathsea_new() };
    unsafe { kpathsea_set_program_name(kpse, program_name.as_ptr(), std::ptr::null()) };
    // SAFETY: a fresh instance after `kpathsea_set_program_name`, its configuration not yet read, under the lock.
    let found = unsafe { cnf_path_finds_texmf_cnf(kpse) };
    drop(guard);
    let kpaths = Kpaths(Backend::InProcess(kpse));
    assert!(
      found,
      "the library's own configuration path finds no texmf.cnf beside the kpsewhich it is anchored on"
    );
    assert!(
      kpaths.find_file("article.cls").is_some(),
      "article.cls must still resolve after the probe"
    );
  }
}

// The opaque Windows bindings of `kpathsea_sys` 0.2.3 omit `kpathsea_xputenv`; TeX Live's kpathsea DLL exports it
// (`KPSEDLL` in lib.h) and the `build-from-source` leg compiles xputenv.c.
#[cfg(all(kpathsea_linked, windows))]
unsafe extern "C" {
  /// `void kpathsea_xputenv (kpathsea kpse, const_string var, const_string value)` (lib.h).
  fn kpathsea_xputenv(kpse: kpathsea, var: const_string, value: const_string);
}

/// Hand the library the TEXMF roots without their trailing separator (dginev/latexml-oxide#950): `kpathsea_var_value`
/// (variable.c) prefers the environment to texmf.cnf, so the cnf's `$TEXMFROOT/texmf-dist` expands without a `//`.
///
/// # Safety
/// `kpse` must be a live instance whose configuration has not been read (`kpathsea_set_program_name` reads none; the
/// first `kpathsea_cnf_get` does), used under [`KPSE_GLOBAL_LOCK`]: `kpathsea_xputenv` writes the process environment.
#[cfg(kpathsea_linked)]
unsafe fn put_texmf_roots(kpse: kpathsea, roots: &[TrimmedRoot]) {
  for root in roots {
    let (Ok(var), Ok(used)) = (CString::new(root.var), CString::new(root.used.as_str())) else {
      continue;
    };
    // SAFETY: NUL-terminated strings that outlive the call (the library copies them into the `VAR=VALUE` it puts).
    unsafe { kpathsea_xputenv(kpse, var.as_ptr(), used.as_ptr()) };
  }
}

/// libkpathsea's `kpse_set_program_name` mutates process-global state:
/// static path buffers and the environment via `putenv`. Two threads
/// constructing `Kpaths` concurrently interleave those buffers and crash
/// libkpathsea ("Can't get directory of program name", with garbled paths —
/// observed under parallel `cargo test`). Construction and (defensively)
/// teardown are serialized behind this lock; lookups on an existing
/// instance are unaffected.
#[cfg(kpathsea_linked)]
static KPSE_GLOBAL_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Walk a NULL-terminated C array of suffix strings (the layout kpathsea
/// uses for `format_info.suffix` and `format_info.alt_suffix`; the array
/// pointer itself may be NULL when a format has no suffixes), returning
/// `true` when `filename` ends with one of them. The filename must be
/// strictly longer than the suffix: a bare extension with an empty stem
/// (e.g. `.sty`) matches nothing, so it falls through to the default
/// format instead.
///
/// # Safety
/// `list` must be NULL or point to a NULL-terminated array of valid
/// NUL-terminated C strings.
#[cfg(all(kpathsea_linked, not(windows)))]
unsafe fn filename_has_suffix_in(filename: &str, mut list: *mut const_string) -> bool {
  while !list.is_null() && !unsafe { *list }.is_null() {
    let suffix = unsafe { CStr::from_ptr(*list) }.to_str().unwrap();
    if filename.len() > suffix.len() && filename.ends_with(suffix) {
      return true;
    }
    list = unsafe { list.offset(1) };
  }
  false
}

/// For a given filename, try to guess the kpse format type from the file
/// extension by looking it up in the format info table. This is a simplified
/// version of the find_format function in kpsewhich.
#[cfg(all(kpathsea_linked, not(windows)))]
fn guess_format_from_filename(kpse: kpathsea, filename: &str) -> Format {
  if !filename.contains('.') {
    // no extension in filename, shorcircuit and default to tex
    return formats::TEX;
  }
  // We go through each format type
  for format_type in 0..kpse_file_format_type_kpse_last_format {
    let format_info: &mut kpse_format_info_type =
      unsafe { &mut (*kpse).format_info[format_type as usize] };
    if format_info.type_.is_null() {
      // If this format hasn't been initialized yet, initialize it now.
      // Otherwise, it won't have the list of suffixes initialized.
      unsafe {
        kpathsea_init_format(kpse, format_type as kpse_file_format_type);
      }
    }

    // Check the suffixes, then the alternate suffixes, for this format
    // type. If the filename ends with one of them, we've found our format.
    if unsafe { filename_has_suffix_in(filename, format_info.suffix) }
      || unsafe { filename_has_suffix_in(filename, format_info.alt_suffix) }
    {
      return format_type as Format;
    }
  }

  // If we don't find any matching suffixes, we guess that it's a tex file
  formats::TEX
}

/// libkpathsea's per-format suffix tables, dumped from a live library
/// (TeX Live 2025, `kpathsea_init_format` + `format_info[..].suffix` /
/// `.alt_suffix`) in C-walk order: ascending formats, suffix list before
/// alt-suffix list within each format. Used on Windows, where the bindings
/// are opaque-pointer-only and `format_info` cannot be walked (see
/// `kpathsea_sys/src/bindings_windows.rs`). Linked non-Windows test builds
/// compile it too, for the drift canary in [`suffix_table_drift`] — Linux
/// CI verifies this table against the linked library's own walk.
#[cfg(all(kpathsea_linked, any(windows, test)))]
#[rustfmt::skip]
const FORMAT_SUFFIXES: &[(Format, &str)] = &[
  (0, "gf"), (1, "pk"),
  (3, ".tfm"), (4, ".afm"), (5, ".base"), (6, ".bib"), (7, ".bst"),
  (8, ".cnf"), (9, "ls-R"), (9, "ls-r"), (10, ".fmt"), (11, ".map"),
  (12, ".mem"), (13, ".mf"), (14, ".pool"), (15, ".mft"), (16, ".mp"),
  (17, ".pool"), (19, ".ocp"), (20, ".ofm"), (20, ".tfm"), (21, ".opl"),
  (21, ".pl"), (22, ".otp"), (23, ".ovf"), (23, ".vf"), (24, ".ovp"),
  (24, ".vpl"), (25, ".eps"), (25, ".epsi"),
  (26, ".tex"), (26, ".sty"), (26, ".cls"), (26, ".fd"), (26, ".aux"),
  (26, ".bbl"), (26, ".def"), (26, ".clo"), (26, ".ldf"),
  (28, ".pool"), (29, ".dtx"), (29, ".ins"), (30, ".pro"),
  (32, ".pfa"), (32, ".pfb"), (33, ".vf"), (35, ".ist"),
  (36, ".ttf"), (36, ".ttc"), (36, ".TTF"), (36, ".TTC"), (36, ".dfont"),
  (37, ".t42"), (37, ".T42"), (42, ".web"), (42, ".ch"),
  (43, ".w"), (43, ".web"), (43, ".ch"), (44, ".enc"), (46, ".sfd"),
  (47, ".otf"), (47, ".OTF"), (49, ".lig"),
  (51, ".lua"), (51, ".luatex"), (51, ".luc"), (51, ".luctex"),
  (51, ".texlua"), (51, ".texluc"), (51, ".tlu"),
  (52, ".fea"), (53, ".cid"), (53, ".cidmap"),
  (54, ".mlbib"), (54, ".bib"), (55, ".mlbst"), (55, ".bst"),
  (56, ".dll"), (56, ".so"), (57, ".ris"), (58, ".bltxml"),
];

/// Windows variant of [`guess_format_from_filename`]: same walk, same
/// match rule (suffix shorter than the filename, `ends_with`), same
/// default — over [`FORMAT_SUFFIXES`] instead of the C `format_info`
/// structs the opaque Windows bindings cannot expose.
#[cfg(all(kpathsea_linked, windows))]
fn guess_format_from_filename(_kpse: kpathsea, filename: &str) -> Format {
  if !filename.contains('.') {
    return formats::TEX;
  }
  for &(format, suffix) in FORMAT_SUFFIXES {
    if filename.len() > suffix.len() && filename.ends_with(suffix) {
      return format;
    }
  }
  formats::TEX
}

/// [`FORMAT_SUFFIXES`] is the Windows backend's substitute for walking the
/// C `format_info` structs; this canary keeps it honest against the linked
/// library on the platforms that CAN walk them. If a TeX Live update
/// changes a suffix list, this fails on Linux CI and the table gets
/// regenerated.
#[cfg(all(test, kpathsea_linked, not(windows)))]
mod suffix_table_drift {
  use super::*;

  #[test]
  fn format_suffixes_match_libkpathsea() {
    let kpaths = Kpaths::new().expect("needs a TeX toolchain with libkpathsea");
    let kpse = match &kpaths.0 {
      Backend::InProcess(kpse) => *kpse,
      Backend::Subprocess(_) => panic!("linked build should construct the in-process backend"),
    };
    let mut live: Vec<(Format, String)> = Vec::new();
    for format_type in 0..kpse_file_format_type_kpse_last_format {
      unsafe { kpathsea_init_format(kpse, format_type) };
      let info = unsafe { &(*kpse).format_info[format_type as usize] };
      for &list in &[info.suffix, info.alt_suffix] {
        let mut entry = list;
        while !entry.is_null() && !unsafe { *entry }.is_null() {
          let suffix = unsafe { CStr::from_ptr(*entry) }
            .to_str()
            .unwrap()
            .to_string();
          live.push((format_type as Format, suffix));
          entry = unsafe { entry.offset(1) };
        }
      }
    }
    let table: Vec<(Format, String)> = FORMAT_SUFFIXES
      .iter()
      .map(|&(format, suffix)| (format, suffix.to_string()))
      .collect();
    assert_eq!(
      table, live,
      "FORMAT_SUFFIXES drifted from the linked libkpathsea — regenerate it from this walk"
    );
  }
}

impl Kpaths {
  /// Obtain a new kpathsea struct, with metadata for the current rust executable.
  ///
  /// Selects the in-process `libkpathsea` backend when the library was
  /// linked at build time, and the subprocess-`kpsewhich` backend
  /// otherwise. Use [`Kpaths::is_in_process`] to inspect the choice.
  ///
  /// **On a linked build this never returns `Err`** — the program-name anchor
  /// degrades instead (see [`program_name_anchor`]). The `Result` remains for
  /// API stability and for the unlinked build, where the subprocess backend
  /// has nothing to shell out to without a `kpsewhich`.
  ///
  /// Construction itself is cheap (measured ~0.1ms, serialized
  /// process-wide on the in-process backend because
  /// `kpse_set_program_name` mutates global state). The expensive step on
  /// the in-process backend is each instance's FIRST lookup (~150ms on a
  /// full TeX Live): libkpathsea parses its config and builds a private
  /// in-memory copy of the `ls-R` database — tens of MB — per instance,
  /// whatever the format. Construct once and reuse — e.g. one instance
  /// per thread — rather than constructing (and re-warming) per lookup.
  /// The subprocess backend shares one `ls-R` cache process-wide and has
  /// no per-instance warm-up.
  ///
  /// On a linked non-Windows build, the configuration search path is fixed
  /// when `new` runs: set `TEXMFCNF` (and anything else that steers where
  /// `texmf.cnf` is looked for) before constructing, not after. See
  /// [`appended_texmfcnf`] for the one case where `new` extends it.
  ///
  /// Both backends read the TEXMF root variables ([`TEXMF_ROOT_VARS`]) without
  /// a trailing separator ([`trim_texmf_root`]): the linked library through
  /// the environment, which a linked `new` rewrites with the trimmed values;
  /// the subprocess backend through each `kpsewhich` it spawns.
  /// [`trimmed_texmf_roots`] reports them.
  pub fn new() -> Result<Self> {
    #[cfg(kpathsea_linked)]
    {
      // Prefer the `kpsewhich` location: kpathsea suggests our own executable
      // name, but that can miss the available TeX distribution.
      let program_name = program_name_anchor(get_kpsewhich_path(), current_exe_program_name);

      // Serialized: see KPSE_GLOBAL_LOCK.
      let _guard = KPSE_GLOBAL_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
      // The roots without a trailing separator (see `trim_texmf_root`), read under the lock: another construction may
      // be writing them.
      let roots = texmf_roots::texmf_root_trims();
      #[cfg_attr(windows, allow(unused_mut))]
      let mut kpse = unsafe { kpathsea_new() };
      // SAFETY: a fresh instance, under the lock; written before the configuration is first read.
      unsafe { put_texmf_roots(kpse, &roots) };
      unsafe { kpathsea_set_program_name(kpse, program_name.as_ptr(), std::ptr::null()) }
      // The configuration is read at the first lookup (`kpathsea_cnf_get`, cnf.c), so TEXMFCNF set here still counts.
      #[cfg(not(windows))]
      if let Some(texmfcnf) = texmfcnf_extension(
        std::env::var("SELFAUTOPARENT").ok().as_deref(),
        // An empty value counts as unset, as kpathsea's `init_path` ignores it.
        |var| std::env::var_os(var).is_some_and(|v| !v.is_empty()),
        |path| path.is_file(),
        || unsafe { cnf_path_finds_texmf_cnf(kpse) },
      ) && let Ok(value) = CString::new(texmfcnf.as_str())
      {
        // SAFETY: under KPSE_GLOBAL_LOCK, like the putenvs of `kpathsea_set_program_name`. The probe cached the
        // compiled-in cnf path in this instance, so a fresh one, anchored the same way, reads the extended one.
        unsafe {
          kpathsea_finish(kpse);
          kpse = kpathsea_new();
          put_texmf_roots(kpse, &roots);
          kpathsea_xputenv(kpse, c"TEXMFCNF".as_ptr(), value.as_ptr());
          kpathsea_set_program_name(kpse, program_name.as_ptr(), std::ptr::null());
        }
        let _ = APPENDED_TEXMFCNF.set(texmfcnf);
      }
      texmf_roots::record_trims(&roots);
      Ok(Kpaths(Backend::InProcess(kpse)))
    }
    #[cfg(not(kpathsea_linked))]
    {
      Self::new_subprocess().map_err(|_| {
        // Terminal: neither backend exists, so NO file can ever resolve. Say so
        // on stderr as well as in the `Err` — callers routinely `.ok()` an error
        // away, and a silently inert resolver is indistinguishable from a TeX
        // tree that simply lacks the file.
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| eprintln!("{NO_BACKEND}"));
        NO_BACKEND
      })
    }
  }

  /// Obtain a kpathsea struct that always resolves through the host's
  /// `kpsewhich` executable (located via the `KPSEWHICH` env var or PATH),
  /// regardless of whether `libkpathsea` is linked. This is the resolution
  /// strategy Perl LaTeXML uses, and the only one possible on TeX
  /// distributions that ship no `libkpathsea` (e.g. MacTeX).
  pub fn new_subprocess() -> Result<Self> {
    Ok(Kpaths(Backend::Subprocess(SubprocessKpse::new()?)))
  }

  /// Like [`Kpaths::new_subprocess`], with an explicit path to the
  /// `kpsewhich` executable (bypassing `KPSEWHICH`/PATH lookup). The path is
  /// not validated up front; a missing executable simply makes every lookup
  /// return `None`.
  pub fn with_kpsewhich<P: Into<PathBuf>>(path: P) -> Self {
    Kpaths(Backend::Subprocess(SubprocessKpse::with_kpsewhich(
      path.into(),
    )))
  }

  /// `true` when this instance calls `libkpathsea` in-process, `false`
  /// when it shells out to `kpsewhich`. Useful for callers that gate
  /// per-lookup work (e.g. format-table prewarming) on the lookup cost.
  pub fn is_in_process(&self) -> bool {
    match &self.0 {
      #[cfg(kpathsea_linked)]
      Backend::InProcess(_) => true,
      Backend::Subprocess(_) => false,
    }
  }

  /// Find a file base name, auto-completing with the standard TeX extensions if needed
  pub fn find_file(&self, name: &str) -> Option<String> {
    match &self.0 {
      #[cfg(kpathsea_linked)]
      Backend::InProcess(kpse) => {
        let file_format_type = guess_format_from_filename(*kpse, name);
        self.find_file_with_format(name, file_format_type)
      }
      Backend::Subprocess(sub) => sub.find_first(&[name]),
    }
  }

  /// Search a list of candidate names, returning the first one found.
  ///
  /// With the subprocess backend this mirrors Perl LaTeXML's
  /// `pathname_kpsewhich`: the `ls-R` cache is consulted for each candidate
  /// first, and a full miss costs only ONE `kpsewhich` invocation for the
  /// whole list. With the in-process backend it is a `find_file` loop.
  pub fn find_first(&self, candidates: &[&str]) -> Option<String> {
    match &self.0 {
      #[cfg(kpathsea_linked)]
      Backend::InProcess(_) => candidates.iter().find_map(|c| self.find_file(c)),
      Backend::Subprocess(sub) => sub.find_first(candidates),
    }
  }

  /// Find a file with a caller-supplied format, bypassing `guess_format_from_filename`.
  ///
  /// `guess_format_from_filename` walks every format type in the kpathsea format
  /// info table and lazily initializes each one (via `kpathsea_init_format`)
  /// before comparing suffixes — measured at ~15-20ms of one-time work on a
  /// fresh `Kpaths` instance. (The bulk of a first in-process lookup, ~150ms
  /// on a full TeX Live, is libkpathsea building its private in-memory `ls-R`
  /// db, which every first search pays regardless of format — see
  /// [`Kpaths::new`].) Prefer this method when you already know the kpathsea
  /// format — it issues exactly one `kpathsea_find_file` call with no
  /// format-table walk.
  ///
  /// With the subprocess backend the `ls-R` cache is consulted first, like
  /// [`Kpaths::find_file`]; on a cache miss, formats from [`formats`] are
  /// passed as `kpsewhich --format=NAME`, and other format values fall back
  /// to a plain lookup.
  ///
  /// Names containing an interior NUL byte cannot exist in a TeX tree and
  /// resolve to `None`.
  pub fn find_file_with_format(&self, name: &str, format: Format) -> Option<String> {
    match &self.0 {
      #[cfg(kpathsea_linked)]
      Backend::InProcess(kpse) => {
        let c_name = CString::new(name).ok()?;

        let c_filename_buf = unsafe { kpathsea_find_file(*kpse, c_name.as_ptr(), format, 0) };

        if !c_filename_buf.is_null() {
          let c_filepath: &CStr = unsafe { CStr::from_ptr(c_filename_buf) };
          let filepath = c_filepath.to_str().unwrap().to_owned();
          if filepath.is_empty() {
            None
          } else {
            Some(filepath)
          }
        } else {
          None
        }
      }
      Backend::Subprocess(sub) => sub.find_with_format_name(name, kpsewhich_format_name(format)),
    }
  }
}

impl Drop for Kpaths {
  /// Cleanup the kpathsea pointer in the destructor
  fn drop(&mut self) {
    match &self.0 {
      #[cfg(kpathsea_linked)]
      Backend::InProcess(kpse) => {
        // Serialized: see KPSE_GLOBAL_LOCK.
        let _guard = KPSE_GLOBAL_LOCK
          .lock()
          .unwrap_or_else(std::sync::PoisonError::into_inner);
        unsafe { kpathsea_finish(*kpse) }
      }
      Backend::Subprocess(_) => {}
    }
  }
}
