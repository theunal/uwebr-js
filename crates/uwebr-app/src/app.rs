use crate::window::Window;
use anyhow::Result;

/// Main application entry point
pub struct App {
    title: String,
    width: u32,
    height: u32,
}

impl App {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            width: 800,
            height: 600,
        }
    }

    pub fn with_size(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn run(self) -> Result<()> {
        let _window = Window::new(&self.title, self.width, self.height)?;
        // TODO: Initialize winit event loop, render pipeline
        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new("uwebr App")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_creation() {
        let app = App::new("Test");
        assert_eq!(app.title, "Test");
        assert_eq!(app.width, 800);
    }

    #[test]
    fn test_app_with_size() {
        let app = App::new("Test").with_size(1024, 768);
        assert_eq!(app.width, 1024);
        assert_eq!(app.height, 768);
    }
}
