use vello::peniko;

// ── Layout Info ────────────────────────────────────────────────────────

/// Position + size computed by taffy
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutInfo {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl LayoutInfo {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        }
    }
}

// ── Background ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Background {
    Solid(peniko::Color),
    LinearGradient {
        start: [f32; 2],
        end: [f32; 2],
        stops: Vec<(f32, peniko::Color)>,
    },
    RadialGradient {
        center: [f32; 2],
        radius: f32,
        stops: Vec<(f32, peniko::Color)>,
    },
}

// ── Border ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BorderStyle {
    pub width: f32,
    pub color: peniko::Color,
}

// ── Text Overflow ──────────────────────────────────────────────────────

/// How overflowing inline text is treated inside its box.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum TextOverflow {
    /// Clip at the box edge with no marker (CSS default).
    #[default]
    Clip,
    /// Clip and append an ellipsis ("…") to the last visible run.
    Ellipsis,
    /// Do not clip; let text spill out of the box.
    Visible,
}

// ── Render Style ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RenderStyle {
    pub background: Option<Background>,
    pub border: Option<BorderStyle>,
    pub border_radius: f32,
    pub opacity: f32,
    pub overflow_hidden: bool,
    pub text_overflow: TextOverflow,
}

impl Default for RenderStyle {
    fn default() -> Self {
        Self {
            background: None,
            border: None,
            border_radius: 0.0,
            opacity: 1.0,
            overflow_hidden: false,
            text_overflow: TextOverflow::default(),
        }
    }
}

// ── Render Node Kind ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum RenderNodeKind {
    Rect,
    RoundRect {
        radius: f32,
    },
    Text {
        content: String,
        font_size: f32,
        color: peniko::Color,
        /// CSS `font-family` list, passed through to parley.
        font_family: Option<String>,
    },
    Image {
        data: Vec<u8>,
        width: u32,
        height: u32,
    },
    Container,
}

// ── Render Node ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RenderNode {
    pub id: u64,
    pub kind: RenderNodeKind,
    pub layout: LayoutInfo,
    pub style: RenderStyle,
}

impl RenderNode {
    pub fn rect(id: u64, layout: LayoutInfo, background: peniko::Color) -> Self {
        Self {
            id,
            kind: RenderNodeKind::Rect,
            layout,
            style: RenderStyle {
                background: Some(Background::Solid(background)),
                ..Default::default()
            },
        }
    }

    pub fn round_rect(id: u64, layout: LayoutInfo, background: peniko::Color, radius: f32) -> Self {
        Self {
            id,
            kind: RenderNodeKind::RoundRect { radius },
            layout,
            style: RenderStyle {
                background: Some(Background::Solid(background)),
                border_radius: radius,
                ..Default::default()
            },
        }
    }

    pub fn text(
        id: u64,
        layout: LayoutInfo,
        content: &str,
        font_size: f32,
        color: peniko::Color,
    ) -> Self {
        Self::text_with_family(id, layout, content, font_size, color, None)
    }

    pub fn text_with_family(
        id: u64,
        layout: LayoutInfo,
        content: &str,
        font_size: f32,
        color: peniko::Color,
        font_family: Option<String>,
    ) -> Self {
        Self {
            id,
            kind: RenderNodeKind::Text {
                content: content.to_string(),
                font_size,
                color,
                font_family,
            },
            layout,
            style: RenderStyle::default(),
        }
    }

    pub fn container(id: u64, layout: LayoutInfo) -> Self {
        Self {
            id,
            kind: RenderNodeKind::Container,
            layout,
            style: RenderStyle::default(),
        }
    }

    pub fn image(id: u64, layout: LayoutInfo, data: Vec<u8>, width: u32, height: u32) -> Self {
        Self {
            id,
            kind: RenderNodeKind::Image {
                data,
                width,
                height,
            },
            layout,
            style: RenderStyle::default(),
        }
    }
}

// ── Render Scene ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RenderScene {
    nodes: Vec<RenderNode>,
}

impl RenderScene {
    pub fn new() -> Self {
        Self { nodes: vec![] }
    }

