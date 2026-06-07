#![deny(missing_docs)]
//! High-level Rust API for working with the kpathsea file-searching library for TeX
//!
//! Two backends are provided:
//!
//! * **in-process** — FFI calls into the system `libkpathsea` (the fast
//!   path, microseconds per lookup). Selected automatically when the
//!   library was found at build time (pkg-config, or the
//!   `KPATHSEA_LIB_DIR` override — see `kpathsea_sys`'s build script).
//! * **subprocess** — delegates to the host TeX distribution's own
//!   `kpsewhich` executable, fronted by a one-shot cache of the TeX
//!   tree's `ls-R` databases. Selected automatically when `libkpathsea`
//!   was *not* found at build time (e.g. MacTeX/BasicTeX ship no library
//!   at all), or explicitly via [`Kpaths::new_subprocess`]. Because it
//!   asks the host's resolver binary, it stays in sync with the ambient
//!   distribution by construction — including MiKTeX, which reimplements
//!   kpathsea. (This mirrors how Perl LaTeXML has always resolved TeX
//!   files; see `src/subprocess.rs`.)

use kpathsea_sys::*;
#[cfg(kpathsea_linked)]
use std::ffi::{CStr, CString};
#[cfg(kpathsea_linked)]
use which::which;

mod subprocess;
use subprocess::SubprocessKpse;

/// External result type for handling library errors
pub type Result<T> = std::result::Result<T, &'static str>;

/// Re-export of the raw kpathsea format type, for callers of
/// [`Kpaths::find_file_with_format`] that want to pass a known format.
pub use kpathsea_sys::kpse_file_format_type as Format;

/// Common kpathsea format constants, re-exported for convenience.
/// The full set is available via the `kpathsea_sys` crate.
pub mod formats {
  use kpathsea_sys::*;
  /// `.tex`, `.sty`, `.cls`, `.def`, `.ltx` and related source formats.
  pub const TEX: kpse_file_format_type = kpse_file_format_type_kpse_tex_format;
  /// `.bib` bibliography source
  pub const BIB: kpse_file_format_type = kpse_file_format_type_kpse_bib_format;
  /// `.bst` bibliography style
  pub const BST: kpse_file_format_type = kpse_file_format_type_kpse_bst_format;
  /// `.cnf` kpathsea config
  pub const CNF: kpse_file_format_type = kpse_file_format_type_kpse_cnf_format;
  /// Fontmap files
  pub const FONTMAP: kpse_file_format_type = kpse_file_format_type_kpse_fontmap_format;
  /// Type 1 (`.pfa`/`.pfb`) fonts
  pub const TYPE1: kpse_file_format_type = kpse_file_format_type_kpse_type1_format;
  /// TrueType fonts
  pub const TRUETYPE: kpse_file_format_type = kpse_file_format_type_kpse_truetype_format;
}

