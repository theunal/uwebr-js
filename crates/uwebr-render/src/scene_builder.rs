use vello::kurbo::{Affine, Rect, RoundedRect, Stroke};
use vello::peniko::{self, Fill, color::palette};

use crate::scene::{Background, RenderNode, RenderNodeKind, RenderScene, RenderStyle};

/// Builds a vello Scene from a RenderScene
pub struct SceneBuilder;

impl SceneBuilder {
    /// Build a vello::Scene from positioned render nodes
    pub fn build_scene(scene: &RenderScene, width: u32, height: u32) -> vello::Scene {
        let mut vello_scene = vello::Scene::new();

        // Background fill for the entire surface
        vello_scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            palette::css::BLACK,
            None,
            &Rect::new(0.0, 0.0, width as f64, height as f64),
        );

        // Draw each node
        for node in scene.nodes() {
            Self::draw_node(&mut vello_scene, node);
        }

        vello_scene
    }

    /// Draw a single node into the vello scene
    fn draw_node(scene: &mut vello::Scene, node: &RenderNode) {
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

        // Draw based on node kind
        match &node.kind {
            RenderNodeKind::Rect => {
                Self::draw_rect(scene, &node.style, x, y, w, h);
            }
            RenderNodeKind::RoundRect { radius } => {
                Self::draw_round_rect(scene, &node.style, x, y, w, h, *radius as f64);
            }
            RenderNodeKind::Text { content: _, font_size, color } => {
                // Text placeholder — real text rendering via text.rs
                scene.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    *color,
                    None,
                    &Rect::new(x, y, x + 100.0, y + (*font_size as f64).max(h)),
                );
            }
            RenderNodeKind::Image { .. } => {
                Self::draw_rect(scene, &node.style, x, y, w, h);
            }
            RenderNodeKind::Container => {
                if node.style.background.is_some() {
                    Self::draw_rect(scene, &node.style, x, y, w, h);
                }
            }
        }

        // Draw border if present
        if let Some(ref border) = node.style.border {
            Self::draw_border(scene, x, y, w, h, border.width as f64, border.color);
        }

        // Pop opacity layer
        if node.style.opacity < 1.0 {
            scene.pop_layer();
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
    fn draw_round_rect(scene: &mut vello::Scene, style: &RenderStyle, x: f64, y: f64, w: f64, h: f64, radius: f64) {
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
    fn draw_border(scene: &mut vello::Scene, x: f64, y: f64, w: f64, h: f64, width: f64, color: peniko::Color) {
        let stroke = Stroke::new(width);
        let rect = Rect::new(x, y, x + w, y + h);
        scene.stroke(
            &stroke,
            Affine::IDENTITY,
            color,
            None,
            &rect,
        );
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
            Some(Background::RadialGradient { center, radius, stops }) => {
                let color_stops: Vec<peniko::ColorStop> = stops
                    .iter()
                    .map(|(offset, color)| peniko::ColorStop {
                        offset: *offset,
                        color: (*color).into(),
                    })
                    .collect();
                let gradient = peniko::Gradient::new_radial(
                    (center[0] as f64, center[1] as f64),
                    *radius,
                )
                .with_stops(color_stops.as_slice());
                peniko::Brush::Gradient(gradient)
            }
            None => palette::css::TRANSPARENT.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Background, LayoutInfo};

    #[test]
    fn test_build_empty_scene() {
        let scene = RenderScene::new();
        let _vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
    }

    #[test]
    fn test_draw_rect() {
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::rect(1, LayoutInfo::new(10.0, 20.0, 100.0, 50.0), palette::css::RED));
        let _vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
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
        let _vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
    }

    #[test]
    fn test_draw_with_opacity() {
        let mut scene = RenderScene::new();
        let mut node = RenderNode::rect(1, LayoutInfo::new(0.0, 0.0, 100.0, 100.0), palette::css::GREEN);
        node.style.opacity = 0.5;
        scene.add_node(node);
        let _vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
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
                stops: vec![
                    (0.0, palette::css::RED),
                    (1.0, palette::css::BLUE),
                ],
            }),
            ..Default::default()
        };
        let brush = SceneBuilder::make_brush(&style);
        assert!(matches!(brush, peniko::Brush::Gradient(_)));
    }

    #[test]
    fn test_draw_border() {
        let mut scene = RenderScene::new();
        let mut node = RenderNode::rect(1, LayoutInfo::new(10.0, 10.0, 100.0, 50.0), palette::css::WHITE);
        node.style.border = Some(crate::scene::BorderStyle {
            width: 2.0,
            color: palette::css::BLACK,
        });
        scene.add_node(node);
        let _vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
    }

    #[test]
    fn test_skip_zero_size_node() {
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::rect(1, LayoutInfo::new(0.0, 0.0, 0.0, 0.0), palette::css::RED));
        let _vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
    }
}
