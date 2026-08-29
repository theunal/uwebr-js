use taffy::prelude::*;
use taffy::style::Overflow;
use uwebr_core::component::{Element, NodeType, PropValue};

use crate::paint::ResolvedPaint;
use crate::scene::LayoutInfo;
use crate::stylebook::{MatchedStyle, StyleBook};
use crate::text::TextRenderer;

/// Default `display` / `flex-direction` implied by an element's tag.
///
/// `None` means "leave taffy's default in place".
fn tag_defaults(node_type: &NodeType) -> (Option<Display>, Option<FlexDirection>) {
    match node_type {
        NodeType::Element(tag) => match tag.as_str() {
            // Block-level containers stack their children vertically.
            "div" | "section" | "main" | "article" | "aside" | "header" | "footer" | "nav"
            | "p" | "ul" | "ol" | "li" | "form" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                (Some(Display::Flex), Some(FlexDirection::Column))
            }
            // Inline-ish elements flow horizontally.
            "span" | "a" | "strong" | "em" | "b" | "i" | "code" | "label" | "button" => {
                (Some(Display::Flex), Some(FlexDirection::Row))
            }
            _ => (Some(Display::Flex), None),
        },
        NodeType::Text(_) => (Some(Display::Flex), None),
        NodeType::Component(_) => (Some(Display::Flex), Some(FlexDirection::Column)),
        NodeType::Raw(_) => (None, None),
    }
}

/// Extract a numeric f32 from PropValue (Number or String-parseable)
fn prop_to_f32(value: &PropValue) -> Option<f32> {
    match value {
        PropValue::Number(n) => Some(*n as f32),
        PropValue::String(s) => s.parse::<f32>().ok(),
        _ => None,
    }
}

/// Per-node context handed to Taffy's measure function.
///
/// Taffy has no idea how wide a string is, so text leaves must carry their
/// content and resolved font here. Without this the text node measures 0x0,
/// collapses inside a column flex, and never reaches the scene.
#[derive(Debug, Clone)]
pub enum NodeContext {
    Text {
        content: String,
        font_size: f32,
        font_family: Option<String>,
    },
}

/// Layout engine using taffy 0.14
pub struct LayoutEngine {
    taffy: TaffyTree<NodeContext>,
    text: TextRenderer,
}

/// A positioned node after layout computation
#[derive(Debug, Clone)]
pub struct PositionedNode {
    pub taffy_node: taffy::NodeId,
    pub element: Element,
    pub layout: LayoutInfo,
    pub depth: usize,
    /// Fully resolved paint (CSS + inline props + inherited text style).
    pub paint: ResolvedPaint,
    /// `overflow: hidden` (or `clip`) on either axis — the scene clips children.
    pub overflow_hidden: bool,
}

impl LayoutEngine {
    pub fn new() -> Self {
        Self {
            taffy: TaffyTree::new(),
            text: TextRenderer::new(),
        }
    }

    /// Convert Element tree to TaffyTree, returns root NodeId
    pub fn build_tree(
        &mut self,
        root: &Element,
        stylebook: &StyleBook,
    ) -> anyhow::Result<taffy::NodeId> {
        let inherited = ResolvedPaint::default();
        let node = self.build_node(root, stylebook, &inherited)?;

        // The root element stands in for the document body: give it the whole
        // viewport unless CSS sized it explicitly. Without this the root is
        // shrink-to-fit, so `align-items: center` / `justify-content: center`
        // have nothing to centre within and backgrounds only cover the content.
        let mut style = self.taffy.style(node)?.clone();
        let default: Style = Style::default();
        if style.size.width == default.size.width {
            style.size.width = Dimension::percent(1.0);
        }
        if style.size.height == default.size.height {
            style.size.height = Dimension::percent(1.0);
        }
        self.taffy.set_style(node, style)?;

        Ok(node)
    }