/// The `kpsewhich --format=NAME` spelling for the constants in [`formats`],
/// used by the subprocess backend. Formats without a mapping fall back to a
/// plain lookup (kpsewhich then guesses from the suffix, like
/// [`Kpaths::find_file`]).
fn kpsewhich_format_name(format: Format) -> Option<&'static str> {
  if format == formats::TEX {
    Some("tex")
  } else if format == formats::BIB {
    Some("bib")
  } else if format == formats::BST {
    Some("bst")
  } else if format == formats::CNF {
    Some("cnf")
  } else if format == formats::FONTMAP {
    Some("map")
  } else if format == formats::TYPE1 {
    Some("type1 fonts")
  } else if format == formats::TRUETYPE {
    Some("truetype fonts")
  } else {
    None
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

/// Returns the path to the kpsewhich executable on the system.
#[cfg(kpathsea_linked)]
fn get_kpsewhich_path() -> Result<CString> {
  let kpsewhich_path = which("kpsewhich").map_err(|_| "Error finding kpsewhich executable")?;
  let kpsewhich_path_str = kpsewhich_path.to_string_lossy();
  Ok(CString::new(kpsewhich_path_str.into_owned().as_str()).unwrap())
}

impl Kpaths {
  /// Obtain a new kpathsea struct, with metadata for the current rust executable.
  ///
  /// Selects the in-process `libkpathsea` backend when the library was
  /// linked at build time, and the subprocess-`kpsewhich` backend
  /// otherwise. Use [`Kpaths::is_in_process`] to inspect the choice.
  pub fn new() -> Result<Self> {
    #[cfg(kpathsea_linked)]
    {
      let kpse = unsafe { kpathsea_new() };

      // kpathsea says we should pass in the current executable name to
      // kpathsea_set_program_name, but there are cases where this causes
      // kpathsea to fail to find the available TeX distribution. Instead, we use
      // the location of the kpsewhich executable, which ensures that we find the
      // correct TeX distribution.
      let kpsewhich_path = get_kpsewhich_path()?;

      unsafe { kpathsea_set_program_name(kpse, kpsewhich_path.as_ptr(), std::ptr::null()) }
      Ok(Kpaths(Backend::InProcess(kpse)))
    }
    #[cfg(not(kpathsea_linked))]
    {
      Self::new_subprocess()
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
  /// `kpsewhich` executable (bypassing `KPSEWHICH`/PATH lookup).
  pub fn with_kpsewhich<P: Into<std::path::PathBuf>>(path: P) -> Self {
    Kpaths(Backend::Subprocess(SubprocessKpse::with_kpsewhich(path.into())))
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

  /// For a given filename, try to guess the kpse format type from the file
  /// extension by looking it up in the format info table. This is a simplified
  /// version of the find_format function in kpsewhich.
  #[cfg(kpathsea_linked)]
  fn guess_format_from_filename(&self, kpse: kpathsea, filename: &str) -> kpse_file_format_type {
    if !filename.contains('.') {
      // no extension in filename, shorcircuit and default to tex
      return kpse_file_format_type_kpse_tex_format;
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

      // First, we check the suffixes for each format type. The suffixes are
      // stored as an array of strings with a null pointer denoting the last
      // value. Also, the pointer to the array can itself be null if there are
      // no suffixes.
      let mut suffix_ptr = format_info.suffix;
      while !suffix_ptr.is_null() && !unsafe { *suffix_ptr }.is_null() {
        // Pull out the suffix
        let suffix_cstr = unsafe { CStr::from_ptr(*suffix_ptr) };
        let suffix = suffix_cstr.to_str().unwrap();

        // We check if the last suffix.len() characters of the filename are
        // equal to the suffix itself. If so, then we've found a type that
        // matches our filename!
        if filename.len() > suffix.len()
          && filename.get(filename.len() - suffix.len()..) == Some(suffix)
        {
          return format_type as kpse_file_format_type;
        }

        // Go to the next suffix in the array.
        suffix_ptr = unsafe { suffix_ptr.offset(1) };
      }

      // Next, we check the alternate suffixes for each format type. This is
      // stored in the exact same way as the normal suffixes.
      // TODO(xymostech): factor this out into a function to avoid duplication
      let mut alt_suffix_ptr = format_info.alt_suffix;
      while !alt_suffix_ptr.is_null() && !unsafe { *alt_suffix_ptr }.is_null() {
        let alt_suffix_cstr = unsafe { CStr::from_ptr(*alt_suffix_ptr) };
        let alt_suffix = alt_suffix_cstr.to_str().unwrap();

        // The same length guard as the suffix loop above: without it, a
        // filename shorter than the alt-suffix (e.g. looking up a bare
        // `.sty`) underflows the subtraction and panics in debug builds.
        if filename.len() > alt_suffix.len()
          && filename.get(filename.len() - alt_suffix.len()..) == Some(alt_suffix)
        {
          return format_type as kpse_file_format_type;
        }

        alt_suffix_ptr = unsafe { alt_suffix_ptr.offset(1) };
      }
    }

    // If we don't find any matching suffixes, we guess that it's a tex file
    kpse_file_format_type_kpse_tex_format
  }

  /// Find a file base name, auto-completing with the standard TeX extensions if needed
  pub fn find_file(&self, name: &str) -> Option<String> {
    match &self.0 {
      #[cfg(kpathsea_linked)]
      Backend::InProcess(kpse) => {
        let file_format_type = self.guess_format_from_filename(*kpse, name);
        self.find_file_with_format(name, file_format_type)
      },
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
  /// before comparing suffixes. On a fresh `Kpaths` instance this parses all of
  /// the relevant texmf config/db files, which in turn dominates profiles for
  /// callers that know the format up front (e.g. a LaTeX frontend searching
  /// only for `kpse_tex_format`). Prefer this method when you already know the
  /// kpathsea format — it issues exactly one `kpathsea_find_file` call with no
  /// format-table walk.
  ///
  /// With the subprocess backend, formats from [`formats`] are passed as
  /// `kpsewhich --format=NAME`; other format values fall back to a plain
  /// lookup.
  pub fn find_file_with_format(&self, name: &str, format: kpse_file_format_type) -> Option<String> {
    match &self.0 {
      #[cfg(kpathsea_linked)]
      Backend::InProcess(kpse) => {
        let c_name = CString::new(name).unwrap();

        let c_filename_buf = unsafe { kpathsea_find_file(*kpse, c_name.as_ptr(), format, 0) };

        if !c_filename_buf.is_null() {
          let c_filepath: &CStr = unsafe { CStr::from_ptr(c_filename_buf) };
          let filepath = c_filepath.to_str().unwrap().to_owned();
          if filepath.is_empty() { None } else { Some(filepath) }
        } else {
          None
        }
      },
      Backend::Subprocess(sub) => sub.find_with_format_name(name, kpsewhich_format_name(format)),
    }
  }
}

impl Drop for Kpaths {
  /// Cleanup the kpathsea pointer in the destructor
  fn drop(&mut self) {
    match &self.0 {
      #[cfg(kpathsea_linked)]
      Backend::InProcess(kpse) => unsafe { kpathsea_finish(*kpse) },
      Backend::Subprocess(_) => {},
    }
  }
}
