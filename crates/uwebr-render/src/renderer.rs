use anyhow::Result;
use vello::peniko::color::palette;

use crate::scene::RenderScene;
use crate::scene_builder::SceneBuilder;

/// GPU Renderer using wgpu + vello
pub struct Renderer {
    width: u32,
    height: u32,
    scene: RenderScene,
    needs_redraw: bool,
}

impl Renderer {
    /// Create a new renderer
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            scene: RenderScene::new(),
            needs_redraw: true,
        }
    }

    /// Update viewport dimensions
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.needs_redraw = true;
    }

    /// Update the scene with new render nodes
    pub fn update_scene(&mut self, scene: RenderScene) {
        self.scene = scene;
        self.needs_redraw = true;
    }

    /// Get the current scene
    pub fn scene(&self) -> &RenderScene {
        &self.scene
    }

    /// Get viewport dimensions
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Check if a redraw is needed
    pub fn needs_redraw(&self) -> bool {
        self.needs_redraw
    }

    /// Build a vello scene from the current render scene
    pub fn build_vello_scene(&self) -> vello::Scene {
        SceneBuilder::build_scene(&self.scene, self.width, self.height)
    }

    /// Render a frame (builds vello scene — GPU submission handled by caller)
    pub fn render_frame(&mut self) -> Result<vello::Scene> {
        let scene = self.build_vello_scene();
        self.needs_redraw = false;
        Ok(scene)
    }

    /// Get background color for the render surface
    pub fn base_color() -> vello::peniko::Color {
        palette::css::BLACK
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
    use crate::scene::{LayoutInfo, RenderNode};
    use vello::peniko::color::palette;

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
        assert!(r.needs_redraw());
    }

    #[test]
    fn test_scene_update() {
        let mut r = Renderer::new(800, 600);
        assert!(r.needs_redraw());

        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::rect(
            1,
            LayoutInfo::new(0.0, 0.0, 100.0, 50.0),
            palette::css::RED,
        ));
        r.update_scene(scene);

        assert_eq!(r.scene().node_count(), 1);
        assert!(r.needs_redraw());
    }

    #[test]
    fn test_build_vello_scene() {
        let mut r = Renderer::new(800, 600);
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::rect(
            1,
            LayoutInfo::new(10.0, 20.0, 100.0, 50.0),
            palette::css::BLUE,
        ));
        r.update_scene(scene);

        let _vello_scene = r.build_vello_scene();
    }

    #[test]
    fn test_render_frame() {
        let mut r = Renderer::new(800, 600);
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::rect(
            1,
            LayoutInfo::new(0.0, 0.0, 800.0, 600.0),
            palette::css::WHITE,
        ));
        r.update_scene(scene);

        let _vello_scene = r.render_frame().unwrap();
        assert!(!r.needs_redraw());
    }

    #[test]
    fn test_default_renderer() {
        let r = Renderer::default();
        assert_eq!(r.width, 800);
        assert_eq!(r.height, 600);
    }
}
