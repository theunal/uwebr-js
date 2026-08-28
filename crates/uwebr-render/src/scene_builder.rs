use vello::kurbo::{Affine, Rect, RoundedRect, Stroke};
use vello::peniko::{self, color::palette, Fill};

use crate::scene::{Background, RenderNode, RenderNodeKind, RenderScene, RenderStyle};
use crate::text::TextRenderer;

/// Builds a vello Scene from a RenderScene.
///
/// Owns a [`TextRenderer`] because glyph runs need a live parley
/// `FontContext`; constructing one per frame would re-enumerate the system
/// font collection.
pub struct SceneBuilder {
    text: TextRenderer,
}

impl SceneBuilder {
    pub fn new() -> Self {
        Self {
            text: TextRenderer::new(),
        }
    }

    /// Build a vello::Scene from positioned render nodes.
    pub fn build(&mut self, scene: &RenderScene, width: u32, height: u32) -> vello::Scene {
        let mut vello_scene = vello::Scene::new();

        // Background fill for the entire surface
        vello_scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            palette::css::BLACK,
            None,
            &Rect::new(0.0, 0.0, width as f64, height as f64),
        );

        for node in scene.nodes() {
            self.draw_node(&mut vello_scene, node);
        }

        vello_scene
    }

    /// Convenience wrapper that builds its own text renderer.
    ///
    /// Prefer [`SceneBuilder::build`] on a long-lived instance in render loops.
    pub fn build_scene(scene: &RenderScene, width: u32, height: u32) -> vello::Scene {
        Self::new().build(scene, width, height)
    }

    /// Draw a single node into the vello scene
    fn draw_node(&mut self, scene: &mut vello::Scene, node: &RenderNode) {
        let x = node.layout.x as f64;
        let y = node.layout.y as f64;
        let w = node.layout.width as f64;
        let h = node.layout.height as f64;

        if w <= 0.0 || h <= 0.0 {
            return;
        }

        // Push opacity layer if needed
        if node.style.opacity < 1.0 {
            scene.push_layer(
                Fill::NonZero,
                peniko::Compose::SrcOver,
                node.style.opacity,
                Affine::IDENTITY,
                &Rect::new(x, y, x + w, y + h),
            );
        }

        // `overflow: hidden` clips descendants to this node's box.
        if node.style.overflow_hidden {
            scene.push_clip_layer(
                Fill::NonZero,
                Affine::IDENTITY,
                &Rect::new(x, y, x + w, y + h),
            );
        }

        // Draw based on node kind
        match &node.kind {
            RenderNodeKind::Rect => {
                Self::draw_rect(scene, &node.style, x, y, w, h);
            }
            RenderNodeKind::RoundRect { radius } => {
                Self::draw_round_rect(scene, &node.style, x, y, w, h, *radius as f64);
            }
            RenderNodeKind::Text {
                content,
                font_size,
                color,
                font_family,
            } => {
                self.draw_text(
                    scene,
                    content,
                    *font_size,
                    *color,
                    font_family.as_deref(),
                    x,
                    y,
                    w,
                );
            }
            RenderNodeKind::Image { .. } => {
                Self::draw_rect(scene, &node.style, x, y, w, h);
            }
            RenderNodeKind::Container => {
                if node.style.background.is_some() {
                    if node.style.border_radius > 0.0 {
                        Self::draw_round_rect(
                            scene,
                            &node.style,
                            x,
                            y,
                            w,
                            h,
                            node.style.border_radius as f64,
                        );
                    } else {
                        Self::draw_rect(scene, &node.style, x, y, w, h);
                    }
                }
            }
        }

        // Draw border if present
        if let Some(ref border) = node.style.border {
            Self::draw_border(scene, x, y, w, h, border.width as f64, border.color);
        }

        if node.style.overflow_hidden {
            scene.pop_layer();
        }

        // Pop opacity layer
        if node.style.opacity < 1.0 {
            scene.pop_layer();
        }
    }

    /// Lay out the string with parley and encode its glyph runs into the scene.
    #[allow(clippy::too_many_arguments)]
    fn draw_text(
        &mut self,
        scene: &mut vello::Scene,
        content: &str,
        font_size: f32,
        color: peniko::Color,
        font_family: Option<&str>,
        x: f64,
        y: f64,
        width: f64,
    ) {
        if content.trim().is_empty() {
            return;
        }

        // Wrap to the box the layout engine assigned to this node.
        let max_advance = if width > 0.0 {
            Some(width as f32)
        } else {
            None
        };
        let layout = self
            .text
            .layout_text(content, font_size, font_family, max_advance);

        for line in layout.lines() {
            for item in line.items() {
                let parley::PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };

                let run = glyph_run.run();
                let run_x = x;
                let run_y = y;

                scene
                    .draw_glyphs(run.font())
                    .font_size(run.font_size())
                    .brush(color)
                    .transform(Affine::translate((run_x, run_y)))
                    .normalized_coords(run.normalized_coords())
                    .draw(
                        Fill::NonZero,
                        glyph_run.positioned_glyphs().map(|g| vello::Glyph {
                            id: g.id,
                            x: g.x,
                            y: g.y,
                        }),
                    );
            }
        }
    }

    /// Draw a filled rectangle
    fn draw_rect(scene: &mut vello::Scene, style: &RenderStyle, x: f64, y: f64, w: f64, h: f64) {
        let brush = Self::make_brush(style);
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &brush,
            None,
            &Rect::new(x, y, x + w, y + h),
        );
    }

    /// Draw a filled rounded rectangle
    fn draw_round_rect(
        scene: &mut vello::Scene,
        style: &RenderStyle,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        radius: f64,
    ) {
        let brush = Self::make_brush(style);
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &brush,
            None,
            &RoundedRect::new(x, y, x + w, y + h, radius),
        );
    }

    /// Draw a rectangle border
    fn draw_border(
        scene: &mut vello::Scene,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        width: f64,
        color: peniko::Color,
    ) {
        let stroke = Stroke::new(width);
        let rect = Rect::new(x, y, x + w, y + h);
        scene.stroke(&stroke, Affine::IDENTITY, color, None, &rect);
    }

    /// Create a peniko::Brush from a Background
    pub fn make_brush(style: &RenderStyle) -> peniko::Brush {
        match &style.background {
            Some(Background::Solid(color)) => (*color).into(),
            Some(Background::LinearGradient { start, end, stops }) => {
                let color_stops: Vec<peniko::ColorStop> = stops
                    .iter()
                    .map(|(offset, color)| peniko::ColorStop {
                        offset: *offset,
                        color: (*color).into(),
                    })
                    .collect();
                let gradient = peniko::Gradient::new_linear(
                    (start[0] as f64, start[1] as f64),
                    (end[0] as f64, end[1] as f64),
                )
                .with_stops(color_stops.as_slice());
                peniko::Brush::Gradient(gradient)
            }
            Some(Background::RadialGradient {
                center,
                radius,
                stops,
            }) => {
                let color_stops: Vec<peniko::ColorStop> = stops
                    .iter()
                    .map(|(offset, color)| peniko::ColorStop {
                        offset: *offset,
                        color: (*color).into(),
                    })
                    .collect();
                let gradient =
                    peniko::Gradient::new_radial((center[0] as f64, center[1] as f64), *radius)
                        .with_stops(color_stops.as_slice());
                peniko::Brush::Gradient(gradient)
            }
            None => palette::css::TRANSPARENT.into(),
        }
    }
}

