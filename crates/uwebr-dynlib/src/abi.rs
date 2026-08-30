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

/// Script state'ini JSON olarak export eden fonksiyon.
///
/// Null-terminated JSON string pointer'ı döndürür.
pub type ExportStateFn = unsafe extern "C" fn() -> *const c_char;

/// Script state'ini JSON'dan import eden fonksiyon.
///
/// Null-terminated JSON string pointer'ı alır.
pub type ImportStateFn = unsafe extern "C" fn(*const c_char);

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::path::Path;

    #[test]
    fn dynlib_ptr_to_string_valid() {
        let cstr = CString::new("hello world").unwrap();
        let ptr = cstr.as_ptr();
        let result = unsafe { ptr_to_string(ptr) };
        assert_eq!(result, Some("hello world".to_string()));
    }

    #[test]
    fn dynlib_ptr_to_string_empty_string_returns_none() {
        let cstr = CString::new("").unwrap();
        let ptr = cstr.as_ptr();
        let result = unsafe { ptr_to_string(ptr) };
        assert_eq!(result, None);
    }

    #[test]
    fn dynlib_ptr_to_string_null_returns_none() {
        let result = unsafe { ptr_to_string(std::ptr::null()) };
        assert_eq!(result, None);
    }

    #[test]
    fn dynlib_library_path_construction() {
        let target_dir = Path::new("/project/target");
        let path = library_path(target_dir, "App");
        let ext = library_extension();
        assert_eq!(
            path.file_name().unwrap().to_str().unwrap(),
            format!("uwebr_dynlib_App.{ext}")
        );
        assert_eq!(path.parent().unwrap(), target_dir);
    }

    #[test]
    fn dynlib_library_path_various_names() {
        let target = Path::new("/out");
        let ext = library_extension();

        let p1 = library_path(target, "Widget");
        assert!(p1
            .to_string_lossy()
            .contains(&format!("uwebr_dynlib_Widget.{ext}")));

        let p2 = library_path(target, "MyApp");
        assert!(p2
            .to_string_lossy()
            .contains(&format!("uwebr_dynlib_MyApp.{ext}")));

        let p3 = library_path(target, "x");
        assert!(p3
            .to_string_lossy()
            .contains(&format!("uwebr_dynlib_x.{ext}")));
    }

    #[test]
    fn dynlib_library_extension_is_valid() {
        let ext = library_extension();
        assert!(ext == "dll" || ext == "so" || ext == "dylib");
    }

    #[test]
    fn dynlib_ptr_to_string_special_chars() {
        let cstr = CString::new("hello\nworld\ttab").unwrap();
        let ptr = cstr.as_ptr();
        let result = unsafe { ptr_to_string(ptr) };
        assert_eq!(result, Some("hello\nworld\ttab".to_string()));
    }

    #[test]
    fn dynlib_library_filename_format() {
        assert_eq!(library_filename("App"), "uwebr_dynlib_App");
        assert_eq!(library_filename("MyWidget"), "uwebr_dynlib_MyWidget");
        assert_eq!(library_filename(""), "uwebr_dynlib_");
    }
}