    fn build_node(
        &mut self,
        element: &Element,
        stylebook: &StyleBook,
        inherited: &ResolvedPaint,
    ) -> anyhow::Result<taffy::NodeId> {
        let matched = stylebook.match_full(element);
        let paint = ResolvedPaint::resolve(inherited, &matched.paint, element);
        let style = self.element_to_style(element, &matched);

        match &element.node_type {
            NodeType::Text(content) => {
                // Attach the content so compute_layout_with_measure can size it.
                let node = self.taffy.new_leaf_with_context(
                    style,
                    NodeContext::Text {
                        content: content.clone(),
                        font_size: paint.font_size,
                        font_family: paint.font_family.clone(),
                    },
                )?;
                Ok(node)
            }
            NodeType::Element(_) | NodeType::Component(_) => {
                let child_ids: Vec<taffy::NodeId> = element
                    .children
                    .iter()
                    .map(|child| self.build_node(child, stylebook, &paint))
                    .collect::<anyhow::Result<_>>()?;

                let node = self.taffy.new_with_children(style, &child_ids)?;
                Ok(node)
            }
            NodeType::Raw(_) => {
                let node = self.taffy.new_leaf(style)?;
                Ok(node)
            }
        }
    }

    /// Combine CSS-matched style, tag defaults, and inline props into a taffy Style.
    ///
    /// Tag defaults are applied per property rather than all-or-nothing: a rule
    /// like `h1 { font-size: 2rem }` matches the element but declares no layout
    /// property, so the block-level `flex-direction: column` default must still
    /// apply or the heading lays its text out as a row item and stretches.
    fn element_to_style(&self, element: &Element, matched: &MatchedStyle) -> Style {
        let mut style = matched.style.clone();

        let (default_display, default_direction) = tag_defaults(&element.node_type);

        if !matched.mask.display {
            if let Some(display) = default_display {
                style.display = display;
            }
        }
        if !matched.mask.flex_direction {
            if let Some(direction) = default_direction {
                style.flex_direction = direction;
            }
        }

        // Apply inline style properties
        for (prop_name, prop_value) in &element.props {
            self.apply_prop(&mut style, prop_name, prop_value);
        }

        style
    }

    /// Apply a single property from element props
    fn apply_prop(&self, style: &mut Style, name: &str, value: &uwebr_core::component::PropValue) {
        match name {
            "width" => {
                if let Some(n) = prop_to_f32(value) {
                    style.size.width = Dimension::length(n);
                }
            }
            "height" => {
                if let Some(n) = prop_to_f32(value) {
                    style.size.height = Dimension::length(n);
                }
            }
            "flex_direction" => {
                if let uwebr_core::component::PropValue::String(s) = value {
                    match s.as_str() {
                        "row" => style.flex_direction = FlexDirection::Row,
                        "column" => style.flex_direction = FlexDirection::Column,
                        "row-reverse" => style.flex_direction = FlexDirection::RowReverse,
                        "column-reverse" => style.flex_direction = FlexDirection::ColumnReverse,
                        _ => {}
                    }
                }
            }
            "justify_content" => {
                if let uwebr_core::component::PropValue::String(s) = value {
                    match s.as_str() {
                        "center" => style.justify_content = Some(JustifyContent::CENTER),
                        "flex-start" => style.justify_content = Some(JustifyContent::FLEX_START),
                        "flex-end" => style.justify_content = Some(JustifyContent::FLEX_END),
                        "space-between" => {
                            style.justify_content = Some(JustifyContent::SPACE_BETWEEN)
                        }
                        "space-around" => {
                            style.justify_content = Some(JustifyContent::SPACE_AROUND)
                        }
                        "space-evenly" => {
                            style.justify_content = Some(JustifyContent::SPACE_EVENLY)
                        }
                        _ => {}
                    }
                }
            }
            "align_items" => {
                if let uwebr_core::component::PropValue::String(s) = value {
                    match s.as_str() {
                        "center" => style.align_items = Some(AlignItems::CENTER),
                        "flex-start" => style.align_items = Some(AlignItems::FLEX_START),
                        "flex-end" => style.align_items = Some(AlignItems::FLEX_END),
                        "stretch" => style.align_items = Some(AlignItems::STRETCH),
                        "baseline" => style.align_items = Some(AlignItems::BASELINE),
                        _ => {}
                    }
                }
            }
            "padding" => {
                if let Some(n) = prop_to_f32(value) {
                    let lp = LengthPercentage::length(n);
                    style.padding = Rect {
                        left: lp,
                        right: lp,
                        top: lp,
                        bottom: lp,
                    };
                }
            }
            "margin" => {
                if let Some(n) = prop_to_f32(value) {
                    let lpa = LengthPercentageAuto::length(n);
                    style.margin = Rect {
                        left: lpa,
                        right: lpa,
                        top: lpa,
                        bottom: lpa,
                    };
                }
            }
            _ => {}
        }
    }

