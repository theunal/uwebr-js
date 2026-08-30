use anyhow::Result;
use std::time::Instant;
use vello::peniko::color::palette;

use crate::metrics::Metrics;
use crate::scene::RenderScene;
use crate::scene_builder::SceneBuilder;

/// Scene assembler: owns a [`RenderScene`] and turns it into a `vello::Scene`.
///
/// Note: this type holds no GPU state. wgpu device/surface handling lives in
/// `uwebr-app::GpuContext`.
pub struct Renderer {
    width: u32,
    height: u32,
    scene: RenderScene,
    needs_redraw: bool,
    builder: SceneBuilder,
    /// Timestamp of the previous `render_frame` call, for frame-time deltas.
    last_frame_time: Option<Instant>,
    /// Duration of the last frame, in milliseconds.
    frame_time_ms: f64,
}

impl Renderer {
    /// Create a new renderer
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            scene: RenderScene::new(),
            needs_redraw: true,
            builder: SceneBuilder::new(),
            last_frame_time: None,
            frame_time_ms: 0.0,
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
    pub fn build_vello_scene(&mut self) -> vello::Scene {
        let (w, h) = (self.width, self.height);
        self.builder.build(&self.scene, w, h)
    }

    /// Render a frame (builds vello scene — GPU submission handled by caller)
    pub fn render_frame(&mut self) -> Result<vello::Scene> {
        let now = Instant::now();
        if let Some(last) = self.last_frame_time {
            self.frame_time_ms = now.duration_since(last).as_secs_f64() * 1000.0;
        }
        self.last_frame_time = Some(now);

        let scene = self.build_vello_scene();
        self.needs_redraw = false;
        Ok(scene)
    }

    /// Frames per second derived from the last measured frame time.
    ///
    /// Zero on the first frame (no previous timestamp to diff against).
    pub fn fps(&self) -> f64 {
        Metrics::fps_from_frame_time(self.frame_time_ms)
    }

    /// Duration of the last rendered frame, in milliseconds.
    pub fn frame_time_ms(&self) -> f64 {
        self.frame_time_ms
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

    #[test]
    fn test_fps_zero_on_first_frame() {
        // No previous timestamp to diff against, so the first frame reports 0.
        let mut r = Renderer::new(800, 600);
        let _ = r.render_frame().unwrap();
        assert_eq!(r.fps(), 0.0);
        assert_eq!(r.frame_time_ms(), 0.0);
    }

    #[test]
    fn test_fps_positive_after_second_frame() {
        let mut r = Renderer::new(800, 600);
        let _ = r.render_frame().unwrap();
        // Ensure a measurable gap between frames.
        std::thread::sleep(std::time::Duration::from_millis(2));
        let _ = r.render_frame().unwrap();
        assert!(r.frame_time_ms() > 0.0, "frame time should be positive");
        assert!(r.fps() > 0.0, "fps should be positive after two frames");
    }

    // ── Renderer edge-case tests ────────────────────────────────

    #[test]
    fn render_renderer_new_zero_dimensions() {
        let r = Renderer::new(0, 0);
        assert_eq!(r.size(), (0, 0));
    }

    #[test]
    fn render_renderer_resize_to_zero() {
        let mut r = Renderer::new(800, 600);
        r.resize(0, 0);
        assert_eq!(r.size(), (0, 0));
        assert!(r.needs_redraw());
    }

    #[test]
    fn render_renderer_resize_same_dimensions() {
        let mut r = Renderer::new(800, 600);
        r.resize(800, 600);
        assert!(r.needs_redraw());
    }

    #[test]
    fn render_renderer_scene_starts_empty() {
        let r = Renderer::new(800, 600);
        assert_eq!(r.scene().node_count(), 0);
    }

    #[test]
    fn render_renderer_build_vello_scene_after_update() {
        let mut r = Renderer::new(800, 600);
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::rect(
            1,
            LayoutInfo::new(0.0, 0.0, 100.0, 100.0),
            palette::css::RED,
        ));
        r.update_scene(scene);
        let vello_scene = r.build_vello_scene();
        drop(vello_scene);
        // build_vello_scene does not clear needs_redraw (only render_frame does)
        assert!(r.needs_redraw());
    }

    #[test]
    fn render_renderer_render_frame_clears_redraw() {
        let mut r = Renderer::new(800, 600);
        assert!(r.needs_redraw());
        let _ = r.render_frame().unwrap();
        assert!(!r.needs_redraw());
    }

    #[test]
    fn render_renderer_render_frame_after_scene_update() {
        let mut r = Renderer::new(800, 600);
        let _ = r.render_frame().unwrap();
        assert!(!r.needs_redraw());
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::rect(
            1,
            LayoutInfo::new(0.0, 0.0, 100.0, 100.0),
            palette::css::RED,
        ));
        r.update_scene(scene);
        assert!(r.needs_redraw());
        let _ = r.render_frame().unwrap();
        assert!(!r.needs_redraw());
    }

    #[test]
    fn render_renderer_base_color() {
        let color = Renderer::base_color();
        assert_eq!(color, palette::css::BLACK);
    }

    #[test]
    fn render_renderer_default_dimensions() {
        let r = Renderer::default();
        assert_eq!(r.size(), (800, 600));
        assert!(r.needs_redraw());
    }

    #[test]
    fn render_renderer_large_dimensions() {
        let r = Renderer::new(7680, 4320);
        assert_eq!(r.size(), (7680, 4320));
    }
}
