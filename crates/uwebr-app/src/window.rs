use anyhow::Result;

/// Window wrapper
pub struct Window {
    width: u32,
    height: u32,
}

impl Window {
    pub fn new(_title: &str, width: u32, height: u32) -> Result<Self> {
        // TODO: Create winit window
        Ok(Self { width, height })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}
