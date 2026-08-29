use uwebr_core::component::{Element, NodeType, PropValue};
use uwebr_render::layout::{LayoutEngine, PositionedNode};
use uwebr_render::paint::ResolvedPaint;
use uwebr_render::scene::{LayoutInfo, RenderNode, RenderNodeKind, RenderScene, RenderStyle};
use uwebr_render::scene_builder::SceneBuilder;
use uwebr_render::stylebook::StyleBook;

/// A clickable region discovered during layout.
///
/// `on:click={increment}` becomes a `PropValue::Closure("increment")` prop; the
/// name is resolved against the action registry when a click lands here.
#[derive(Debug, Clone, PartialEq)]
pub struct HitTarget {
    pub action: String,
    pub bounds: LayoutInfo,
    /// Tree depth — deeper nodes win, matching DOM event targeting.
    pub depth: usize,
}

/// Full render pipeline: Element → Layout → Scene → vello Scene
pub struct RenderPipeline {
    layout_engine: LayoutEngine,
    render_scene: RenderScene,
    stylebook: StyleBook,
    scene_builder: SceneBuilder,
    hit_targets: Vec<HitTarget>,
    /// Raw CSS kept so `vw`/`vh` can be re-resolved when the viewport changes.
    css_string: Option<String>,
}

impl RenderPipeline {
    pub fn new() -> Self {
        Self {
            layout_engine: LayoutEngine::new(),
            render_scene: RenderScene::new(),
            stylebook: StyleBook::empty(),
            // Reused across frames: building one enumerates the system fonts.
            scene_builder: SceneBuilder::new(),
            hit_targets: Vec::new(),
            css_string: None,
        }
    }

    /// Load CSS rules into the pipeline
    pub fn with_css(mut self, css: &str) -> Self {
        if let Ok(sb) = StyleBook::parse(css) {
            self.stylebook = sb;
        }
        self.css_string = Some(css.to_string());
        self
    }

    /// Set the stylebook directly
    pub fn with_stylebook(mut self, stylebook: StyleBook) -> Self {
        self.stylebook = stylebook;
        self
    }

    /// Keep the raw CSS so `vw`/`vh` are re-resolved when the viewport changes.
    pub fn with_css_source(mut self, css: &str) -> Self {
        self.css_string = Some(css.to_string());
        self
    }

    /// Full pipeline: Element → positioned nodes → RenderScene → vello Scene
    pub fn render(&mut self, element: &Element, width: u32, height: u32) -> vello::Scene {
        self.build_render_scene(element, width, height);
        let (w, h) = (width, height);
        self.scene_builder.build(&self.render_scene, w, h)
    }

    /// Run layout and populate the intermediate `RenderScene` (without encoding).
    ///
    /// Exposed so tests can assert on the node list rather than on opaque
    /// vello encoding output.
    pub fn build_render_scene(&mut self, element: &Element, width: u32, height: u32) {
        self.layout_engine.reset();
        self.render_scene.clear();
        self.hit_targets.clear();

        // Re-resolve `vw`/`vh` against the current viewport before layout.
        if let Some(ref css) = self.css_string {
            let _ = self.stylebook.reparse(css, width as f32, height as f32);
        }

        let Ok(root) = self.layout_engine.build_tree(element, &self.stylebook) else {
            return;
        };

        if self
            .layout_engine
            .compute(root, width as f32, height as f32)
            .is_err()
        {
            return;
        }

        let positioned =
            self.layout_engine
                .collect_positioned_nodes(root, element, &self.stylebook);

        for pos_node in &positioned {
            if let Some(action) = click_action(&pos_node.element.props) {
                self.hit_targets.push(HitTarget {
                    action,
                    bounds: pos_node.layout,
                    depth: pos_node.depth,
                });
            }
            if let Some(render_node) = positioned_to_render_node(pos_node) {
                self.render_scene.add_node(render_node);
            }
        }
    }

    /// Access the intermediate render scene (post-layout, pre-encoding).
    pub fn render_scene(&self) -> &RenderScene {
        &self.render_scene
    }

