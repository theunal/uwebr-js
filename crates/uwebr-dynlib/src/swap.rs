use crate::abi::library_extension;
use crate::loader::LoadedLibrary;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// Errors during hot-swap.
#[derive(Debug)]
pub enum SwapError {
    /// New library could not be loaded.
    LoadFailed { path: PathBuf, error: String },
    /// Render symbol not found or returned null.
    RenderFailed { detail: String },
    /// CSS could not be parsed.
    CssParseFailed { error: String },
}

impl std::fmt::Display for SwapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SwapError::LoadFailed { path, error } => {
                write!(f, "load failed ({}): {}", path.display(), error)
            }
            SwapError::RenderFailed { detail } => {
                write!(f, "render failed: {detail}")
            }
            SwapError::CssParseFailed { error } => {
                write!(f, "css parse failed: {error}")
            }
        }
    }
}

impl std::error::Error for SwapError {}

/// Hot-swap result.
#[derive(Debug)]
pub struct SwapResult {
    /// Time to first render after swap (ms).
    pub render_time_ms: u64,
    /// Whether CSS changed.
    pub css_changed: bool,
    /// Previous library path (None on first load).
    pub old_library: Option<PathBuf>,
    /// New library path.
    pub new_library: PathBuf,
}

/// Manages hot-swapping of shared libraries at runtime.
pub struct HotSwapManager {
    current: Option<LoadedLibrary>,
    library_dir: PathBuf,
    component_name: String,
    version: AtomicU32,
}

impl HotSwapManager {
    /// Create a new HotSwapManager.
    pub fn new(library_dir: PathBuf, component_name: String) -> Self {
        Self {
            current: None,
            library_dir,
            component_name,
            version: AtomicU32::new(0),
        }
    }

    /// Initial load from the expected path in library_dir.
    pub fn load_initial(&mut self) -> Result<()> {
        let path = self.expected_path();
        let lib = LoadedLibrary::load(&path)
            .with_context(|| format!("initial load failed: {}", path.display()))?;
        self.current = Some(lib);
        self.version.store(1, Ordering::Relaxed);
        log::info!("loaded initial library: {}", path.display());
        Ok(())
    }

    /// Initial load from an explicit path.
    pub fn load_initial_from(&mut self, path: &Path) -> Result<()> {
        let lib = LoadedLibrary::load(path)
            .with_context(|| format!("initial load failed: {}", path.display()))?;
        self.current = Some(lib);
        self.version.store(1, Ordering::Relaxed);
        log::info!("loaded initial library: {}", path.display());
        Ok(())
    }

    /// Try to swap to a new library. On success the old library is dropped.
    pub fn try_swap(&mut self, new_library_path: &Path) -> Result<SwapResult, SwapError> {
        self.try_swap_inner(new_library_path, false)
    }

    /// Try to swap while preserving state from the old library.
    ///
    /// Eski library'den state'i export edip yeni library'ye import eder.
    pub fn try_swap_with_state(
        &mut self,
        new_library_path: &Path,
    ) -> Result<SwapResult, SwapError> {
        self.try_swap_inner(new_library_path, true)
    }

    fn try_swap_inner(
        &mut self,
        new_library_path: &Path,
        preserve_state: bool,
    ) -> Result<SwapResult, SwapError> {
        // 1. Eski state'i export et
        let old_state = if preserve_state {
            self.current.as_ref().and_then(|lib| lib.export_state())
        } else {
            None
        };

        // 2. Yeni library'yi yükle
        let new_lib = LoadedLibrary::load(new_library_path).map_err(|e| SwapError::LoadFailed {
            path: new_library_path.to_path_buf(),
            error: e.to_string(),
        })?;

        // 3. Render test
        let render_start = std::time::Instant::now();
        let ptr = new_lib.render_element();
        let render_time_ms = render_start.elapsed().as_millis() as u64;

        if ptr.is_null() {
            return Err(SwapError::RenderFailed {
                detail: "render() returned null".into(),
            });
        }
        unsafe { drop(Box::from_raw(ptr)) };

        // 4. State'i import et
        if let Some(json) = &old_state {
            new_lib.import_state(json);
        }

        // 5. CSS karşılaştır
        let old_css = self.current.as_ref().and_then(|c| c.css());
        let new_css = new_lib.css();
        let css_changed = old_css != new_css;

        // 6. Swap
        let had_old = self.current.is_some();
        let old_path = self.expected_path();
        self.current = Some(new_lib);
        self.version.fetch_add(1, Ordering::Relaxed);

        log::info!("hot-swap completed: {}", new_library_path.display());

        Ok(SwapResult {
            render_time_ms,
            css_changed,
            old_library: if had_old { Some(old_path) } else { None },
            new_library: new_library_path.to_path_buf(),
        })
    }

