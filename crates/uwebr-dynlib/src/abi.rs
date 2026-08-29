use std::ffi::{c_char, CStr};
use uwebr_core::component::Element;

/// Library'nin export edeceği render fonksiyonu imzası.
///
/// Component tree'yi render edip Element pointer'ı döndürür.
/// Çağrı Hubbard `Box::from_raw()` ile geri alıp free etmeli.
pub type RenderFn = unsafe extern "C" fn() -> *mut Element;

/// CSS string'ini null-terminated olarak döndüren fonksiyon.
///
/// CSS yoksa null pointer döndürür.
pub type CssFn = unsafe extern "C" fn() -> *const c_char;

/// Library unload edilmeden önce çağrılan cleanup fonksiyonu.
pub type CleanupFn = unsafe extern "C" fn();

/// Library uzantısını döndürür.
pub fn library_extension() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "dll"
    }
    #[cfg(target_os = "macos")]
    {
        "dylib"
    }
    #[cfg(target_os = "linux")]
    {
        "so"
    }
}

/// Shared library dosya adı üretir.
///
/// Örnek: `uwebr_dynlib_App.dll`
pub fn library_filename(component_name: &str) -> String {
    format!("uwebr_dynlib_{component_name}")
}

/// Shared library tam dosya yolu üretir.
pub fn library_path(target_dir: &std::path::Path, component_name: &str) -> std::path::PathBuf {
    target_dir.join(format!(
        "{}.{}",
        library_filename(component_name),
        library_extension()
    ))
}

/// Null-terminated C string'ini Rust String'e çevirir.
///
/// # Safety
/// `ptr` geçerli ve null-terminated bir C string'e işaret etmeli.
pub unsafe fn ptr_to_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    unsafe {
        let cstr = CStr::from_ptr(ptr);
        let s = cstr.to_string_lossy().into_owned();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    }
}
