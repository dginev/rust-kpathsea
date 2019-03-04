#![deny(missing_docs)]
//! High-level Rust API for working with the kpathsea file-searching library for TeX

use kpathsea_sys::*;
use std::env::current_exe;
use std::ffi::{CStr,CString};

/// High-level interface struct for the kpathsea API
pub struct Kpaths(kpathsea);

impl Kpaths {
  /// Obtain a new kpathsea struct, with metadata for the current rust executable
  pub fn new() -> Self {
    let kpse = unsafe { kpathsea_new() };
    let current_exe_path = current_exe().expect("we need the current executable's path, for kpathsea's bookkeeping");
    let mut current_exe_str = current_exe_path.to_string_lossy();
    let program_name = CString::new(current_exe_str.to_mut().as_str()).unwrap();
    unsafe { kpathsea_set_program_name(kpse, program_name.as_ptr(), program_name.as_ptr()); }
    Kpaths(kpse)
  }

  /// Find a file base name, auto-completing with the standard TeX extensions if needed
  pub fn find_file(&self, name: &str) -> Option<String> {
    let c_name = CString::new(name).unwrap();
    let c_filename_buf = unsafe { kpathsea_find_file(
      self.0,
      c_name.as_ptr(),
      kpse_file_format_type_kpse_tex_format, // TODO: extend
      0
    )};
    
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
}