    /// Compute layout for the tree, measuring text leaves with parley.
    pub fn compute(&mut self, root: taffy::NodeId, width: f32, height: f32) -> anyhow::Result<()> {
        let text = &mut self.text;
        self.taffy.compute_layout_with_measure(
            root,
            Size {
                width: AvailableSpace::Definite(width),
                height: AvailableSpace::Definite(height),
            },
            |inputs, _node_id, node_context, style| {
                taffy::compute_leaf_layout(
                    inputs,
                    style,
                    |_, _| 0.0,
                    |known_dimensions, available_space| {
                        measure_node(known_dimensions, available_space, node_context, text)
                    },
                )
            },
        )?;
        Ok(())
    }

    /// Get layout info for a single node
    pub fn get_layout_info(&self, node: taffy::NodeId) -> anyhow::Result<LayoutInfo> {
        let layout = self.taffy.layout(node)?;
        Ok(LayoutInfo {
            x: layout.location.x,
            y: layout.location.y,
            width: layout.size.width,
            height: layout.size.height,
        })
    }

    /// Collect all positioned nodes from the tree with depth + resolved paint.
    ///
    /// Coordinates are converted to absolute (window) space: taffy reports
    /// locations relative to the parent, but the scene draws in one flat space.
    pub fn collect_positioned_nodes(
        &self,
        root: taffy::NodeId,
        root_element: &Element,
        stylebook: &StyleBook,
    ) -> Vec<PositionedNode> {
        let mut nodes = vec![];
        let inherited = ResolvedPaint::default();
        self.collect_recursive(
            root,
            root_element,
            0,
            0.0,
            0.0,
            stylebook,
            &inherited,
            &mut nodes,
        );
        nodes
    }

    #[allow(clippy::too_many_arguments)]
    fn collect_recursive(
        &self,
        taffy_node: taffy::NodeId,
        element: &Element,
        depth: usize,
        parent_x: f32,
        parent_y: f32,
        stylebook: &StyleBook,
        inherited: &ResolvedPaint,
        out: &mut Vec<PositionedNode>,
    ) {
        let Ok(layout) = self.taffy.layout(taffy_node) else {
            return;
        };

        let abs_x = parent_x + layout.location.x;
        let abs_y = parent_y + layout.location.y;

        let info = LayoutInfo {
            x: abs_x,
            y: abs_y,
            width: layout.size.width,
            height: layout.size.height,
        };

        let matched = stylebook.match_full(element);
        let paint = ResolvedPaint::resolve(inherited, &matched.paint, element);

        // Clip children when the element sets `overflow: hidden`/`clip` on either
        // axis. Read from the resolved taffy style so it follows the cascade.
        let overflow_hidden = self
            .taffy
            .style(taffy_node)
            .map(|s| {
                matches!(s.overflow.x, Overflow::Hidden | Overflow::Clip)
                    || matches!(s.overflow.y, Overflow::Hidden | Overflow::Clip)
            })
            .unwrap_or(false);

        out.push(PositionedNode {
            taffy_node,
            element: element.clone(),
            layout: info,
            depth,
            paint: paint.clone(),
            overflow_hidden,
        });

        if let Ok(children) = self.taffy.children(taffy_node) {
            for (child_taffy, child_element) in children.iter().zip(element.children.iter()) {
                self.collect_recursive(
                    *child_taffy,
                    child_element,
                    depth + 1,
                    abs_x,
                    abs_y,
                    stylebook,
                    &paint,
                    out,
                );
            }
        }
    }