    /// Clickable regions from the last layout pass.
    pub fn hit_targets(&self) -> &[HitTarget] {
        &self.hit_targets
    }

    /// Find the action registered at a point, innermost target first.
    pub fn hit_test(&self, x: f32, y: f32) -> Option<&str> {
        self.hit_targets
            .iter()
            .filter(|t| contains_point(&t.bounds, x, y))
            .max_by_key(|t| t.depth)
            .map(|t| t.action.as_str())
    }
}

fn contains_point(bounds: &LayoutInfo, x: f32, y: f32) -> bool {
    x >= bounds.x && y >= bounds.y && x < bounds.x + bounds.width && y < bounds.y + bounds.height
}

/// Extract the click handler name from an element's props.
fn click_action(props: &[(String, PropValue)]) -> Option<String> {
    props.iter().find_map(|(name, value)| {
        if name != "on:click" {
            return None;
        }
        match value {
            PropValue::Closure(action) => Some(action.clone()),
            _ => None,
        }
    })
}

impl Default for RenderPipeline {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert one positioned node into a drawable render node.
///
/// Zero-area nodes are skipped, but text is checked on content rather than on
/// box size: a text leaf can legitimately report a 0 width when no system font
/// resolved, and dropping it there is what previously made all text invisible.
fn positioned_to_render_node(pos: &PositionedNode) -> Option<RenderNode> {
    let layout = pos.layout;
    let id = u64::from(pos.taffy_node);

    match &pos.element.node_type {
        NodeType::Text(content) => {
            if content.trim().is_empty() {
                return None;
            }
            Some(RenderNode::text_with_family(
                id,
                layout,
                content,
                pos.paint.font_size,
                pos.paint.color,
                pos.paint.font_family.clone(),
            ))
        }
        NodeType::Element(tag) => {
            if tag == "img" {
                return img_to_render_node(pos, id);
            }
            if layout.width <= 0.0 || layout.height <= 0.0 {
                return None;
            }
            Some(RenderNode {
                id,
                kind: RenderNodeKind::Container,
                layout,
                style: paint_to_render_style(&pos.paint, pos.overflow_hidden),
            })
        }
        NodeType::Component(_) => {
            if layout.width <= 0.0 || layout.height <= 0.0 {
                return None;
            }
            Some(RenderNode {
                id,
                kind: RenderNodeKind::Container,
                layout,
                style: paint_to_render_style(&pos.paint, pos.overflow_hidden),
            })
        }
        NodeType::Raw(html) => {
            if let Some(el) = uwebr_render::html_parse::parse_runtime_html(html) {
                return raw_element_to_render_node(&el, id, layout, &pos.paint);
            }
            // Fallback: show the raw markup as literal text rather than dropping it.
            if html.trim().is_empty() {
                return None;
            }
            Some(RenderNode::text_with_family(
                id,
                layout,
                html,
                pos.paint.font_size,
                pos.paint.color,
                pos.paint.font_family.clone(),
            ))
        }
    }
}

/// Build an image render node from an `<img>` element's props.
///
/// FAZ 11 accepts the raw image bytes through the `src` string prop; `width`
/// and `height` props are advisory hints carried alongside the decoded data.
fn img_to_render_node(pos: &PositionedNode, id: u64) -> Option<RenderNode> {
    if pos.layout.width <= 0.0 || pos.layout.height <= 0.0 {
        return None;
    }

    let data = pos
        .element
        .props
        .iter()
        .find(|(k, _)| k == "src")
        .and_then(|(_, v)| match v {
            PropValue::String(s) => Some(s.as_bytes().to_vec()),
            _ => None,
        })
        .unwrap_or_default();

    let dim = |key: &str| -> u32 {
        pos.element
            .props
            .iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, v)| match v {
                PropValue::Number(n) => Some(*n as u32),
                PropValue::String(s) => s.trim().parse::<u32>().ok(),
                _ => None,
            })
            .unwrap_or(0)
    };

    Some(RenderNode {
        id,
        kind: RenderNodeKind::Image {
            data,
            width: dim("width"),
            height: dim("height"),
        },
        layout: pos.layout,
        style: paint_to_render_style(&pos.paint, pos.overflow_hidden),
    })
}