    pub fn add_node(&mut self, node: RenderNode) {
        self.nodes.push(node);
    }

    pub fn nodes(&self) -> &[RenderNode] {
        &self.nodes
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn clear(&mut self) {
        self.nodes.clear();
    }
}

impl Default for RenderScene {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use vello::peniko::color::palette;

    #[test]
    fn test_render_scene_add_node() {
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::rect(
            1,
            LayoutInfo::new(0.0, 0.0, 100.0, 50.0),
            palette::css::RED,
        ));
        assert_eq!(scene.node_count(), 1);
    }

    #[test]
    fn test_render_style_defaults() {
        let style = RenderStyle::default();
        assert!(style.background.is_none());
        assert!(style.border.is_none());
        assert_eq!(style.border_radius, 0.0);
        assert_eq!(style.opacity, 1.0);
        assert!(!style.overflow_hidden);
        assert_eq!(style.text_overflow, TextOverflow::Clip);
    }

    #[test]
    fn test_background_solid() {
        let bg = Background::Solid(palette::css::BLUE);
        assert!(matches!(bg, Background::Solid(_)));
    }

    #[test]
    fn test_background_gradient() {
        let bg = Background::LinearGradient {
            start: [0.0, 0.0],
            end: [100.0, 100.0],
            stops: vec![(0.0, palette::css::RED), (1.0, palette::css::BLUE)],
        };
        match bg {
            Background::LinearGradient { start, end, stops } => {
                assert_eq!(start, [0.0, 0.0]);
                assert_eq!(end, [100.0, 100.0]);
                assert_eq!(stops.len(), 2);
            }
            _ => panic!("Expected LinearGradient"),
        }
    }

    #[test]
    fn test_render_node_rect() {
        let node = RenderNode::rect(
            1,
            LayoutInfo::new(10.0, 20.0, 100.0, 50.0),
            palette::css::RED,
        );
        assert_eq!(node.id, 1);
        assert_eq!(node.layout.width, 100.0);
        assert!(matches!(node.kind, RenderNodeKind::Rect));
        assert!(node.style.background.is_some());
    }

    #[test]
    fn test_render_node_text() {
        let node = RenderNode::text(
            2,
            LayoutInfo::new(0.0, 0.0, 200.0, 30.0),
            "Hello",
            16.0,
            palette::css::WHITE,
        );
        assert_eq!(node.id, 2);
        match &node.kind {
            RenderNodeKind::Text {
                content,
                font_size,
                color: _,
                font_family,
            } => {
                assert_eq!(content, "Hello");
                assert_eq!(*font_size, 16.0);
                assert!(font_family.is_none());
            }
            _ => panic!("Expected Text"),
        }
    }

    #[test]
    fn test_render_node_text_with_family() {
        let node = RenderNode::text_with_family(
            3,
            LayoutInfo::new(0.0, 0.0, 100.0, 20.0),
            "Hi",
            14.0,
            palette::css::WHITE,
            Some("monospace".to_string()),
        );
        match &node.kind {
            RenderNodeKind::Text { font_family, .. } => {
                assert_eq!(font_family.as_deref(), Some("monospace"));
            }
            _ => panic!("Expected Text"),
        }
    }

