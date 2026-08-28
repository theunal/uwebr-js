use uwebr_core::component::{Element, NodeType, PropValue};
use uwebr_render::layout::{LayoutEngine, PositionedNode};
use uwebr_render::scene::{Background, RenderNode, RenderNodeKind, RenderScene, RenderStyle};
use uwebr_render::scene_builder::SceneBuilder;
use vello::peniko;

/// Full render pipeline: Element → Layout → Scene → vello Scene
pub struct RenderPipeline {
    layout_engine: LayoutEngine,
    render_scene: RenderScene,
}

impl RenderPipeline {
    pub fn new() -> Self {
        Self {
            layout_engine: LayoutEngine::new(),
            render_scene: RenderScene::new(),
        }
    }

    /// Full pipeline: Element → positioned nodes → RenderScene → vello Scene
    pub fn render(&mut self, element: &Element, width: u32, height: u32) -> vello::Scene {
        self.layout_engine.reset();
        self.render_scene.clear();

        let root = match self.layout_engine.build_tree(element) {
            Ok(r) => r,
            Err(_) => return vello::Scene::new(),
        };

        if self.layout_engine.compute(root, width as f32, height as f32).is_err() {
            return vello::Scene::new();
        }

        let positioned = self.layout_engine.collect_positioned_nodes(root, element);

        for pos_node in &positioned {
            if let Some(render_node) = positioned_to_render_node(pos_node) {
                self.render_scene.add_node(render_node);
            }
        }

        SceneBuilder::build_scene(&self.render_scene, width, height)
    }
}

impl Default for RenderPipeline {
    fn default() -> Self {
        Self::new()
    }
}

fn positioned_to_render_node(pos: &PositionedNode) -> Option<RenderNode> {
    let layout = pos.layout;
    if layout.width <= 0.0 || layout.height <= 0.0 {
        return None;
    }
    let id = u64::from(pos.taffy_node);

    match &pos.element.node_type {
        NodeType::Text(content) => {
            let (font_size, color) = extract_text_style(&pos.element.props);
            Some(RenderNode::text(id, layout, content, font_size, color))
        }
        NodeType::Element(_tag) => {
            let style = extract_render_style(&pos.element.props);
            let kind = RenderNodeKind::Container;
            Some(RenderNode { id, kind, layout, style })
        }
        NodeType::Component(_) => Some(RenderNode::container(id, layout)),
        NodeType::Raw(_) => None,
    }
}

fn extract_render_style(props: &[(String, PropValue)]) -> RenderStyle {
    let mut style = RenderStyle::default();
    for (name, value) in props {
        match name.as_str() {
            "background" | "bg" => {
                if let PropValue::String(s) = value {
                    style.background = Some(Background::Solid(parse_simple_color(s)));
                }
            }
            "opacity" => {
                if let PropValue::Number(n) = value {
                    style.opacity = (*n as f32).clamp(0.0, 1.0);
                }
            }
            "border_width" | "border" => {
                if let PropValue::Number(n) = value {
                    let color = extract_prop_color(props, "border_color");
                    style.border = Some(uwebr_render::scene::BorderStyle {
                        width: *n as f32,
                        color,
                    });
                }
            }
            "border_radius" | "rounded" => {
                if let PropValue::Number(n) = value {
                    style.border_radius = *n as f32;
                }
            }
            _ => {}
        }
    }
    style
}

fn extract_text_style(props: &[(String, PropValue)]) -> (f32, peniko::Color) {
    let mut font_size = 16.0;
    let mut color = peniko::color::palette::css::WHITE;
    for (name, value) in props {
        match name.as_str() {
            "font_size" | "font-size" => {
                if let PropValue::Number(n) = value { font_size = *n as f32; }
            }
            "color" | "text_color" => {
                if let PropValue::String(s) = value { color = parse_simple_color(s); }
            }
            _ => {}
        }
    }
    (font_size, color)
}

#[allow(dead_code)]
fn extract_border_radius(props: &[(String, PropValue)]) -> f32 {
    for (name, value) in props {
        if name == "border_radius" || name == "rounded" {
            if let PropValue::Number(n) = value { return *n as f32; }
        }
    }
    0.0
}

fn extract_prop_color(props: &[(String, PropValue)], key: &str) -> peniko::Color {
    for (name, value) in props {
        if name == key {
            if let PropValue::String(s) = value { return parse_simple_color(s); }
        }
    }
    peniko::color::palette::css::WHITE
}

fn parse_simple_color(s: &str) -> peniko::Color {
    use peniko::color::palette::css;
    match s.to_lowercase().as_str() {
        "red" => css::RED,
        "blue" => css::BLUE,
        "green" => css::GREEN,
        "white" => css::WHITE,
        "black" => css::BLACK,
        "yellow" => css::YELLOW,
        "orange" => css::ORANGE,
        "purple" => css::PURPLE,
        "transparent" => css::TRANSPARENT,
        _ => parse_hex_color(s).unwrap_or(css::WHITE),
    }
}