    /// Render the current component.
    pub fn render(&self) -> Option<std::boxed::Box<uwebr_core::component::Element>> {
        self.current.as_ref().and_then(|lib| lib.render())
    }

    /// Get the current CSS string.
    pub fn css(&self) -> Option<String> {
        self.current.as_ref().and_then(|lib| lib.css())
    }

    /// Expected library path for the current component.
    fn expected_path(&self) -> PathBuf {
        let name = &self.component_name;
        let ext = library_extension();
        self.library_dir.join(format!("uwebr_dynlib_{name}.{ext}"))
    }

    /// Current version counter.
    pub fn version(&self) -> u32 {
        self.version.load(Ordering::Relaxed)
    }

    /// Library directory.
    pub fn library_dir(&self) -> &Path {
        &self.library_dir
    }

    /// Component name.
    pub fn component_name(&self) -> &str {
        &self.component_name
    }
}

/// Generate a versioned file path, incrementing the counter each call.
pub fn next_version_path(library_dir: &Path, component_name: &str, version: &mut u32) -> PathBuf {
    *version += 1;
    let ext = library_extension();
    library_dir.join(format!("uwebr_dynlib_{component_name}_v{version}.{ext}"))
}

/// Generate a versioned filename (name only, no directory).
pub fn versioned_filename(component_name: &str, version: u32) -> String {
    let ext = library_extension();
    format!("uwebr_dynlib_{component_name}_v{version}.{ext}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_version_path_increments() {
        let dir = PathBuf::from("/tmp/dynlib");
        let mut v = 0;

        let p1 = next_version_path(&dir, "App", &mut v);
        assert_eq!(v, 1);
        assert!(p1.to_string_lossy().contains("v1"));

        let p2 = next_version_path(&dir, "App", &mut v);
        assert_eq!(v, 2);
        assert!(p2.to_string_lossy().contains("v2"));
    }

    #[test]
    fn test_versioned_filename() {
        let name = versioned_filename("Counter", 3);
        assert!(name.contains("v3"));
        assert!(name.contains("Counter"));
        assert!(name.ends_with(library_extension()));
    }

    #[test]
    fn test_hot_swap_manager_new() {
        let mgr = HotSwapManager::new(PathBuf::from("/tmp"), "App".into());
        assert_eq!(mgr.version(), 0);
        assert_eq!(mgr.component_name(), "App");
    }

    #[test]
    fn test_swap_error_display() {
        let err = SwapError::LoadFailed {
            path: PathBuf::from("/tmp/bad.dll"),
            error: "not found".into(),
        };
        assert!(format!("{err}").contains("load failed"));
    }

    #[test]
    fn test_swap_error_render_display() {
        let err = SwapError::RenderFailed {
            detail: "null pointer".into(),
        };
        assert!(format!("{err}").contains("render failed"));
    }

    #[test]
    fn test_swap_error_css_display() {
        let err = SwapError::CssParseFailed {
            error: "bad syntax".into(),
        };
        assert!(format!("{err}").contains("css parse failed"));
    }

    #[test]
    fn dynlib_next_version_path_overflow() {
        let dir = PathBuf::from("/tmp/dynlib");
        let mut v = u32::MAX - 2;
        let p1 = next_version_path(&dir, "App", &mut v);
        assert_eq!(v, u32::MAX - 1);
        assert!(p1.to_string_lossy().contains(&format!("v{}", u32::MAX - 1)));
        let p2 = next_version_path(&dir, "App", &mut v);
        assert_eq!(v, u32::MAX);
        assert!(p2.to_string_lossy().contains(&format!("v{}", u32::MAX)));
    }
}
