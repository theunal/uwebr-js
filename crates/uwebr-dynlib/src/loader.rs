use crate::abi::{CleanupFn, CssFn, RenderFn};
use anyhow::{Context, Result};
use libloading::Library;
use std::path::Path;
use uwebr_core::component::Element;

/// Yüklenmiş shared library'yi temsil eder.
///
/// Drop edildiğinde `cleanup()` çağrılır ve libloading otomatik unload eder.
pub struct LoadedLibrary {
    _lib: Library,
    render: RenderFn,
    css: Option<CssFn>,
    cleanup: Option<CleanupFn>,
}

// SAFETY: libloading::Library Send+Sync; extern "C" fonksiyon pointer'ları da öyle.
unsafe impl Send for LoadedLibrary {}
unsafe impl Sync for LoadedLibrary {}

impl LoadedLibrary {
    /// Shared library'yi diskten yükler ve sembolleri resolve eder.
    pub fn load(path: &Path) -> Result<Self> {
        unsafe {
            let lib = Library::new(path)
                .with_context(|| format!("failed to load library: {}", path.display()))?;

            // Resolve symbols while Library is still alive, then copy the fn pointers.
            // Symbol borrows from Library, so we must copy before moving Library into the struct.
            let render_fn: RenderFn = *lib
                .get::<RenderFn>(b"render")
                .context("symbol 'render' not found in library")?;

            let css_fn: Option<CssFn> = lib.get::<CssFn>(b"css").ok().map(|s| *s);
            let cleanup_fn: Option<CleanupFn> = lib.get::<CleanupFn>(b"cleanup").ok().map(|s| *s);

            Ok(Self {
                _lib: lib,
                render: render_fn,
                css: css_fn,
                cleanup: cleanup_fn,
            })
        }
    }

    /// Component'i render edip Element pointer'ı döndürür.
    ///
    /// Çağrının `Box::from_raw()` ile free etmesi gerekir.
    pub fn render_element(&self) -> *mut Element {
        unsafe { (self.render)() }
    }

    /// Component'i render edip `Option<Element>` olarak döndürür.
    ///
    /// Null pointer ise `None` döner, aksi halde sahipliği caller'a geçer.
    pub fn render(&self) -> Option<Box<Element>> {
        let ptr = self.render_element();
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { Box::from_raw(ptr) })
        }
    }

    /// CSS string'ini döndürür (varsa).
    pub fn css(&self) -> Option<String> {
        unsafe {
            let css_fn = self.css?;
            let ptr = css_fn();
            crate::abi::ptr_to_string(ptr)
        }
    }

    /// Cleanup fonksiyonunu çağırır (varsa).
    pub fn cleanup(&self) {
        if let Some(cleanup_fn) = self.cleanup {
            unsafe { cleanup_fn() };
        }
    }
}

impl Drop for LoadedLibrary {
    fn drop(&mut self) {
        self.cleanup();
        // Library drop edildiğinde libloading otomatik unload eder
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loaded_library_send() {
        fn assert_send<T: Send>() {}
        assert_send::<LoadedLibrary>();
    }

    #[test]
    fn test_loaded_library_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<LoadedLibrary>();
    }
}