    #[test]
    fn test_render_scene_clear() {
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::rect(1, LayoutInfo::zero(), palette::css::RED));
        scene.add_node(RenderNode::rect(2, LayoutInfo::zero(), palette::css::BLUE));
        assert_eq!(scene.node_count(), 2);
        scene.clear();
        assert_eq!(scene.node_count(), 0);
    }

    #[test]
    fn test_text_overflow_default_is_clip() {
        assert_eq!(TextOverflow::default(), TextOverflow::Clip);
    }

    #[test]
    fn test_render_node_image_helper() {
        let node = RenderNode::image(
            9,
            LayoutInfo::new(0.0, 0.0, 64.0, 64.0),
            vec![1, 2, 3],
            32,
            32,
        );
        match &node.kind {
            RenderNodeKind::Image {
                data,
                width,
                height,
            } => {
                assert_eq!(data, &vec![1, 2, 3]);
                assert_eq!(*width, 32);
                assert_eq!(*height, 32);
            }
            _ => panic!("Expected Image"),
        }
    }

    // ── Scene edge-case tests ───────────────────────────────────

    #[test]
    fn render_layout_info_new_values() {
        let info = LayoutInfo::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(info.x, 1.0);
        assert_eq!(info.y, 2.0);
        assert_eq!(info.width, 3.0);
        assert_eq!(info.height, 4.0);
    }

    #[test]
    fn render_layout_info_zero() {
        let info = LayoutInfo::zero();
        assert_eq!(info.x, 0.0);
        assert_eq!(info.y, 0.0);
        assert_eq!(info.width, 0.0);
        assert_eq!(info.height, 0.0);
    }

    #[test]
    fn render_layout_info_partial_eq() {
        let a = LayoutInfo::new(1.0, 2.0, 3.0, 4.0);
        let b = LayoutInfo::new(1.0, 2.0, 3.0, 4.0);
        let c = LayoutInfo::new(1.0, 2.0, 3.0, 5.0);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn render_render_scene_multiple_add() {
        let mut scene = RenderScene::new();
        scene.add_node(RenderNode::rect(
            1,
            LayoutInfo::zero(),
            vello::peniko::color::palette::css::RED,
        ));
        scene.add_node(RenderNode::rect(
            2,
            LayoutInfo::zero(),
            vello::peniko::color::palette::css::BLUE,
        ));
        scene.add_node(RenderNode::rect(
            3,
            LayoutInfo::zero(),
            vello::peniko::color::palette::css::GREEN,
        ));
        assert_eq!(scene.node_count(), 3);
        assert_eq!(scene.nodes()[0].id, 1);
        assert_eq!(scene.nodes()[2].id, 3);
    }

    #[test]
    fn render_render_node_round_rect_helpers() {
        let node = RenderNode::round_rect(
            10,
            LayoutInfo::new(5.0, 10.0, 50.0, 30.0),
            vello::peniko::color::palette::css::CYAN,
            8.0,
        );
        assert_eq!(node.id, 10);
        assert_eq!(node.style.border_radius, 8.0);
        assert!(node.style.background.is_some());
        assert!(matches!(node.kind, RenderNodeKind::RoundRect { radius } if radius == 8.0));
    }

    #[test]
    fn render_render_node_container_default_style() {
        let node = RenderNode::container(42, LayoutInfo::new(0.0, 0.0, 100.0, 50.0));
        assert!(node.style.background.is_none());
        assert_eq!(node.style.opacity, 1.0);
        assert!(!node.style.overflow_hidden);
        assert!(matches!(node.kind, RenderNodeKind::Container));
    }

    #[test]
    fn render_background_gradient_clone() {
        let bg = Background::LinearGradient {
            start: [0.0, 0.0],
            end: [1.0, 1.0],
            stops: vec![
                (0.0, vello::peniko::color::palette::css::RED),
                (1.0, vello::peniko::color::palette::css::BLUE),
            ],
        };
        let cloned = bg.clone();
        assert_eq!(bg, cloned);
    }

    #[test]
    fn render_background_radial_gradient_eq() {
        let a = Background::RadialGradient {
            center: [0.5, 0.5],
            radius: 0.5,
            stops: vec![],
        };
        let b = Background::RadialGradient {
            center: [0.5, 0.5],
            radius: 0.5,
            stops: vec![],
        };
        assert_eq!(a, b);
    }

    #[test]
    fn render_text_overflow_variants() {
        assert_ne!(TextOverflow::Clip, TextOverflow::Ellipsis);
        assert_ne!(TextOverflow::Ellipsis, TextOverflow::Visible);
        assert_ne!(TextOverflow::Clip, TextOverflow::Visible);
    }

    #[test]
    fn render_border_style_fields() {
        let bs = crate::scene::BorderStyle {
            width: 2.5,
            color: vello::peniko::color::palette::css::BLACK,
        };
        assert_eq!(bs.width, 2.5);
    }
}