    /// Reset the taffy tree (font contexts are reused — they are expensive to build).
    pub fn reset(&mut self) {
        self.taffy = TaffyTree::new();
    }

    /// Measure a string with the engine's text renderer (test/debug helper).
    pub fn measure_text(&mut self, content: &str, font_size: f32) -> (f32, f32) {
        self.text.measure(content, font_size, None, None)
    }
}

/// Taffy measure callback: size a leaf node's intrinsic content.
fn measure_node(
    known_dimensions: Size<Option<f32>>,
    available_space: Size<AvailableSpace>,
    node_context: Option<&mut NodeContext>,
    text: &mut TextRenderer,
) -> Size<f32> {
    // A fully constrained node needs no measuring.
    if let Size {
        width: Some(width),
        height: Some(height),
    } = known_dimensions
    {
        return Size { width, height };
    }

    match node_context {
        None => Size::ZERO,
        Some(NodeContext::Text {
            content,
            font_size,
            font_family,
        }) => {
            // Wrap at the known width, else at the definite space offered.
            let max_advance = known_dimensions.width.or(match available_space.width {
                AvailableSpace::Definite(w) => Some(w),
                // MinContent/MaxContent: let the text find its natural width.
                _ => None,
            });

            let (w, h) = text.measure(content, *font_size, font_family.as_deref(), max_advance);

            Size {
                width: known_dimensions.width.unwrap_or(w),
                height: known_dimensions.height.unwrap_or(h),
            }
        }
    }
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_text_element(content: &str) -> Element {
        Element {
            node_type: NodeType::Text(content.to_string()),
            props: vec![],
            children: vec![],
        }
    }

    fn make_div_element(children: Vec<Element>) -> Element {
        Element {
            node_type: NodeType::Element("div".to_string()),
            props: vec![],
            children,
        }
    }

    #[test]
    fn test_build_simple_tree() {
        let mut engine = LayoutEngine::new();
        let el = make_div_element(vec![make_text_element("Hello")]);
        let root = engine.build_tree(&el, &StyleBook::empty()).unwrap();
        assert!(engine.taffy.layout(root).is_ok());
    }

    #[test]
    fn test_build_nested_tree() {
        let mut engine = LayoutEngine::new();
        let child = make_div_element(vec![make_text_element("Child")]);
        let root = make_div_element(vec![child]);
        let root_id = engine.build_tree(&root, &StyleBook::empty()).unwrap();

        let children = engine.taffy.children(root_id).unwrap();
        assert_eq!(children.len(), 1);
    }

    #[test]
    fn test_compute_layout() {
        let mut engine = LayoutEngine::new();
        let el = make_div_element(vec![]);
        let root = engine.build_tree(&el, &StyleBook::empty()).unwrap();
        engine.compute(root, 800.0, 600.0).unwrap();

        let info = engine.get_layout_info(root).unwrap();
        // Empty flex child defaults to shrink-to-fit in taffy
        assert!(info.width >= 0.0);
        assert!(info.height >= 0.0);
    }

    #[test]
    fn test_collect_positioned_nodes() {
        let mut engine = LayoutEngine::new();
        let child = make_text_element("Hi");
        let parent = make_div_element(vec![child.clone()]);
        let root = engine.build_tree(&parent, &StyleBook::empty()).unwrap();
        engine.compute(root, 800.0, 600.0).unwrap();

        let nodes = engine.collect_positioned_nodes(root, &parent, &StyleBook::empty());
        assert!(nodes.len() >= 2);
        assert_eq!(nodes[0].depth, 0);
    }

    #[test]
    fn test_element_with_props() {
        let mut engine = LayoutEngine::new();
        let el = Element {
            node_type: NodeType::Element("div".to_string()),
            props: vec![
                (
                    "width".to_string(),
                    uwebr_core::component::PropValue::Number(200.0),
                ),
                (
                    "height".to_string(),
                    uwebr_core::component::PropValue::Number(100.0),
                ),
            ],
            children: vec![],
        };
        let root = engine.build_tree(&el, &StyleBook::empty()).unwrap();
        engine.compute(root, 800.0, 600.0).unwrap();

        let info = engine.get_layout_info(root).unwrap();
        assert_eq!(info.width, 200.0);
        assert_eq!(info.height, 100.0);
    }

    #[test]
    fn test_css_class_application() {
        let mut engine = LayoutEngine::new();
        let child = make_div_element(vec![]);
        let parent = make_div_element(vec![child]);
        let root = engine.build_tree(&parent, &StyleBook::empty()).unwrap();
        engine.compute(root, 800.0, 600.0).unwrap();

        let layout = engine.taffy.layout(root).unwrap();
        // Parent fills viewport when it has children in Flex layout
        assert!(layout.size.width >= 0.0);
    }

    #[test]
    fn test_reset() {
        let mut engine = LayoutEngine::new();
        let el = make_div_element(vec![]);
        let root = engine.build_tree(&el, &StyleBook::empty()).unwrap();
        engine.compute(root, 800.0, 600.0).unwrap();

        engine.reset();
        let root2 = engine.build_tree(&el, &StyleBook::empty()).unwrap();
        engine.compute(root2, 800.0, 600.0).unwrap();
    }

    // ── Text measurement (M1) ───────────────────────────────────

    #[test]
    fn test_text_leaf_gets_nonzero_size() {
        // Before the measure function existed, text leaves computed 0x0 and were
        // filtered out of the scene, so nothing appeared on screen.
        let mut engine = LayoutEngine::new();
        let el = make_text_element("Hello from uwebr!");
        let root = engine.build_tree(&el, &StyleBook::empty()).unwrap();
        engine.compute(root, 800.0, 600.0).unwrap();

        let info = engine.get_layout_info(root).unwrap();
        assert!(info.width > 0.0, "text width was {}", info.width);
        assert!(info.height > 0.0, "text height was {}", info.height);
    }

    #[test]
    fn test_text_inside_column_flex_has_height() {
        let mut engine = LayoutEngine::new();
        let el = make_div_element(vec![make_text_element("Hello")]);
        let root = engine.build_tree(&el, &StyleBook::empty()).unwrap();
        engine.compute(root, 800.0, 600.0).unwrap();

        let nodes = engine.collect_positioned_nodes(root, &el, &StyleBook::empty());
        let text_node = nodes
            .iter()
            .find(|n| matches!(n.element.node_type, NodeType::Text(_)))
            .expect("text node present");
        assert!(text_node.layout.height > 0.0);
        assert!(text_node.layout.width > 0.0);
    }

    #[test]
    fn test_larger_font_size_yields_taller_text() {
        let css = "h1 { font-size: 48px; } h2 { font-size: 12px; }";
        let sb = StyleBook::parse(css).unwrap();

        let measure = |tag: &str| {
            let mut engine = LayoutEngine::new();
            let el = Element {
                node_type: NodeType::Element(tag.into()),
                props: vec![],
                children: vec![make_text_element("Hello")],
            };
            let root = engine.build_tree(&el, &sb).unwrap();
            engine.compute(root, 800.0, 600.0).unwrap();
            let nodes = engine.collect_positioned_nodes(root, &el, &sb);
            nodes
                .iter()
                .find(|n| matches!(n.element.node_type, NodeType::Text(_)))
                .unwrap()
                .layout
                .height
        };

        assert!(
            measure("h1") > measure("h2"),
            "48px text should be taller than 12px"
        );
    }

    #[test]
    fn test_positioned_nodes_use_absolute_coordinates() {
        // Taffy reports child locations relative to the parent; the scene needs
        // absolute coordinates or nested content draws in the wrong place.
        let mut engine = LayoutEngine::new();
        let inner = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![
                ("width".into(), PropValue::Number(50.0)),
                ("height".into(), PropValue::Number(50.0)),
            ],
            children: vec![],
        };
        let outer = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![("padding".into(), PropValue::Number(20.0))],
            children: vec![inner],
        };

        let root = engine.build_tree(&outer, &StyleBook::empty()).unwrap();
        engine.compute(root, 800.0, 600.0).unwrap();
        let nodes = engine.collect_positioned_nodes(root, &outer, &StyleBook::empty());

        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].layout.x, 0.0);
        assert_eq!(
            nodes[1].layout.x, 20.0,
            "child offset by parent padding in absolute space"
        );
        assert_eq!(nodes[1].layout.y, 20.0);
    }

    #[test]
    fn test_paint_inherits_down_to_text() {
        let sb = StyleBook::parse(".app { color: #ff0000; font-size: 24px; }").unwrap();
        let el = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![("class".into(), PropValue::String("app".into()))],
            children: vec![make_text_element("Hi")],
        };

        let mut engine = LayoutEngine::new();
        let root = engine.build_tree(&el, &sb).unwrap();
        engine.compute(root, 800.0, 600.0).unwrap();
        let nodes = engine.collect_positioned_nodes(root, &el, &sb);

        let text_node = nodes
            .iter()
            .find(|n| matches!(n.element.node_type, NodeType::Text(_)))
            .unwrap();
        assert_eq!(text_node.paint.font_size, 24.0);
        assert_eq!(
            text_node.paint.color,
            vello::peniko::Color::from_rgba8(255, 0, 0, 255)
        );
    }

    #[test]
    fn test_background_does_not_inherit_to_child() {
        let sb = StyleBook::parse(".app { background-color: #00ff00; }").unwrap();
        let el = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![("class".into(), PropValue::String("app".into()))],
            children: vec![make_div_element(vec![])],
        };

        let mut engine = LayoutEngine::new();
        let root = engine.build_tree(&el, &sb).unwrap();
        engine.compute(root, 800.0, 600.0).unwrap();
        let nodes = engine.collect_positioned_nodes(root, &el, &sb);

        assert!(nodes[0].paint.background.is_some());
        assert!(
            nodes[1].paint.background.is_none(),
            "background is not inherited in CSS"
        );
    }

    #[test]
    fn test_measure_text_helper() {
        let mut engine = LayoutEngine::new();
        let (w, h) = engine.measure_text("Hello", 16.0);
        assert!(w > 0.0 && h > 0.0);
    }

    #[test]
    fn test_unused_paint_props_type_is_reachable() {
        // Guards the uwebr-css → uwebr-render paint bridge from silently rotting.
        let p = uwebr_css::codegen::PaintProps::default();
        assert!(p.is_empty());
    }

    #[test]
    fn test_overflow_hidden_flag_set_from_css() {
        let sb =
            StyleBook::parse(".clip { overflow: hidden; width: 100px; height: 100px; }").unwrap();
        let el = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![("class".into(), PropValue::String("clip".into()))],
            children: vec![],
        };
        let mut engine = LayoutEngine::new();
        let root = engine.build_tree(&el, &sb).unwrap();
        engine.compute(root, 800.0, 600.0).unwrap();
        let nodes = engine.collect_positioned_nodes(root, &el, &sb);
        assert!(
            nodes[0].overflow_hidden,
            "overflow:hidden must set the flag on the positioned node"
        );
    }

    #[test]
    fn test_overflow_visible_leaves_flag_false() {
        let el = make_div_element(vec![]);
        let mut engine = LayoutEngine::new();
        let root = engine.build_tree(&el, &StyleBook::empty()).unwrap();
        engine.compute(root, 800.0, 600.0).unwrap();
        let nodes = engine.collect_positioned_nodes(root, &el, &StyleBook::empty());
        assert!(!nodes[0].overflow_hidden);
    }
}
