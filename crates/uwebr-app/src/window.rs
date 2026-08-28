use std::sync::Arc;

/// Window handle — wraps a winit window
pub struct Window {
    inner: Arc<winit::window::Window>,
}

impl Window {
    /// Create from an existing winit window
    pub fn from_winit(window: Arc<winit::window::Window>) -> Self {
        Self { inner: window }
    }

    /// Get the underlying winit window
    pub fn winit(&self) -> &winit::window::Window {
        &self.inner
    }

    pub fn inner_size(&self) -> (u32, u32) {
        let size = self.inner.inner_size();
        (size.width, size.height)
    }

    pub fn set_title(&self, title: &str) {
        self.inner.set_title(title);
    }

    pub fn request_redraw(&self) {
        self.inner.request_redraw();
    }
}
