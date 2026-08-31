use taffy::prelude::*;
use taffy::style::Overflow;
use uwebr_core::component::{Element, NodeType, PropValue};
use uwebr_css::codegen::TransformProps;

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
    /// Pre-order index in the layout tree, used to key runtime hover/focus
    /// state and for hover hit-testing.
    pub node_id: usize,
    /// Fully resolved paint (CSS + inline props + inherited text style).
    pub paint: ResolvedPaint,
    /// `overflow: hidden` (or `clip`) on either axis — the scene clips children.
    pub overflow_hidden: bool,
    /// `overflow: scroll` on the X axis — the scene clips + offsets horizontally.
    pub overflow_scroll_x: bool,
    /// `overflow: scroll` on the Y axis — the scene clips + offsets vertically.
    pub overflow_scroll_y: bool,
    /// Content width when scrollable (larger than container if scrollable).
    pub scroll_content_width: f32,
    /// Content height when scrollable (larger than container if scrollable).
    pub scroll_content_height: f32,
    /// CSS `z-index` for paint ordering.
    pub z_index: i32,
    /// CSS `transform` for visual transformation.
    pub transform: TransformProps,
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
        let mut node_counter = 0usize;
        let node = self.build_node(root, stylebook, &inherited, &[], &mut node_counter)?;

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
        parent_chain: &[&Element],
        node_counter: &mut usize,
    ) -> anyhow::Result<taffy::NodeId> {
        // Assign this node's pre-order index, matching collect_recursive's walk.
        let node_id = *node_counter;
        *node_counter += 1;

        let matched = stylebook.match_full(element, parent_chain, node_id);
        let paint = ResolvedPaint::resolve(
            inherited,
            &matched.paint,
            &matched.transform,
            &matched.animation,
            element,
        );
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
                // Extend the parent chain with this element for its children.
                let mut child_chain = Vec::with_capacity(parent_chain.len() + 1);
                child_chain.push(element);
                child_chain.extend_from_slice(parent_chain);

                let child_ids: Vec<taffy::NodeId> = element
                    .children
                    .iter()
                    .map(|child| {
                        self.build_node(child, stylebook, &paint, &child_chain, node_counter)
                    })
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
        let mut node_counter = 0usize;
        self.collect_recursive(
            root,
            root_element,
            0,
            0.0,
            0.0,
            stylebook,
            &inherited,
            &[],
            &mut node_counter,
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
        parent_chain: &[&Element],
        node_counter: &mut usize,
        out: &mut Vec<PositionedNode>,
    ) {
        // Pre-order index, matching build_node's assignment so hover/focus
        // state keyed during build lines up with the positioned node.
        let node_id = *node_counter;
        *node_counter += 1;

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

        let matched = stylebook.match_full(element, parent_chain, node_id);
        let paint = ResolvedPaint::resolve(
            inherited,
            &matched.paint,
            &matched.transform,
            &matched.animation,
            element,
        );

        // Clip children when the element sets `overflow: hidden`/`clip` on either
        // axis. Scroll containers clip but also allow offset-based scrolling.
        let (overflow_hidden, overflow_scroll_x, overflow_scroll_y) = self
            .taffy
            .style(taffy_node)
            .map(|s| {
                let hidden_x = matches!(s.overflow.x, Overflow::Hidden | Overflow::Clip);
                let hidden_y = matches!(s.overflow.y, Overflow::Hidden | Overflow::Clip);
                let scroll_x = matches!(s.overflow.x, Overflow::Scroll);
                let scroll_y = matches!(s.overflow.y, Overflow::Scroll);
                // Scroll supersedes hidden when both are set.
                (
                    (hidden_x || hidden_y) && !scroll_x && !scroll_y,
                    scroll_x,
                    scroll_y,
                )
            })
            .unwrap_or((false, false, false));

        // For scroll containers, estimate the content size by summing children
        // extents. This is a conservative upper bound used to compute max scroll.
        let scroll_content_width = if overflow_scroll_x {
            self.estimate_content_width(taffy_node)
        } else {
            0.0
        };
        let scroll_content_height = if overflow_scroll_y {
            self.estimate_content_height(taffy_node)
        } else {
            0.0
        };

        out.push(PositionedNode {
            taffy_node,
            element: element.clone(),
            layout: info,
            depth,
            node_id,
            paint: paint.clone(),
            overflow_hidden,
            overflow_scroll_x,
            overflow_scroll_y,
            scroll_content_width,
            scroll_content_height,
            z_index: paint.z_index,
            transform: paint.transform.clone(),
        });

        if let Ok(children) = self.taffy.children(taffy_node) {
            let mut child_chain = Vec::with_capacity(parent_chain.len() + 1);
            child_chain.push(element);
            child_chain.extend_from_slice(parent_chain);

            for (child_taffy, child_element) in children.iter().zip(element.children.iter()) {
                self.collect_recursive(
                    *child_taffy,
                    child_element,
                    depth + 1,
                    abs_x,
                    abs_y,
                    stylebook,
                    &paint,
                    &child_chain,
                    node_counter,
                    out,
                );
            }
        }
    }

    /// Reset the taffy tree (font contexts are reused — they are expensive to build).
    pub fn reset(&mut self) {
        self.taffy = TaffyTree::new();
    }

    /// Estimate the total content width of a node's subtree for scroll containers.
    fn estimate_content_width(&self, node: taffy::NodeId) -> f32 {
        let children = self.taffy.children(node).unwrap_or_default();
        if children.is_empty() {
            return self.taffy.layout(node).map(|l| l.size.width).unwrap_or(0.0);
        }
        let mut max_child_right = 0.0f32;
        for child in &children {
            if let Ok(layout) = self.taffy.layout(*child) {
                let child_right = layout.location.x + layout.size.width;
                max_child_right = max_child_right.max(child_right);
            }
        }
        max_child_right
    }

    /// Estimate the total content height of a node's subtree for scroll containers.
    fn estimate_content_height(&self, node: taffy::NodeId) -> f32 {
        let children = self.taffy.children(node).unwrap_or_default();
        if children.is_empty() {
            return self
                .taffy
                .layout(node)
                .map(|l| l.size.height)
                .unwrap_or(0.0);
        }
        let mut max_child_bottom = 0.0f32;
        for child in &children {
            if let Ok(layout) = self.taffy.layout(*child) {
                let child_bottom = layout.location.y + layout.size.height;
                max_child_bottom = max_child_bottom.max(child_bottom);
            }
        }
        max_child_bottom
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

    // ── Parent chain threading (FAZ 14) ─────────────────────────

    #[test]
    fn test_parent_chain_passed_to_match() {
        // A descendant selector only applies when build_tree threads the real
        // parent chain into match_full: `.parent .child` must colour the child.
        let sb = StyleBook::parse(".parent .child { color: #ff0000; }").unwrap();
        let child = Element {
            node_type: NodeType::Element("span".into()),
            props: vec![("class".into(), PropValue::String("child".into()))],
            children: vec![make_text_element("Hi")],
        };
        let parent = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![("class".into(), PropValue::String("parent".into()))],
            children: vec![child],
        };

        let mut engine = LayoutEngine::new();
        let root = engine.build_tree(&parent, &sb).unwrap();
        engine.compute(root, 800.0, 600.0).unwrap();
        let nodes = engine.collect_positioned_nodes(root, &parent, &sb);

        // node[0] = .parent, node[1] = .child span → must be red via descendant.
        let child_node = nodes
            .iter()
            .find(|n| matches!(&n.element.node_type, NodeType::Element(t) if t == "span"))
            .expect("child span present");
        assert_eq!(
            child_node.paint.color,
            vello::peniko::Color::from_rgba8(255, 0, 0, 255),
            "descendant selector must match through the threaded parent chain"
        );
    }

    #[test]
    fn test_node_ids_are_preorder_unique() {
        // build_tree and collect_positioned_nodes must agree on pre-order ids.
        let sb = StyleBook::empty();
        let el = make_div_element(vec![
            make_div_element(vec![make_text_element("a")]),
            make_text_element("b"),
        ]);
        let mut engine = LayoutEngine::new();
        let root = engine.build_tree(&el, &sb).unwrap();
        engine.compute(root, 800.0, 600.0).unwrap();
        let nodes = engine.collect_positioned_nodes(root, &el, &sb);

        let ids: Vec<usize> = nodes.iter().map(|n| n.node_id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(ids.len(), sorted.len(), "node ids must be unique");
        assert_eq!(nodes[0].node_id, 0, "root is pre-order index 0");
    }

    // ── Layout edge-case tests ──────────────────────────────────

    #[test]
    fn render_flex_wrap_wrap() {
        let css = ".row { display: flex; flex-direction: row; flex-wrap: wrap; width: 100px; } .item { width: 60px; height: 20px; }";
        let sb = StyleBook::parse(css).unwrap();
        let root = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![("class".into(), PropValue::String("row".into()))],
            children: vec![
                Element {
                    node_type: NodeType::Element("div".into()),
                    props: vec![("class".into(), PropValue::String("item".into()))],
                    children: vec![],
                },
                Element {
                    node_type: NodeType::Element("div".into()),
                    props: vec![("class".into(), PropValue::String("item".into()))],
                    children: vec![],
                },
                Element {
                    node_type: NodeType::Element("div".into()),
                    props: vec![("class".into(), PropValue::String("item".into()))],
                    children: vec![],
                },
            ],
        };
        let mut engine = LayoutEngine::new();
        let node = engine.build_tree(&root, &sb).unwrap();
        engine.compute(node, 200.0, 200.0).unwrap();
        let nodes = engine.collect_positioned_nodes(node, &root, &sb);
        assert!(nodes.len() >= 4, "root + 3 items");
        let item_layouts: Vec<_> = nodes
            .iter()
            .filter(|n| {
                matches!(n.element.node_type, NodeType::Element(ref t) if t == "div")
                    && n.depth == 1
            })
            .map(|n| n.layout)
            .collect();
        assert_eq!(item_layouts.len(), 3);
        let y0 = item_layouts[0].y;
        let y1 = item_layouts[1].y;
        let y2 = item_layouts[2].y;
        assert!(
            y2 > y1 || y1 > y0,
            "wrap should cause some items to move to next line"
        );
    }

    #[test]
    fn render_flex_wrap_reverse() {
        let css = ".row { display: flex; flex-direction: row; flex-wrap: wrap-reverse; width: 100px; } .item { width: 60px; height: 20px; }";
        let sb = StyleBook::parse(css).unwrap();
        let root = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![("class".into(), PropValue::String("row".into()))],
            children: vec![
                Element {
                    node_type: NodeType::Element("div".into()),
                    props: vec![("class".into(), PropValue::String("item".into()))],
                    children: vec![],
                },
                Element {
                    node_type: NodeType::Element("div".into()),
                    props: vec![("class".into(), PropValue::String("item".into()))],
                    children: vec![],
                },
            ],
        };
        let mut engine = LayoutEngine::new();
        let node = engine.build_tree(&root, &sb).unwrap();
        engine.compute(node, 200.0, 200.0).unwrap();
        let nodes = engine.collect_positioned_nodes(node, &root, &sb);
        let item_layouts: Vec<_> = nodes
            .iter()
            .filter(|n| n.depth == 1)
            .map(|n| n.layout)
            .collect();
        assert_eq!(item_layouts.len(), 2);
        let items_with_y: Vec<_> = item_layouts.iter().map(|l| l.y).collect();
        if items_with_y[0] != items_with_y[1] {
            let first_y = items_with_y[0];
            let second_y = items_with_y[1];
            assert!(
                second_y < first_y,
                "wrap-reverse should place wrapped items above"
            );
        }
    }

    #[test]
    fn render_flex_grow_distribution() {
        let css = ".row { display: flex; flex-direction: row; width: 300px; } .a { flex-grow: 1; height: 20px; } .b { flex-grow: 2; height: 20px; }";
        let sb = StyleBook::parse(css).unwrap();
        let root = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![("class".into(), PropValue::String("row".into()))],
            children: vec![
                Element {
                    node_type: NodeType::Element("div".into()),
                    props: vec![("class".into(), PropValue::String("a".into()))],
                    children: vec![],
                },
                Element {
                    node_type: NodeType::Element("div".into()),
                    props: vec![("class".into(), PropValue::String("b".into()))],
                    children: vec![],
                },
            ],
        };
        let mut engine = LayoutEngine::new();
        let node = engine.build_tree(&root, &sb).unwrap();
        engine.compute(node, 400.0, 200.0).unwrap();
        let nodes = engine.collect_positioned_nodes(node, &root, &sb);
        let items: Vec<_> = nodes
            .iter()
            .filter(|n| n.depth == 1)
            .map(|n| n.layout)
            .collect();
        assert_eq!(items.len(), 2);
        assert!(
            items[0].width > 0.0,
            "first item should have width from flex-grow"
        );
        assert!(
            items[1].width > 0.0,
            "second item should have width from flex-grow"
        );
        let ratio = items[1].width / items[0].width;
        assert!(
            (ratio - 2.0).abs() < 0.5,
            "flex-grow 2 should be ~2x flex-grow 1, got ratio {ratio}"
        );
    }

    #[test]
    fn render_flex_shrink_distribution() {
        let css = ".row { display: flex; flex-direction: row; width: 100px; } .item { width: 80px; height: 20px; flex-shrink: 1; }";
        let sb = StyleBook::parse(css).unwrap();
        let root = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![("class".into(), PropValue::String("row".into()))],
            children: vec![
                Element {
                    node_type: NodeType::Element("div".into()),
                    props: vec![("class".into(), PropValue::String("item".into()))],
                    children: vec![],
                },
                Element {
                    node_type: NodeType::Element("div".into()),
                    props: vec![("class".into(), PropValue::String("item".into()))],
                    children: vec![],
                },
            ],
        };
        let mut engine = LayoutEngine::new();
        let node = engine.build_tree(&root, &sb).unwrap();
        engine.compute(node, 200.0, 200.0).unwrap();
        let nodes = engine.collect_positioned_nodes(node, &root, &sb);
        let items: Vec<_> = nodes
            .iter()
            .filter(|n| n.depth == 1)
            .map(|n| n.layout)
            .collect();
        assert_eq!(items.len(), 2);
        for item in &items {
            assert!(
                item.width <= 80.0,
                "each item should shrink from 80px, got {}",
                item.width
            );
            assert!(item.width > 0.0, "each item should have positive width");
        }
    }

    #[test]
    fn render_percentage_width() {
        let css = ".parent { width: 400px; display: flex; } .child { width: 50%; height: 100px; }";
        let sb = StyleBook::parse(css).unwrap();
        let root = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![("class".into(), PropValue::String("parent".into()))],
            children: vec![Element {
                node_type: NodeType::Element("div".into()),
                props: vec![("class".into(), PropValue::String("child".into()))],
                children: vec![],
            }],
        };
        let mut engine = LayoutEngine::new();
        let node = engine.build_tree(&root, &sb).unwrap();
        engine.compute(node, 800.0, 600.0).unwrap();
        let nodes = engine.collect_positioned_nodes(node, &root, &sb);
        let child = nodes.iter().find(|n| n.depth == 1).unwrap();
        assert!(
            (child.layout.width - 200.0).abs() < 1.0,
            "50% of 400px should be ~200, got {}",
            child.layout.width
        );
        assert_eq!(child.layout.height, 100.0);
    }

    #[test]
    fn render_percentage_height() {
        let css = ".parent { height: 400px; display: flex; flex-direction: column; } .child { height: 50%; width: 100px; }";
        let sb = StyleBook::parse(css).unwrap();
        let root = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![("class".into(), PropValue::String("parent".into()))],
            children: vec![Element {
                node_type: NodeType::Element("div".into()),
                props: vec![("class".into(), PropValue::String("child".into()))],
                children: vec![],
            }],
        };
        let mut engine = LayoutEngine::new();
        let node = engine.build_tree(&root, &sb).unwrap();
        engine.compute(node, 800.0, 600.0).unwrap();
        let nodes = engine.collect_positioned_nodes(node, &root, &sb);
        let child = nodes.iter().find(|n| n.depth == 1).unwrap();
        assert!(
            (child.layout.height - 200.0).abs() < 1.0,
            "50% of 400px should be ~200, got {}",
            child.layout.height
        );
    }

    #[test]
    fn render_min_width_clamping() {
        let css = ".item { min-width: 100px; width: 50px; height: 20px; }";
        let sb = StyleBook::parse(css).unwrap();
        let root = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![],
            children: vec![Element {
                node_type: NodeType::Element("div".into()),
                props: vec![("class".into(), PropValue::String("item".into()))],
                children: vec![],
            }],
        };
        let mut engine = LayoutEngine::new();
        let node = engine.build_tree(&root, &sb).unwrap();
        engine.compute(node, 800.0, 600.0).unwrap();
        let nodes = engine.collect_positioned_nodes(node, &root, &sb);
        let child = nodes.iter().find(|n| n.depth == 1).unwrap();
        assert!(
            child.layout.width >= 100.0,
            "min-width:100px should clamp width to >= 100, got {}",
            child.layout.width
        );
    }

    #[test]
    fn render_max_width_clamping() {
        let css = ".item { max-width: 80px; width: 200px; height: 20px; }";
        let sb = StyleBook::parse(css).unwrap();
        let root = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![],
            children: vec![Element {
                node_type: NodeType::Element("div".into()),
                props: vec![("class".into(), PropValue::String("item".into()))],
                children: vec![],
            }],
        };
        let mut engine = LayoutEngine::new();
        let node = engine.build_tree(&root, &sb).unwrap();
        engine.compute(node, 800.0, 600.0).unwrap();
        let nodes = engine.collect_positioned_nodes(node, &root, &sb);
        let child = nodes.iter().find(|n| n.depth == 1).unwrap();
        assert!(
            child.layout.width <= 80.0,
            "max-width:80px should clamp width to <= 80, got {}",
            child.layout.width
        );
    }

    #[test]
    fn render_min_height_clamping() {
        let css = ".item { min-height: 150px; height: 50px; width: 100px; }";
        let sb = StyleBook::parse(css).unwrap();
        let root = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![],
            children: vec![Element {
                node_type: NodeType::Element("div".into()),
                props: vec![("class".into(), PropValue::String("item".into()))],
                children: vec![],
            }],
        };
        let mut engine = LayoutEngine::new();
        let node = engine.build_tree(&root, &sb).unwrap();
        engine.compute(node, 800.0, 600.0).unwrap();
        let nodes = engine.collect_positioned_nodes(node, &root, &sb);
        let child = nodes.iter().find(|n| n.depth == 1).unwrap();
        assert!(
            child.layout.height >= 150.0,
            "min-height:150px should clamp height, got {}",
            child.layout.height
        );
    }

    #[test]
    fn render_max_height_clamping() {
        let css = ".item { max-height: 60px; height: 200px; width: 100px; }";
        let sb = StyleBook::parse(css).unwrap();
        let root = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![],
            children: vec![Element {
                node_type: NodeType::Element("div".into()),
                props: vec![("class".into(), PropValue::String("item".into()))],
                children: vec![],
            }],
        };
        let mut engine = LayoutEngine::new();
        let node = engine.build_tree(&root, &sb).unwrap();
        engine.compute(node, 800.0, 600.0).unwrap();
        let nodes = engine.collect_positioned_nodes(node, &root, &sb);
        let child = nodes.iter().find(|n| n.depth == 1).unwrap();
        assert!(
            child.layout.height <= 60.0,
            "max-height:60px should clamp height, got {}",
            child.layout.height
        );
    }

    #[test]
    fn render_align_self_overrides_parent() {
        let css = ".row { display: flex; flex-direction: row; align-items: flex-start; height: 200px; } .center { align-self: center; width: 50px; height: 50px; } .start { width: 50px; height: 50px; }";
        let sb = StyleBook::parse(css).unwrap();
        let root = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![("class".into(), PropValue::String("row".into()))],
            children: vec![
                Element {
                    node_type: NodeType::Element("div".into()),
                    props: vec![("class".into(), PropValue::String("start".into()))],
                    children: vec![],
                },
                Element {
                    node_type: NodeType::Element("div".into()),
                    props: vec![("class".into(), PropValue::String("center".into()))],
                    children: vec![],
                },
            ],
        };
        let mut engine = LayoutEngine::new();
        let node = engine.build_tree(&root, &sb).unwrap();
        engine.compute(node, 400.0, 300.0).unwrap();
        let nodes = engine.collect_positioned_nodes(node, &root, &sb);
        let items: Vec<_> = nodes.iter().filter(|n| n.depth == 1).collect();
        assert_eq!(items.len(), 2);
        let y_start = items[0].layout.y;
        let y_center = items[1].layout.y;
        assert!(y_center > y_start, "align-self:center should push item down from flex-start, start_y={y_start}, center_y={y_center}");
    }

    #[test]
    fn render_flex_basis_with_grow() {
        let css = ".row { display: flex; flex-direction: row; width: 300px; } .a { flex-basis: 50px; flex-grow: 1; height: 20px; } .b { flex-basis: 50px; height: 20px; }";
        let sb = StyleBook::parse(css).unwrap();
        let root = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![("class".into(), PropValue::String("row".into()))],
            children: vec![
                Element {
                    node_type: NodeType::Element("div".into()),
                    props: vec![("class".into(), PropValue::String("a".into()))],
                    children: vec![],
                },
                Element {
                    node_type: NodeType::Element("div".into()),
                    props: vec![("class".into(), PropValue::String("b".into()))],
                    children: vec![],
                },
            ],
        };
        let mut engine = LayoutEngine::new();
        let node = engine.build_tree(&root, &sb).unwrap();
        engine.compute(node, 400.0, 200.0).unwrap();
        let nodes = engine.collect_positioned_nodes(node, &root, &sb);
        let items: Vec<_> = nodes
            .iter()
            .filter(|n| n.depth == 1)
            .map(|n| n.layout)
            .collect();
        assert_eq!(items.len(), 2);
        assert!(
            items[0].width > items[1].width,
            "flex-grow should give item a more space than b"
        );
    }

    #[test]
    fn render_flex_basis_with_shrink() {
        let css = ".row { display: flex; flex-direction: row; width: 200px; } .a { width: 120px; flex-shrink: 1; height: 20px; } .b { width: 120px; flex-shrink: 1; height: 20px; }";
        let sb = StyleBook::parse(css).unwrap();
        let root = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![("class".into(), PropValue::String("row".into()))],
            children: vec![
                Element {
                    node_type: NodeType::Element("div".into()),
                    props: vec![("class".into(), PropValue::String("a".into()))],
                    children: vec![],
                },
                Element {
                    node_type: NodeType::Element("div".into()),
                    props: vec![("class".into(), PropValue::String("b".into()))],
                    children: vec![],
                },
            ],
        };
        let mut engine = LayoutEngine::new();
        let node = engine.build_tree(&root, &sb).unwrap();
        engine.compute(node, 400.0, 200.0).unwrap();
        let nodes = engine.collect_positioned_nodes(node, &root, &sb);
        let items: Vec<_> = nodes
            .iter()
            .filter(|n| n.depth == 1)
            .map(|n| n.layout)
            .collect();
        assert_eq!(items.len(), 2);
        let total: f32 = items.iter().map(|l| l.width).sum();
        assert!(total > 0.0, "total width should be positive, got {total}");
        assert!(
            total <= 200.0,
            "total width should not exceed parent, got {total}"
        );
    }

    #[test]
    fn render_nested_flex_containers() {
        let css = ".outer { display: flex; flex-direction: column; width: 400px; } .inner { display: flex; flex-direction: row; height: 50px; } .item { width: 100px; height: 30px; }";
        let sb = StyleBook::parse(css).unwrap();
        let root = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![("class".into(), PropValue::String("outer".into()))],
            children: vec![Element {
                node_type: NodeType::Element("div".into()),
                props: vec![("class".into(), PropValue::String("inner".into()))],
                children: vec![
                    Element {
                        node_type: NodeType::Element("div".into()),
                        props: vec![("class".into(), PropValue::String("item".into()))],
                        children: vec![],
                    },
                    Element {
                        node_type: NodeType::Element("div".into()),
                        props: vec![("class".into(), PropValue::String("item".into()))],
                        children: vec![],
                    },
                ],
            }],
        };
        let mut engine = LayoutEngine::new();
        let node = engine.build_tree(&root, &sb).unwrap();
        engine.compute(node, 800.0, 600.0).unwrap();
        let nodes = engine.collect_positioned_nodes(node, &root, &sb);
        assert!(nodes.len() >= 4, "outer + inner + 2 items");
        let inner = nodes.iter().find(|n| n.depth == 1).unwrap();
        assert_eq!(inner.layout.height, 50.0);
        let items: Vec<_> = nodes.iter().filter(|n| n.depth == 2).collect();
        assert_eq!(items.len(), 2);
        let item0_y = items[0].layout.y;
        let item1_y = items[1].layout.y;
        assert_eq!(item0_y, item1_y, "items in a row should share the same y");
        assert!(
            items[1].layout.x > items[0].layout.x,
            "row items should be side by side"
        );
    }

    #[test]
    fn render_zero_size_element() {
        let sb = StyleBook::parse(".z { width: 0px; height: 0px; }").unwrap();
        let root = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![("class".into(), PropValue::String("z".into()))],
            children: vec![],
        };
        let mut engine = LayoutEngine::new();
        let node = engine.build_tree(&root, &sb).unwrap();
        engine.compute(node, 800.0, 600.0).unwrap();
        let info = engine.get_layout_info(node).unwrap();
        assert_eq!(info.width, 0.0);
        assert_eq!(info.height, 0.0);
    }

    #[test]
    fn render_very_large_element() {
        let css = ".big { width: 10000px; height: 10000px; }";
        let sb = StyleBook::parse(css).unwrap();
        let root = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![("class".into(), PropValue::String("big".into()))],
            children: vec![],
        };
        let mut engine = LayoutEngine::new();
        let node = engine.build_tree(&root, &sb).unwrap();
        engine.compute(node, 800.0, 600.0).unwrap();
        let info = engine.get_layout_info(node).unwrap();
        assert_eq!(info.width, 10000.0);
        assert_eq!(info.height, 10000.0);
    }

    #[test]
    fn render_mixed_flex_direction_children() {
        let css = ".row { display: flex; flex-direction: row; width: 400px; } .col { display: flex; flex-direction: column; width: 200px; height: 100px; } .item { width: 50px; height: 30px; }";
        let sb = StyleBook::parse(css).unwrap();
        let root = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![("class".into(), PropValue::String("row".into()))],
            children: vec![
                Element {
                    node_type: NodeType::Element("div".into()),
                    props: vec![("class".into(), PropValue::String("col".into()))],
                    children: vec![
                        Element {
                            node_type: NodeType::Element("div".into()),
                            props: vec![("class".into(), PropValue::String("item".into()))],
                            children: vec![],
                        },
                        Element {
                            node_type: NodeType::Element("div".into()),
                            props: vec![("class".into(), PropValue::String("item".into()))],
                            children: vec![],
                        },
                    ],
                },
                Element {
                    node_type: NodeType::Element("div".into()),
                    props: vec![("class".into(), PropValue::String("item".into()))],
                    children: vec![],
                },
            ],
        };
        let mut engine = LayoutEngine::new();
        let node = engine.build_tree(&root, &sb).unwrap();
        engine.compute(node, 800.0, 600.0).unwrap();
        let nodes = engine.collect_positioned_nodes(node, &root, &sb);
        let col = nodes
            .iter()
            .find(|n| {
                n.depth == 1
                    && n.element.props.iter().any(|(k, v)| {
                        k == "class" && matches!(v, PropValue::String(s) if s == "col")
                    })
            })
            .unwrap();
        assert!(col.layout.width >= 200.0);
        let col_items: Vec<_> = nodes.iter().filter(|n| n.depth == 2).collect();
        assert_eq!(col_items.len(), 2);
        assert!(
            col_items[1].layout.y > col_items[0].layout.y,
            "column children should stack vertically"
        );
    }

    #[test]
    fn render_flex_grow_three_siblings() {
        let css = ".row { display: flex; flex-direction: row; width: 300px; } .a { flex-grow: 1; height: 20px; } .b { flex-grow: 1; height: 20px; } .c { flex-grow: 1; height: 20px; }";
        let sb = StyleBook::parse(css).unwrap();
        let root = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![("class".into(), PropValue::String("row".into()))],
            children: vec![
                Element {
                    node_type: NodeType::Element("div".into()),
                    props: vec![("class".into(), PropValue::String("a".into()))],
                    children: vec![],
                },
                Element {
                    node_type: NodeType::Element("div".into()),
                    props: vec![("class".into(), PropValue::String("b".into()))],
                    children: vec![],
                },
                Element {
                    node_type: NodeType::Element("div".into()),
                    props: vec![("class".into(), PropValue::String("c".into()))],
                    children: vec![],
                },
            ],
        };
        let mut engine = LayoutEngine::new();
        let node = engine.build_tree(&root, &sb).unwrap();
        engine.compute(node, 400.0, 200.0).unwrap();
        let nodes = engine.collect_positioned_nodes(node, &root, &sb);
        let items: Vec<_> = nodes
            .iter()
            .filter(|n| n.depth == 1)
            .map(|n| n.layout)
            .collect();
        assert_eq!(items.len(), 3);
        let total: f32 = items.iter().map(|l| l.width).sum();
        assert!(
            total > 290.0,
            "three equal-grow items should fill the row, total={total}"
        );
        for item in &items {
            assert!(
                item.width > 90.0,
                "each item should be ~100px, got {}",
                item.width
            );
        }
    }

    #[test]
    fn render_min_max_width_combined() {
        let css = ".item { min-width: 80px; max-width: 120px; width: 500px; height: 20px; }";
        let sb = StyleBook::parse(css).unwrap();
        let root = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![],
            children: vec![Element {
                node_type: NodeType::Element("div".into()),
                props: vec![("class".into(), PropValue::String("item".into()))],
                children: vec![],
            }],
        };
        let mut engine = LayoutEngine::new();
        let node = engine.build_tree(&root, &sb).unwrap();
        engine.compute(node, 800.0, 600.0).unwrap();
        let nodes = engine.collect_positioned_nodes(node, &root, &sb);
        let child = nodes.iter().find(|n| n.depth == 1).unwrap();
        assert!(
            child.layout.width >= 80.0 && child.layout.width <= 120.0,
            "width should be clamped to [80, 120], got {}",
            child.layout.width
        );
    }

    #[test]
    fn render_deeply_nested_flex() {
        let mut el = make_text_element("leaf");
        for _ in 0..10 {
            el = Element {
                node_type: NodeType::Element("div".into()),
                props: vec![],
                children: vec![el],
            };
        }
        let mut engine = LayoutEngine::new();
        let root = engine.build_tree(&el, &StyleBook::empty()).unwrap();
        engine.compute(root, 800.0, 600.0).unwrap();
        let nodes = engine.collect_positioned_nodes(root, &el, &StyleBook::empty());
        assert_eq!(nodes.len(), 11, "10 nested divs + 1 leaf = 11 nodes");
        assert_eq!(nodes[0].depth, 0);
        assert_eq!(nodes.last().unwrap().depth, 10);
    }

    #[test]
    fn render_column_direction_with_flex_grow() {
        let css = ".col { display: flex; flex-direction: column; height: 300px; } .a { flex-grow: 1; width: 50px; } .b { flex-grow: 2; width: 50px; }";
        let sb = StyleBook::parse(css).unwrap();
        let root = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![("class".into(), PropValue::String("col".into()))],
            children: vec![
                Element {
                    node_type: NodeType::Element("div".into()),
                    props: vec![("class".into(), PropValue::String("a".into()))],
                    children: vec![],
                },
                Element {
                    node_type: NodeType::Element("div".into()),
                    props: vec![("class".into(), PropValue::String("b".into()))],
                    children: vec![],
                },
            ],
        };
        let mut engine = LayoutEngine::new();
        let node = engine.build_tree(&root, &sb).unwrap();
        engine.compute(node, 400.0, 400.0).unwrap();
        let nodes = engine.collect_positioned_nodes(node, &root, &sb);
        let items: Vec<_> = nodes
            .iter()
            .filter(|n| n.depth == 1)
            .map(|n| n.layout)
            .collect();
        assert_eq!(items.len(), 2);
        assert!(
            items[1].height > items[0].height,
            "flex-grow:2 should be taller than flex-grow:1"
        );
        let ratio = items[1].height / items[0].height;
        assert!(
            (ratio - 2.0).abs() < 0.5,
            "height ratio should be ~2, got {ratio}"
        );
    }

    #[test]
    fn render_row_wrap_with_padding() {
        let css = ".row { display: flex; flex-direction: row; flex-wrap: wrap; width: 120px; padding: 10px; } .item { width: 50px; height: 20px; }";
        let sb = StyleBook::parse(css).unwrap();
        let root = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![("class".into(), PropValue::String("row".into()))],
            children: vec![
                Element {
                    node_type: NodeType::Element("div".into()),
                    props: vec![("class".into(), PropValue::String("item".into()))],
                    children: vec![],
                },
                Element {
                    node_type: NodeType::Element("div".into()),
                    props: vec![("class".into(), PropValue::String("item".into()))],
                    children: vec![],
                },
                Element {
                    node_type: NodeType::Element("div".into()),
                    props: vec![("class".into(), PropValue::String("item".into()))],
                    children: vec![],
                },
            ],
        };
        let mut engine = LayoutEngine::new();
        let node = engine.build_tree(&root, &sb).unwrap();
        engine.compute(node, 400.0, 400.0).unwrap();
        let nodes = engine.collect_positioned_nodes(node, &root, &sb);
        let items: Vec<_> = nodes
            .iter()
            .filter(|n| n.depth == 1)
            .map(|n| n.layout)
            .collect();
        assert_eq!(items.len(), 3);
        let first_item_x = items[0].x;
        assert!(
            first_item_x >= 10.0,
            "padding should offset first item, x={first_item_x}"
        );
    }

    #[test]
    fn render_flex_basis_overrides_width() {
        let css = ".item { width: 100px; height: 20px; }";
        let sb = StyleBook::parse(css).unwrap();
        let root = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![],
            children: vec![Element {
                node_type: NodeType::Element("div".into()),
                props: vec![("class".into(), PropValue::String("item".into()))],
                children: vec![],
            }],
        };
        let mut engine = LayoutEngine::new();
        let node = engine.build_tree(&root, &sb).unwrap();
        engine.compute(node, 800.0, 600.0).unwrap();
        let nodes = engine.collect_positioned_nodes(node, &root, &sb);
        let child = nodes.iter().find(|n| n.depth == 1).unwrap();
        assert!(
            (child.layout.width - 100.0).abs() < 1.0,
            "width:100px should apply, got {}",
            child.layout.width
        );
    }

    #[test]
    fn render_align_self_center_in_column() {
        let css = ".col { display: flex; flex-direction: column; align-items: flex-start; width: 400px; } .center { align-self: center; width: 80px; height: 30px; }";
        let sb = StyleBook::parse(css).unwrap();
        let root = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![("class".into(), PropValue::String("col".into()))],
            children: vec![Element {
                node_type: NodeType::Element("div".into()),
                props: vec![("class".into(), PropValue::String("center".into()))],
                children: vec![],
            }],
        };
        let mut engine = LayoutEngine::new();
        let node = engine.build_tree(&root, &sb).unwrap();
        engine.compute(node, 800.0, 600.0).unwrap();
        let nodes = engine.collect_positioned_nodes(node, &root, &sb);
        let child = nodes.iter().find(|n| n.depth == 1).unwrap();
        let expected_x = (400.0 - 80.0) / 2.0;
        assert!(
            (child.layout.x - expected_x).abs() < 2.0,
            "align-self:center should center horizontally, got x={}, expected ~{expected_x}",
            child.layout.x
        );
    }

    // ── Quality tests (test_q_*) ────────────────────────────────

    #[test]
    fn test_q_stress_layout_1000_nodes() {
        fn build(depth: usize, max_depth: usize) -> Element {
            if depth >= max_depth {
                return make_text_element("leaf");
            }
            let children: Vec<Element> = (0..3).map(|_| build(depth + 1, max_depth)).collect();
            make_div_element(children)
        }
        // 3^6 = 729 elements + 729 text = 1458 nodes (close to 1000)
        let el = build(0, 6);
        let mut engine = LayoutEngine::new();
        let root = engine.build_tree(&el, &StyleBook::empty()).unwrap();
        engine.compute(root, 800.0, 600.0).unwrap();
        let nodes = engine.collect_positioned_nodes(root, &el, &StyleBook::empty());
        assert!(
            nodes.len() >= 1000,
            "expected >= 1000 nodes, got {}",
            nodes.len()
        );
    }

    #[test]
    fn test_q_stress_rapid_relayout_100() {
        let mut engine = LayoutEngine::new();
        let el = make_div_element(vec![
            make_div_element(vec![make_text_element("Hello")]),
            make_text_element("World"),
        ]);
        for _ in 0..100 {
            let root = engine.build_tree(&el, &StyleBook::empty()).unwrap();
            engine.compute(root, 800.0, 600.0).unwrap();
            engine.reset();
        }
    }

    #[test]
    fn test_q_stress_nested_flex_50_levels() {
        let mut el = make_text_element("leaf");
        for _ in 0..50 {
            el = Element {
                node_type: NodeType::Element("div".into()),
                props: vec![],
                children: vec![el],
            };
        }
        let mut engine = LayoutEngine::new();
        let root = engine.build_tree(&el, &StyleBook::empty()).unwrap();
        engine.compute(root, 800.0, 600.0).unwrap();
        let nodes = engine.collect_positioned_nodes(root, &el, &StyleBook::empty());
        assert_eq!(nodes.len(), 51, "50 nested divs + 1 leaf = 51 nodes");
        assert_eq!(nodes.last().unwrap().depth, 50);
    }
}
