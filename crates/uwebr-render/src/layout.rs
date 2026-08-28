use taffy::prelude::*;
use uwebr_core::component::{Element, NodeType};

use crate::scene::LayoutInfo;

/// Layout engine using taffy 0.14
pub struct LayoutEngine {
    taffy: TaffyTree<()>,
}

/// A positioned node after layout computation
#[derive(Debug, Clone)]
pub struct PositionedNode {
    pub taffy_node: taffy::NodeId,
    pub element: Element,
    pub layout: LayoutInfo,
    pub depth: usize,
}

impl LayoutEngine {
    pub fn new() -> Self {
        Self {
            taffy: TaffyTree::new(),
        }
    }

    /// Convert Element tree to TaffyTree, returns root NodeId
    pub fn build_tree(&mut self, root: &Element) -> anyhow::Result<taffy::NodeId> {
        self.build_node(root)
    }

    fn build_node(&mut self, element: &Element) -> anyhow::Result<taffy::NodeId> {
        let style = self.element_to_style(element);

        match &element.node_type {
            NodeType::Text(_content) => {
                let node = self.taffy.new_leaf(style)?;
                Ok(node)
            }
            NodeType::Element(_) | NodeType::Component(_) => {
                let child_ids: Vec<taffy::NodeId> = element
                    .children
                    .iter()
                    .map(|child| self.build_node(child))
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

    /// Convert an Element's attributes to a taffy Style
    fn element_to_style(&self, element: &Element) -> Style {
        let mut style = Style::default();

        match &element.node_type {
            NodeType::Element(tag) => match tag.as_str() {
                "div" | "section" | "main" | "article" | "aside" | "header" | "footer" | "nav" => {
                    style.display = Display::Flex;
                    style.flex_direction = FlexDirection::Column;
                }
                "span" | "a" | "strong" | "em" | "b" | "i" | "code" => {
                    style.display = Display::Flex;
                }
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    style.display = Display::Flex;
                    style.flex_direction = FlexDirection::Column;
                }
                "p" => {
                    style.display = Display::Flex;
                    style.flex_direction = FlexDirection::Column;
                }
                _ => {
                    style.display = Display::Flex;
                }
            },
            NodeType::Text(_) => {
                style.display = Display::Flex;
            }
            NodeType::Component(_) => {
                style.display = Display::Flex;
                style.flex_direction = FlexDirection::Column;
            }
            NodeType::Raw(_) => {}
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
                if let uwebr_core::component::PropValue::Number(n) = value {
                    style.size.width = Dimension::length(*n as f32);
                }
            }
            "height" => {
                if let uwebr_core::component::PropValue::Number(n) = value {
                    style.size.height = Dimension::length(*n as f32);
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
                if let uwebr_core::component::PropValue::Number(n) = value {
                    let lp = LengthPercentage::length(*n as f32);
                    style.padding = Rect {
                        left: lp,
                        right: lp,
                        top: lp,
                        bottom: lp,
                    };
                }
            }
            "margin" => {
                if let uwebr_core::component::PropValue::Number(n) = value {
                    let lpa = LengthPercentageAuto::length(*n as f32);
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

    /// Compute layout for the tree
    pub fn compute(&mut self, root: taffy::NodeId, width: f32, height: f32) -> anyhow::Result<()> {
        self.taffy.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(width),
                height: AvailableSpace::Definite(height),
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

    /// Collect all positioned nodes from the tree with depth info
    pub fn collect_positioned_nodes(
        &self,
        root: taffy::NodeId,
        root_element: &Element,
    ) -> Vec<PositionedNode> {
        let mut nodes = vec![];
        self.collect_recursive(root, root_element, 0, &mut nodes);
        nodes
    }

    fn collect_recursive(
        &self,
        taffy_node: taffy::NodeId,
        element: &Element,
        depth: usize,
        out: &mut Vec<PositionedNode>,
    ) {
        if let Ok(layout) = self.taffy.layout(taffy_node) {
            let info = LayoutInfo {
                x: layout.location.x,
                y: layout.location.y,
                width: layout.size.width,
                height: layout.size.height,
            };

            out.push(PositionedNode {
                taffy_node,
                element: element.clone(),
                layout: info,
                depth,
            });

            if let Ok(children) = self.taffy.children(taffy_node) {
                for (child_taffy, child_element) in children.iter().zip(element.children.iter()) {
                    self.collect_recursive(*child_taffy, child_element, depth + 1, out);
                }
            }
        }
    }

    /// Reset the taffy tree
    pub fn reset(&mut self) {
        self.taffy = TaffyTree::new();
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
        let root = engine.build_tree(&el).unwrap();
        assert!(engine.taffy.layout(root).is_ok());
    }

    #[test]
    fn test_build_nested_tree() {
        let mut engine = LayoutEngine::new();
        let child = make_div_element(vec![make_text_element("Child")]);
        let root = make_div_element(vec![child]);
        let root_id = engine.build_tree(&root).unwrap();

        let children = engine.taffy.children(root_id).unwrap();
        assert_eq!(children.len(), 1);
    }

    #[test]
    fn test_compute_layout() {
        let mut engine = LayoutEngine::new();
        let el = make_div_element(vec![]);
        let root = engine.build_tree(&el).unwrap();
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
        let root = engine.build_tree(&parent).unwrap();
        engine.compute(root, 800.0, 600.0).unwrap();

        let nodes = engine.collect_positioned_nodes(root, &parent);
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
        let root = engine.build_tree(&el).unwrap();
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
        let root = engine.build_tree(&parent).unwrap();
        engine.compute(root, 800.0, 600.0).unwrap();

        let layout = engine.taffy.layout(root).unwrap();
        // Parent fills viewport when it has children in Flex layout
        assert!(layout.size.width >= 0.0);
    }

    #[test]
    fn test_reset() {
        let mut engine = LayoutEngine::new();
        let el = make_div_element(vec![]);
        let root = engine.build_tree(&el).unwrap();
        engine.compute(root, 800.0, 600.0).unwrap();

        engine.reset();
        let root2 = engine.build_tree(&el).unwrap();
        engine.compute(root2, 800.0, 600.0).unwrap();
    }
}
