use anyhow::Result;

/// GPU Renderer using wgpu + vello
pub struct Renderer {
    width: u32,
    height: u32,
}

impl Renderer {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    pub fn render_frame(&self) -> Result<()> {
        // TODO: Initialize wgpu device, render scene
        Ok(())
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new(800, 600)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_renderer_creation() {
        let r = Renderer::new(1024, 768);
        assert_eq!(r.width, 1024);
        assert_eq!(r.height, 768);
    }

    #[test]
    fn test_renderer_resize() {
        let mut r = Renderer::new(800, 600);
        r.resize(1920, 1080);
        assert_eq!(r.width, 1920);
        assert_eq!(r.height, 1080);
    }
}
