use taffy::Style;
use uwebr_core::component::{Element, NodeType};
use uwebr_css::codegen::convert_to_taffy_styles;
use uwebr_css::parser::parse_css;

/// Parsed CSS stylesheet ready for layout matching
#[derive(Debug, Clone, Default)]
pub struct StyleBook {
    rules: Vec<(String, Style)>,
}

impl StyleBook {
    /// Parse a CSS string into a StyleBook
    pub fn parse(css: &str) -> anyhow::Result<Self> {
        let rules = parse_css(css)?;
        let styles = convert_to_taffy_styles(&rules)?;
        Ok(Self { rules: styles })
    }

    /// Create from pre-converted rules
    pub fn from_rules(rules: Vec<(String, Style)>) -> Self {
        Self { rules }
    }

    /// Empty stylebook (no rules)
    pub fn empty() -> Self {
        Self { rules: vec![] }
    }

    /// Match an element against all rules and return merged Style
    /// Priority: tag < class < id (later rules win, inline props override in layout.rs)
    pub fn match_element(&self, element: &Element) -> (Style, bool) {
        let mut style = Style::default();
        let mut matched = false;

        match &element.node_type {
            NodeType::Element(tag) => {
                // 1. Apply tag rules (e.g., "div", "button")
                for (selector_key, rule_style) in &self.rules {
                    if selector_key == tag {
                        merge_style(&mut style, rule_style);
                        matched = true;
                    }
                }

                // 2. Apply class rules (e.g., ".container", ".flex")
                for (selector_key, rule_style) in &self.rules {
                    if let Some(class_name) = selector_key.strip_prefix('.') {
                        // Check if element has this class
                        if element.props.iter().any(|(name, val)| {
                            name == "class"
                                && matches!(val, uwebr_core::component::PropValue::String(s) if s == class_name || s.split_whitespace().any(|c| c == class_name))
                        }) {
                            merge_style(&mut style, rule_style);
                            matched = true;
                        }
                    }
                }

                // 3. Apply id rules (e.g., "#main", "#header")
                for (selector_key, rule_style) in &self.rules {
                    if let Some(id_name) = selector_key.strip_prefix('#') {
                        if element.props.iter().any(|(name, val)| {
                            name == "id"
                                && matches!(val, uwebr_core::component::PropValue::String(s) if s == id_name)
                        }) {
                            merge_style(&mut style, rule_style);
                            matched = true;
                        }
                    }
                }
            }
            NodeType::Text(_) | NodeType::Component(_) | NodeType::Raw(_) => {}
        }

        (style, matched)
    }

    /// Number of rules in the book
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// Check if the stylebook is empty
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Get all selector keys (for debugging)
    pub fn selectors(&self) -> Vec<&str> {
        self.rules.iter().map(|(k, _)| k.as_str()).collect()
    }
}

/// Merge source style into target (overwrites target fields)
fn merge_style(target: &mut Style, source: &Style) {
    target.display = source.display;
    target.flex_direction = source.flex_direction;
    target.flex_wrap = source.flex_wrap;
    target.justify_content = source.justify_content;
    target.align_items = source.align_items;
    target.align_self = source.align_self;
    target.flex_grow = source.flex_grow;
    target.flex_shrink = source.flex_shrink;
    target.flex_basis = source.flex_basis;
    target.size = source.size;
    target.min_size = source.min_size;
    target.max_size = source.max_size;
    target.padding = source.padding;
    target.margin = source.margin;
    target.border = source.border;
    target.position = source.position;
    target.inset = source.inset;
    target.overflow = source.overflow;
    target.gap = source.gap;
}

#[cfg(test)]
mod tests {
    use super::*;
    use uwebr_core::component::PropValue;

    fn make_element(tag: &str, props: Vec<(String, PropValue)>) -> Element {
        Element {
            node_type: NodeType::Element(tag.to_string()),
            props,
            children: vec![],
        }
    }

    #[test]
    fn test_stylebook_parse() {
        let sb = StyleBook::parse(".box { width: 100px; }").unwrap();
        assert_eq!(sb.len(), 1);
        assert_eq!(sb.selectors(), vec![".box"]);
    }

    #[test]
    fn test_stylebook_empty() {
        let sb = StyleBook::empty();
        assert!(sb.is_empty());
    }

    #[test]
    fn test_match_tag() {
        let sb = StyleBook::parse("div { width: 200px; }").unwrap();
        let el = make_element("div", vec![]);
        let (style, matched) = sb.match_element(&el);
        assert!(matched);
        assert_eq!(style.size.width, taffy::Dimension::length(200.0));
    }

    #[test]
    fn test_match_class() {
        let sb = StyleBook::parse(".container { display: flex; flex-direction: row; }").unwrap();
        let el = make_element(
            "div",
            vec![("class".into(), PropValue::String("container".into()))],
        );
        let (style, matched) = sb.match_element(&el);
        assert!(matched);
        assert_eq!(style.display, taffy::Display::Flex);
        assert_eq!(style.flex_direction, taffy::FlexDirection::Row);
    }

    #[test]
    fn test_match_id() {
        let sb = StyleBook::parse("#main { padding: 16px; }").unwrap();
        let el = make_element(
            "div",
            vec![("id".into(), PropValue::String("main".into()))],
        );
        let (style, matched) = sb.match_element(&el);
        assert!(matched);
        assert_eq!(
            style.padding.top,
            taffy::LengthPercentage::length(16.0)
        );
    }

    #[test]
    fn test_match_no_rules() {
        let sb = StyleBook::empty();
        let el = make_element("div", vec![]);
        let (_style, matched) = sb.match_element(&el);
        assert!(!matched);
    }

    #[test]
    fn test_multiple_classes() {
        let sb = StyleBook::parse(".flex { display: flex; } .gap { padding: 8px; }").unwrap();
        let el = make_element(
            "div",
            vec![(
                "class".into(),
                PropValue::String("flex gap".into()),
            )],
        );
        let (style, matched) = sb.match_element(&el);
        assert!(matched);
        assert_eq!(style.display, taffy::Display::Flex);
        assert_eq!(style.padding.top, taffy::LengthPercentage::length(8.0));
    }

    #[test]
    fn test_priority_class_over_tag() {
        let sb = StyleBook::parse("div { width: 100px; } .wide { width: 300px; }").unwrap();
        let el = make_element(
            "div",
            vec![("class".into(), PropValue::String("wide".into()))],
        );
        let (style, matched) = sb.match_element(&el);
        assert!(matched);
        // Class rule should override tag rule
        assert_eq!(style.size.width, taffy::Dimension::length(300.0));
    }

    #[test]
    fn test_priority_id_over_class() {
        let sb = StyleBook::parse(".box { width: 100px; } #special { width: 500px; }").unwrap();
        let el = make_element(
            "div",
            vec![
                ("class".into(), PropValue::String("box".into())),
                ("id".into(), PropValue::String("special".into())),
            ],
        );
        let (style, matched) = sb.match_element(&el);
        assert!(matched);
        assert_eq!(style.size.width, taffy::Dimension::length(500.0));
    }
}