/// Convert a parsed `{@html}` element subtree into a single render node.
///
/// The runtime parser produces a lightweight tree; here we surface its first
/// meaningful content (text) into the box laid out for the `Raw` node.
fn raw_element_to_render_node(
    element: &uwebr_core::component::Element,
    id: u64,
    layout: LayoutInfo,
    paint: &ResolvedPaint,
) -> Option<RenderNode> {
    if let Some(text) = first_text(element) {
        if text.trim().is_empty() {
            return None;
        }
        return Some(RenderNode::text_with_family(
            id,
            layout,
            &text,
            paint.font_size,
            paint.color,
            paint.font_family.clone(),
        ));
    }

    if layout.width <= 0.0 || layout.height <= 0.0 {
        return None;
    }
    Some(RenderNode {
        id,
        kind: RenderNodeKind::Container,
        layout,
        style: paint_to_render_style(paint, false),
    })
}

/// Depth-first search for the first text content in a parsed element tree.
fn first_text(element: &uwebr_core::component::Element) -> Option<String> {
    if let NodeType::Text(t) = &element.node_type {
        return Some(t.clone());
    }
    for child in &element.children {
        if let Some(t) = first_text(child) {
            return Some(t);
        }
    }
    None
}

/// Translate resolved paint into the scene's style representation.
fn paint_to_render_style(paint: &ResolvedPaint, overflow_hidden: bool) -> RenderStyle {
    RenderStyle {
        background: paint.background.clone(),
        border: if paint.border_width > 0.0 {
            Some(uwebr_render::scene::BorderStyle {
                width: paint.border_width,
                color: paint.border_color,
            })
        } else {
            None
        },
        border_radius: paint.border_radius,
        opacity: paint.opacity,
        overflow_hidden,
        text_overflow: paint.text_overflow.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uwebr_core::component::PropValue;
    use uwebr_render::scene::Background;
    use vello::peniko;

    fn make_text(content: &str) -> Element {
        Element {
            node_type: NodeType::Text(content.to_string()),
            props: vec![],
            children: vec![],
        }
    }

    fn make_div(children: Vec<Element>) -> Element {
        Element {
            node_type: NodeType::Element("div".to_string()),
            props: vec![],
            children,
        }
    }

    fn make_div_with_props(props: Vec<(String, PropValue)>, children: Vec<Element>) -> Element {
        Element {
            node_type: NodeType::Element("div".to_string()),
            props,
            children,
        }
    }

    fn make_el(tag: &str, props: Vec<(String, PropValue)>, children: Vec<Element>) -> Element {
        Element {
            node_type: NodeType::Element(tag.to_string()),
            props,
            children,
        }
    }

    /// Number of glyphs encoded into a vello scene.
    fn glyph_count(scene: &vello::Scene) -> usize {
        scene.encoding().resources.glyphs.len()
    }

    /// Number of filled/stroked paths encoded into a vello scene.
    fn path_count(scene: &vello::Scene) -> usize {
        scene.encoding().n_paths as usize
    }

    fn text_nodes(scene: &RenderScene) -> Vec<&RenderNode> {
        scene
            .nodes()
            .iter()
            .filter(|n| matches!(n.kind, RenderNodeKind::Text { .. }))
            .collect()
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
            vec![
                ("width".into(), PropValue::Number(200.0)),
                ("height".into(), PropValue::Number(100.0)),
            ],
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
            paint: ResolvedPaint::default(),
            overflow_hidden: false,
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
            paint: ResolvedPaint::default(),
            overflow_hidden: false,
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
            paint: ResolvedPaint::default(),
            overflow_hidden: false,
        };
        assert!(positioned_to_render_node(&pos).is_none());
    }

    #[test]
    fn test_zero_size_text_is_kept() {
        // Text must survive a 0x0 box: when no system font resolves, the leaf
        // measures zero and the old early-return dropped it, so nothing showed.
        let pos = PositionedNode {
            taffy_node: taffy::NodeId::new(0),
            element: make_text("Hello"),
            layout: uwebr_render::scene::LayoutInfo::new(0.0, 0.0, 0.0, 0.0),
            depth: 0,
            paint: ResolvedPaint::default(),
            overflow_hidden: false,
        };
        assert!(positioned_to_render_node(&pos).is_some());
    }

    #[test]
    fn test_blank_text_is_dropped() {
        let pos = PositionedNode {
            taffy_node: taffy::NodeId::new(0),
            element: make_text("   \n  "),
            layout: uwebr_render::scene::LayoutInfo::new(0.0, 0.0, 100.0, 20.0),
            depth: 0,
            paint: ResolvedPaint::default(),
            overflow_hidden: false,
        };
        assert!(positioned_to_render_node(&pos).is_none());
    }

    // ── CSS integration tests ─────────────────────────────────

    #[test]
    fn test_pipeline_with_css_tag() {
        let mut pipeline = RenderPipeline::new().with_css("div { width: 300px; height: 150px; }");
        let el = make_div(vec![]);
        let _scene = pipeline.render(&el, 800, 600);
    }

    #[test]
    fn test_pipeline_with_css_class() {
        let mut pipeline = RenderPipeline::new().with_css(".box { width: 200px; height: 100px; }");
        let el = make_div_with_props(
            vec![("class".into(), PropValue::String("box".into()))],
            vec![],
        );
        let _scene = pipeline.render(&el, 800, 600);
    }

    #[test]
    fn test_pipeline_with_css_id() {
        let mut pipeline = RenderPipeline::new().with_css("#main { width: 400px; height: 300px; }");
        let el = make_div_with_props(
            vec![("id".into(), PropValue::String("main".into()))],
            vec![],
        );
        let _scene = pipeline.render(&el, 800, 600);
    }

    #[test]
    fn test_pipeline_css_override_tag_default() {
        let mut pipeline =
            RenderPipeline::new().with_css("div { display: flex; flex-direction: row; }");
        let inner = make_div(vec![make_text("Child")]);
        let el = make_div(vec![inner]);
        let _scene = pipeline.render(&el, 800, 600);
    }

    #[test]
    fn test_pipeline_css_empty_string() {
        let mut pipeline = RenderPipeline::new().with_css("");
        let el = make_div(vec![]);
        let _scene = pipeline.render(&el, 800, 600);
    }

    #[test]
    fn test_pipeline_css_invalid() {
        let mut pipeline = RenderPipeline::new().with_css("invalid { {{ {");
        let el = make_div(vec![]);
        let _scene = pipeline.render(&el, 800, 600);
    }

    #[test]
    fn test_pipeline_with_stylebook() {
        let sb = StyleBook::parse(".flex { display: flex; }").unwrap();
        let mut pipeline = RenderPipeline::new().with_stylebook(sb);
        let el = make_div_with_props(
            vec![("class".into(), PropValue::String("flex".into()))],
            vec![make_text("Styled")],
        );
        let _scene = pipeline.render(&el, 800, 600);
    }

    #[test]
    fn test_pipeline_css_multiple_rules() {
        let css = ".header { width: 100%; height: 60px; } .content { padding: 16px; } .footer { height: 40px; }";
        let mut pipeline = RenderPipeline::new().with_css(css);
        let el = make_div(vec![
            make_div_with_props(
                vec![("class".into(), PropValue::String("header".into()))],
                vec![make_text("Header")],
            ),
            make_div_with_props(
                vec![("class".into(), PropValue::String("content".into()))],
                vec![make_text("Content")],
            ),
            make_div_with_props(
                vec![("class".into(), PropValue::String("footer".into()))],
                vec![make_text("Footer")],
            ),
        ]);
        let _scene = pipeline.render(&el, 800, 600);
    }

    // ── End-to-end assertions (M1 + M2) ───────────────────────

    /// The scaffold that `uwebr init` generates, verified end to end.
    fn scaffold_app() -> (String, Element) {
        let css = r#"
            .app {
                display: flex;
                flex-direction: column;
                align-items: center;
                justify-content: center;
                height: 100vh;
                background-color: #1a1a2e;
                color: #e0e0e0;
            }
            h1 { font-size: 2rem; }
        "#;
        let el = make_div_with_props(
            vec![("class".into(), PropValue::String("app".into()))],
            vec![make_el("h1", vec![], vec![make_text("Hello from uwebr!")])],
        );
        (css.to_string(), el)
    }

    #[test]
    fn test_scaffold_produces_text_node_in_scene() {
        let (css, el) = scaffold_app();
        let mut pipeline = RenderPipeline::new().with_css(&css);
        pipeline.build_render_scene(&el, 800, 600);

        let texts = text_nodes(pipeline.render_scene());
        assert_eq!(texts.len(), 1, "expected exactly one text node");
        match &texts[0].kind {
            RenderNodeKind::Text { content, .. } => {
                assert_eq!(content, "Hello from uwebr!");
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_scaffold_text_uses_css_font_size_and_color() {
        let (css, el) = scaffold_app();
        let mut pipeline = RenderPipeline::new().with_css(&css);
        pipeline.build_render_scene(&el, 800, 600);

        let texts = text_nodes(pipeline.render_scene());
        match &texts[0].kind {
            RenderNodeKind::Text {
                font_size, color, ..
            } => {
                // h1 { font-size: 2rem } → 32px, inherited .app color #e0e0e0.
                assert_eq!(*font_size, 32.0, "2rem should resolve to 32px");
                assert_eq!(
                    *color,
                    peniko::Color::from_rgba8(0xe0, 0xe0, 0xe0, 255),
                    "text colour must be inherited from .app"
                );
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_scaffold_background_reaches_scene() {
        let (css, el) = scaffold_app();
        let mut pipeline = RenderPipeline::new().with_css(&css);
        pipeline.build_render_scene(&el, 800, 600);

        let root = &pipeline.render_scene().nodes()[0];
        match &root.style.background {
            Some(Background::Solid(c)) => {
                assert_eq!(*c, peniko::Color::from_rgba8(0x1a, 0x1a, 0x2e, 255));
            }
            other => panic!("expected solid #1a1a2e background, got {other:?}"),
        }
    }

    #[test]
    fn test_scaffold_text_node_has_nonzero_box() {
        let (css, el) = scaffold_app();
        let mut pipeline = RenderPipeline::new().with_css(&css);
        pipeline.build_render_scene(&el, 800, 600);

        let texts = text_nodes(pipeline.render_scene());
        assert!(
            texts[0].layout.width > 0.0 && texts[0].layout.height > 0.0,
            "text box was {}x{}",
            texts[0].layout.width,
            texts[0].layout.height
        );
    }

    #[test]
    fn test_scaffold_encodes_glyphs_and_background_fill() {
        let (css, el) = scaffold_app();
        let mut pipeline = RenderPipeline::new().with_css(&css);
        let scene = pipeline.render(&el, 800, 600);

        assert!(
            path_count(&scene) >= 2,
            "surface background + .app background expected, got {}",
            path_count(&scene)
        );
        assert!(
            glyph_count(&scene) >= 5,
            "expected glyphs for 'Hello from uwebr!', got {}",
            glyph_count(&scene)
        );
    }

    #[test]
    fn test_text_centred_inside_app_container() {
        // justify-content/align-items: center means the text must not sit at 0,0.
        let (css, el) = scaffold_app();
        let mut pipeline = RenderPipeline::new().with_css(&css);
        pipeline.build_render_scene(&el, 800, 600);

        let texts = text_nodes(pipeline.render_scene());
        assert!(
            texts[0].layout.x > 0.0,
            "centred text should be offset from the left edge, x={}",
            texts[0].layout.x
        );
    }

    #[test]
    fn test_inline_prop_overrides_css_color() {
        let mut pipeline = RenderPipeline::new().with_css(".app { color: #ff0000; }");
        let el = make_div_with_props(
            vec![
                ("class".into(), PropValue::String("app".into())),
                ("color".into(), PropValue::String("#00ff00".into())),
            ],
            vec![make_text("Hi")],
        );
        pipeline.build_render_scene(&el, 800, 600);

        let texts = text_nodes(pipeline.render_scene());
        match &texts[0].kind {
            RenderNodeKind::Text { color, .. } => {
                assert_eq!(*color, peniko::Color::from_rgb8(0, 255, 0));
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_font_size_from_string_prop_reaches_scene() {
        // The transpiler emits every literal HTML attribute as a String, so a
        // Number-only read would silently fall back to the 16px default.
        let mut pipeline = RenderPipeline::new();
        let el = make_el(
            "h1",
            vec![("font-size".into(), PropValue::String("40".into()))],
            vec![make_text("Big")],
        );
        pipeline.build_render_scene(&el, 800, 600);

        let texts = text_nodes(pipeline.render_scene());
        match &texts[0].kind {
            RenderNodeKind::Text { font_size, .. } => assert_eq!(*font_size, 40.0),
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_multiple_text_nodes_all_present() {
        let mut pipeline = RenderPipeline::new();
        let el = make_div(vec![
            make_el("h1", vec![], vec![make_text("Title")]),
            make_el("p", vec![], vec![make_text("Body")]),
            make_el("span", vec![], vec![make_text("Footer")]),
        ]);
        pipeline.build_render_scene(&el, 800, 600);

        let texts = text_nodes(pipeline.render_scene());
        assert_eq!(texts.len(), 3);
    }

    #[test]
    fn test_nested_text_uses_absolute_position() {
        let mut pipeline = RenderPipeline::new().with_css(".pad { padding: 25px; }");
        let el = make_div_with_props(
            vec![("class".into(), PropValue::String("pad".into()))],
            vec![make_el("h1", vec![], vec![make_text("Indented")])],
        );
        pipeline.build_render_scene(&el, 800, 600);

        let texts = text_nodes(pipeline.render_scene());
        assert!(
            texts[0].layout.x >= 25.0,
            "text should be pushed in by the container padding, x={}",
            texts[0].layout.x
        );
    }

    #[test]
    fn test_render_is_idempotent_across_frames() {
        let (css, el) = scaffold_app();
        let mut pipeline = RenderPipeline::new().with_css(&css);
        let first = pipeline.render(&el, 800, 600);
        let second = pipeline.render(&el, 800, 600);
        assert_eq!(glyph_count(&first), glyph_count(&second));
        assert_eq!(path_count(&first), path_count(&second));
    }

    #[test]
    fn test_border_from_css_reaches_scene() {
        let mut pipeline =
            RenderPipeline::new().with_css(".b { border-width: 3px; border-color: red; }");
        let el = make_div_with_props(
            vec![
                ("class".into(), PropValue::String("b".into())),
                ("width".into(), PropValue::Number(50.0)),
                ("height".into(), PropValue::Number(50.0)),
            ],
            vec![],
        );
        pipeline.build_render_scene(&el, 800, 600);

        let border = pipeline.render_scene().nodes()[0]
            .style
            .border
            .as_ref()
            .expect("border present");
        assert_eq!(border.width, 3.0);
        assert_eq!(border.color, peniko::Color::from_rgba8(255, 0, 0, 255));
    }

    // ── Hit testing for on:click (M6) ─────────────────────────

    fn clickable_button(action: &str) -> Element {
        make_el(
            "button",
            vec![
                ("on:click".into(), PropValue::Closure(action.into())),
                ("width".into(), PropValue::Number(100.0)),
                ("height".into(), PropValue::Number(40.0)),
            ],
            vec![make_text("+")],
        )
    }

    #[test]
    fn test_click_prop_registers_hit_target() {
        let mut pipeline = RenderPipeline::new();
        pipeline.build_render_scene(&clickable_button("increment"), 800, 600);

        let targets = pipeline.hit_targets();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].action, "increment");
    }

    #[test]
    fn test_hit_test_inside_bounds() {
        let mut pipeline = RenderPipeline::new();
        pipeline.build_render_scene(&clickable_button("increment"), 800, 600);
        assert_eq!(pipeline.hit_test(10.0, 10.0), Some("increment"));
    }

    #[test]
    fn test_hit_test_outside_bounds() {
        let mut pipeline = RenderPipeline::new();
        pipeline.build_render_scene(&clickable_button("increment"), 800, 600);
        assert_eq!(pipeline.hit_test(700.0, 500.0), None);
    }

    #[test]
    fn test_hit_test_prefers_innermost_target() {
        // Nested clickables: the deeper one should win, as in the DOM.
        let inner = make_el(
            "button",
            vec![
                ("on:click".into(), PropValue::Closure("inner".into())),
                ("width".into(), PropValue::Number(50.0)),
                ("height".into(), PropValue::Number(20.0)),
            ],
            vec![],
        );
        let outer = make_el(
            "div",
            vec![
                ("on:click".into(), PropValue::Closure("outer".into())),
                ("width".into(), PropValue::Number(200.0)),
                ("height".into(), PropValue::Number(100.0)),
            ],
            vec![inner],
        );

        let mut pipeline = RenderPipeline::new();
        pipeline.build_render_scene(&outer, 800, 600);
        assert_eq!(pipeline.hit_test(5.0, 5.0), Some("inner"));
        assert_eq!(
            pipeline.hit_test(5.0, 60.0),
            Some("outer"),
            "below the inner button, still inside the outer div"
        );
    }

    #[test]
    fn test_non_closure_click_prop_ignored() {
        // A literal string is not a resolvable action name.
        let el = make_el(
            "button",
            vec![
                ("on:click".into(), PropValue::String("increment".into())),
                ("width".into(), PropValue::Number(10.0)),
                ("height".into(), PropValue::Number(10.0)),
            ],
            vec![],
        );
        let mut pipeline = RenderPipeline::new();
        pipeline.build_render_scene(&el, 800, 600);
        assert!(pipeline.hit_targets().is_empty());
    }

    #[test]
    fn test_hit_targets_cleared_between_frames() {
        let mut pipeline = RenderPipeline::new();
        pipeline.build_render_scene(&clickable_button("a"), 800, 600);
        pipeline.build_render_scene(&make_div(vec![]), 800, 600);
        assert!(pipeline.hit_targets().is_empty());
    }

    #[test]
    fn test_hit_test_uses_absolute_coordinates() {
        // The clickable sits inside a padded container, so its hit box must be
        // offset — a parent-relative box would mis-target clicks.
        let mut pipeline = RenderPipeline::new().with_css(".pad { padding: 30px; }");
        let el = make_div_with_props(
            vec![("class".into(), PropValue::String("pad".into()))],
            vec![clickable_button("go")],
        );
        pipeline.build_render_scene(&el, 800, 600);

        assert_eq!(pipeline.hit_test(5.0, 5.0), None, "inside the padding");
        assert_eq!(pipeline.hit_test(40.0, 40.0), Some("go"));
    }

    // ── CSS fixes (FAZ 10) ─────────────────────────────────────

    #[test]
    fn test_overflow_hidden_reaches_render_style() {
        let mut pipeline = RenderPipeline::new()
            .with_css(".clip { overflow: hidden; width: 100px; height: 100px; }");
        let el = make_div_with_props(
            vec![("class".into(), PropValue::String("clip".into()))],
            vec![],
        );
        pipeline.build_render_scene(&el, 800, 600);

        assert!(
            pipeline.render_scene().nodes()[0].style.overflow_hidden,
            "overflow:hidden must reach the render style"
        );
    }

    #[test]
    fn test_gradient_background_reaches_scene() {
        let mut pipeline = RenderPipeline::new().with_css(
            ".g { background: linear-gradient(to right, red, blue); width: 100px; height: 100px; }",
        );
        let el = make_div_with_props(
            vec![("class".into(), PropValue::String("g".into()))],
            vec![],
        );
        pipeline.build_render_scene(&el, 800, 600);

        match &pipeline.render_scene().nodes()[0].style.background {
            Some(Background::LinearGradient { stops, .. }) => {
                assert_eq!(stops.len(), 2);
            }
            other => panic!("expected a linear gradient, got {other:?}"),
        }
    }

    #[test]
    fn test_nested_vw_resolves_against_viewport() {
        // An inner element sized 50vw must be 400px on an 800px viewport, even
        // though its parent is narrower — the old percent approximation failed.
        let mut pipeline = RenderPipeline::new().with_css(
            ".outer { width: 600px; height: 400px; } .inner { width: 50vw; height: 50px; }",
        );
        let el = make_div_with_props(
            vec![("class".into(), PropValue::String("outer".into()))],
            vec![make_div_with_props(
                vec![("class".into(), PropValue::String("inner".into()))],
                vec![],
            )],
        );
        pipeline.build_render_scene(&el, 800, 600);

        let inner = &pipeline.render_scene().nodes()[1];
        assert_eq!(
            inner.layout.width, 400.0,
            "50vw must resolve to 400px against the 800px viewport, got {}",
            inner.layout.width
        );
    }

    // ── FAZ 11: image, ellipsis, {@html} ───────────────────────

    #[test]
    fn test_img_element_produces_image_node() {
        let mut pipeline = RenderPipeline::new();
        let el = make_el(
            "img",
            vec![
                ("src".into(), PropValue::String("fake-bytes".into())),
                ("width".into(), PropValue::Number(100.0)),
                ("height".into(), PropValue::Number(80.0)),
            ],
            vec![],
        );
        pipeline.build_render_scene(&el, 800, 600);

        let node = &pipeline.render_scene().nodes()[0];
        match &node.kind {
            RenderNodeKind::Image {
                data,
                width,
                height,
            } => {
                assert_eq!(data, b"fake-bytes");
                assert_eq!(*width, 100);
                assert_eq!(*height, 80);
            }
            other => panic!("expected an image node, got {other:?}"),
        }
    }

    #[test]
    fn test_img_width_height_from_string_props() {
        let mut pipeline = RenderPipeline::new();
        let el = make_el(
            "img",
            vec![
                ("src".into(), PropValue::String("x".into())),
                ("width".into(), PropValue::String("64".into())),
                ("height".into(), PropValue::String("48".into())),
            ],
            vec![],
        );
        pipeline.build_render_scene(&el, 800, 600);

        match &pipeline.render_scene().nodes()[0].kind {
            RenderNodeKind::Image { width, height, .. } => {
                assert_eq!(*width, 64);
                assert_eq!(*height, 48);
            }
            other => panic!("expected an image node, got {other:?}"),
        }
    }

    #[test]
    fn test_text_overflow_reaches_render_style() {
        let mut pipeline = RenderPipeline::new()
            .with_css(".t { text-overflow: ellipsis; width: 100px; height: 20px; }");
        let el = make_div_with_props(
            vec![("class".into(), PropValue::String("t".into()))],
            vec![make_text("Some overflowing text")],
        );
        pipeline.build_render_scene(&el, 800, 600);

        assert_eq!(
            pipeline.render_scene().nodes()[0].style.text_overflow,
            uwebr_render::scene::TextOverflow::Ellipsis
        );
    }

    #[test]
    fn test_raw_html_produces_render_node() {
        let mut pipeline = RenderPipeline::new();
        let el = Element {
            node_type: NodeType::Raw("<div>Hi</div>".to_string()),
            props: vec![],
            children: vec![],
        };
        pipeline.build_render_scene(&el, 800, 600);

        let texts = text_nodes(pipeline.render_scene());
        assert_eq!(texts.len(), 1, "runtime HTML text should reach the scene");
        match &texts[0].kind {
            RenderNodeKind::Text { content, .. } => assert_eq!(content, "Hi"),
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_raw_invalid_html_falls_back_to_text() {
        let mut pipeline = RenderPipeline::new();
        let el = Element {
            node_type: NodeType::Raw("not markup".to_string()),
            props: vec![],
            children: vec![],
        };
        pipeline.build_render_scene(&el, 800, 600);

        let texts = text_nodes(pipeline.render_scene());
        assert_eq!(texts.len(), 1);
        match &texts[0].kind {
            RenderNodeKind::Text { content, .. } => assert_eq!(content, "not markup"),
            _ => unreachable!(),
        }
    }
}