impl Default for SceneBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Background, LayoutInfo};

    /// Number of encoded fills/strokes — a proxy for "something was drawn".
    fn path_count(scene: &vello::Scene) -> usize {
        scene.encoding().n_paths as usize
    }

    /// Number of positioned glyphs encoded into the scene.
    fn glyph_count(scene: &vello::Scene) -> usize {
        scene.encoding().resources.glyphs.len()
    }

    #[test]
    fn test_build_empty_scene() {
        let scene = RenderScene::new();
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        // Only the surface background fill.
        assert_eq!(path_count(&vello_scene), 1);
    }

    #[test]
    fn test_draw_rect() {
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::rect(
            1,
            LayoutInfo::new(10.0, 20.0, 100.0, 50.0),
            palette::css::RED,
        ));
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert_eq!(path_count(&vello_scene), 2, "background + rect");
    }

    #[test]
    fn test_draw_rounded_rect() {
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::round_rect(
            1,
            LayoutInfo::new(10.0, 20.0, 100.0, 50.0),
            palette::css::BLUE,
            8.0,
        ));
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert_eq!(path_count(&vello_scene), 2);
    }

    #[test]
    fn test_draw_with_opacity() {
        let mut scene = RenderScene::new();
        let mut node = RenderNode::rect(
            1,
            LayoutInfo::new(0.0, 0.0, 100.0, 100.0),
            palette::css::GREEN,
        );
        node.style.opacity = 0.5;
        scene.add_node(node);
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert!(path_count(&vello_scene) >= 2);
    }

    #[test]
    fn test_solid_brush() {
        let style = RenderStyle {
            background: Some(Background::Solid(palette::css::RED)),
            ..Default::default()
        };
        let brush = SceneBuilder::make_brush(&style);
        assert!(matches!(brush, peniko::Brush::Solid(_)));
    }

    #[test]
    fn test_gradient_brush() {
        let style = RenderStyle {
            background: Some(Background::LinearGradient {
                start: [0.0, 0.0],
                end: [100.0, 0.0],
                stops: vec![(0.0, palette::css::RED), (1.0, palette::css::BLUE)],
            }),
            ..Default::default()
        };
        let brush = SceneBuilder::make_brush(&style);
        assert!(matches!(brush, peniko::Brush::Gradient(_)));
    }

    #[test]
    fn test_draw_border() {
        let mut scene = RenderScene::new();
        let mut node = RenderNode::rect(
            1,
            LayoutInfo::new(10.0, 10.0, 100.0, 50.0),
            palette::css::WHITE,
        );
        node.style.border = Some(crate::scene::BorderStyle {
            width: 2.0,
            color: palette::css::BLACK,
        });
        scene.add_node(node);
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert_eq!(path_count(&vello_scene), 3, "background + fill + stroke");
    }

    #[test]
    fn test_skip_zero_size_node() {
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::rect(
            1,
            LayoutInfo::new(0.0, 0.0, 0.0, 0.0),
            palette::css::RED,
        ));
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert_eq!(path_count(&vello_scene), 1, "only the background");
    }

    // ── Text rendering (M1) ─────────────────────────────────────

    #[test]
    fn test_text_node_encodes_glyphs() {
        // The old implementation drew a fixed 100px placeholder rect and dropped
        // the string entirely; nothing readable reached the screen.
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::text(
            1,
            LayoutInfo::new(10.0, 10.0, 400.0, 30.0),
            "Hello from uwebr!",
            24.0,
            palette::css::WHITE,
        ));
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);

        if glyph_count(&vello_scene) == 0 {
            // No system fonts (headless CI): parley cannot shape anything.
            // Assert we at least did not emit a bogus placeholder rectangle.
            assert_eq!(path_count(&vello_scene), 1);
        } else {
            assert!(
                glyph_count(&vello_scene) >= 5,
                "expected glyphs for a 17-char string, got {}",
                glyph_count(&vello_scene)
            );
        }
    }

    #[test]
    fn test_empty_text_draws_nothing() {
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::text(
            1,
            LayoutInfo::new(0.0, 0.0, 100.0, 20.0),
            "   ",
            16.0,
            palette::css::WHITE,
        ));
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert_eq!(glyph_count(&vello_scene), 0);
        assert_eq!(path_count(&vello_scene), 1, "no placeholder rect");
    }

    #[test]
    fn test_text_does_not_emit_placeholder_rect() {
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::text(
            1,
            LayoutInfo::new(0.0, 0.0, 300.0, 20.0),
            "abc",
            16.0,
            palette::css::RED,
        ));
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        // Glyphs are encoded as glyph runs, not as extra filled paths.
        assert_eq!(path_count(&vello_scene), 1);
    }

    #[test]
    fn test_background_and_text_both_encoded() {
        let mut scene = RenderScene::new();
        let mut container = RenderNode::container(1, LayoutInfo::new(0.0, 0.0, 800.0, 600.0));
        container.style.background = Some(Background::Solid(peniko::Color::from_rgb8(
            0x1a, 0x1a, 0x2e,
        )));
        scene.add_node(container);
        scene.add_node(RenderNode::text(
            2,
            LayoutInfo::new(10.0, 10.0, 400.0, 30.0),
            "Hello",
            32.0,
            peniko::Color::from_rgb8(0xe0, 0xe0, 0xe0),
        ));

        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert_eq!(
            path_count(&vello_scene),
            2,
            "surface background + container background"
        );
    }

    #[test]
    fn test_container_without_background_draws_nothing() {
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::container(
            1,
            LayoutInfo::new(0.0, 0.0, 10.0, 10.0),
        ));
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert_eq!(path_count(&vello_scene), 1);
    }

    #[test]
    fn test_overflow_hidden_pushes_clip() {
        let mut scene = RenderScene::new();
        let mut node =
            RenderNode::rect(1, LayoutInfo::new(0.0, 0.0, 50.0, 50.0), palette::css::RED);
        node.style.overflow_hidden = true;
        scene.add_node(node);
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert!(
            vello_scene.encoding().n_clips > 0,
            "overflow:hidden must encode a clip layer"
        );
    }

    #[test]
    fn test_container_background_radius_uses_rounded_rect() {
        let mut scene = RenderScene::new();
        let mut node = RenderNode::container(1, LayoutInfo::new(0.0, 0.0, 40.0, 40.0));
        node.style.background = Some(Background::Solid(palette::css::RED));
        node.style.border_radius = 6.0;
        scene.add_node(node);
        let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
        assert_eq!(path_count(&vello_scene), 2);
    }

    #[test]
    fn test_reused_builder_produces_same_result() {
        // The builder is long-lived in the render loop; repeated builds must not
        // accumulate state.
        let mut builder = SceneBuilder::new();
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::rect(
            1,
            LayoutInfo::new(0.0, 0.0, 10.0, 10.0),
            palette::css::RED,
        ));
        let first = path_count(&builder.build(&scene, 800, 600));
        let second = path_count(&builder.build(&scene, 800, 600));
        assert_eq!(first, second);
    }
}