fn parse_hex_color(s: &str) -> Option<peniko::Color> {
    let hex = s.trim_start_matches('#');
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
            Some(peniko::Color::from_rgb8(r * 17, g * 17, b * 17))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(peniko::Color::from_rgb8(r, g, b))
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            Some(peniko::Color::from_rgba8(r, g, b, a))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_text(content: &str) -> Element {
        Element { node_type: NodeType::Text(content.to_string()), props: vec![], children: vec![] }
    }

    fn make_div(children: Vec<Element>) -> Element {
        Element { node_type: NodeType::Element("div".to_string()), props: vec![], children }
    }

    fn make_div_with_props(props: Vec<(String, PropValue)>, children: Vec<Element>) -> Element {
        Element { node_type: NodeType::Element("div".to_string()), props, children }
    }

    #[test]
    fn test_pipeline_empty() {
        let mut pipeline = RenderPipeline::new();
        let el = make_div(vec![]);
        let _scene = pipeline.render(&el, 800, 600);
    }

    #[test]
    fn test_pipeline_text_only() {
        let mut pipeline = RenderPipeline::new();
        let el = make_text("Hello");
        let _scene = pipeline.render(&el, 800, 600);
    }

    #[test]
    fn test_pipeline_div_with_text() {
        let mut pipeline = RenderPipeline::new();
        let el = make_div(vec![make_text("Hello")]);
        let _scene = pipeline.render(&el, 800, 600);
    }

    #[test]
    fn test_pipeline_nested_divs() {
        let mut pipeline = RenderPipeline::new();
        let inner = make_div(vec![make_text("Inner")]);
        let outer = make_div(vec![inner, make_text("Outer")]);
        let _scene = pipeline.render(&outer, 800, 600);
    }

    #[test]
    fn test_pipeline_with_background() {
        let mut pipeline = RenderPipeline::new();
        let el = make_div_with_props(
            vec![("bg".into(), PropValue::String("red".into()))],
            vec![make_text("Red box")],
        );
        let _scene = pipeline.render(&el, 800, 600);
    }

    #[test]
    fn test_pipeline_with_opacity() {
        let mut pipeline = RenderPipeline::new();
        let el = make_div_with_props(
            vec![("opacity".into(), PropValue::Number(0.5))],
            vec![make_text("Half transparent")],
        );
        let _scene = pipeline.render(&el, 800, 600);
    }

    #[test]
    fn test_pipeline_with_size() {
        let mut pipeline = RenderPipeline::new();
        let el = make_div_with_props(
            vec![("width".into(), PropValue::Number(200.0)), ("height".into(), PropValue::Number(100.0))],
            vec![],
        );
        let _scene = pipeline.render(&el, 800, 600);
    }

    #[test]
    fn test_pipeline_reset_reuse() {
        let mut pipeline = RenderPipeline::new();
        let _ = pipeline.render(&make_text("First"), 800, 600);
        let _ = pipeline.render(&make_text("Second"), 800, 600);
    }

    #[test]
    fn test_positioned_to_render_node_text() {
        let pos = PositionedNode {
            taffy_node: taffy::NodeId::new(0),
            element: make_text("Hi"),
            layout: uwebr_render::scene::LayoutInfo::new(10.0, 20.0, 100.0, 30.0),
            depth: 0,
        };
        let node = positioned_to_render_node(&pos).unwrap();
        assert!(matches!(node.kind, RenderNodeKind::Text { .. }));
    }

    #[test]
    fn test_positioned_to_render_node_div() {
        let pos = PositionedNode {
            taffy_node: taffy::NodeId::new(0),
            element: make_div(vec![]),
            layout: uwebr_render::scene::LayoutInfo::new(0.0, 0.0, 800.0, 600.0),
            depth: 0,
        };
        let node = positioned_to_render_node(&pos).unwrap();
        assert!(matches!(node.kind, RenderNodeKind::Container));
    }

    #[test]
    fn test_positioned_zero_size_returns_none() {
        let pos = PositionedNode {
            taffy_node: taffy::NodeId::new(0),
            element: make_div(vec![]),
            layout: uwebr_render::scene::LayoutInfo::new(0.0, 0.0, 0.0, 0.0),
            depth: 0,
        };
        assert!(positioned_to_render_node(&pos).is_none());
    }

    #[test]
    fn test_parse_hex_color_3() {
        let c = parse_hex_color("#f00").unwrap();
        assert_eq!(c, peniko::color::palette::css::RED);
    }

    #[test]
    fn test_parse_hex_color_6() {
        let c = parse_hex_color("#00ff00").unwrap();
        assert_eq!(c, peniko::Color::from_rgb8(0, 255, 0));
    }

    #[test]
    fn test_parse_hex_color_8() {
        let c = parse_hex_color("#0000ff80").unwrap();
        assert_eq!(c, peniko::Color::from_rgba8(0, 0, 255, 128));
    }

    #[test]
    fn test_parse_hex_color_invalid() {
        assert!(parse_hex_color("not_a_color").is_none());
    }

    #[test]
    fn test_parse_simple_color_named() {
        assert_eq!(parse_simple_color("red"), peniko::color::palette::css::RED);
        assert_eq!(parse_simple_color("BLUE"), peniko::color::palette::css::BLUE);
    }

    #[test]
    fn test_parse_simple_color_hex() {
        let c = parse_simple_color("#ff00ff");
        assert_eq!(c, peniko::Color::from_rgb8(255, 0, 255));
    }

    #[test]
    fn test_parse_simple_color_unknown() {
        let c = parse_simple_color("mauve");
        assert_eq!(c, peniko::color::palette::css::WHITE);
    }
}
