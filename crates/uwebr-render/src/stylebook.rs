use taffy::Style;
use uwebr_core::component::{Element, NodeType, PropValue};
use uwebr_css::codegen::{convert_to_style_entries, PaintProps, StyleEntry, StyleMask};
use uwebr_css::parser::parse_css;

/// Result of matching an element against the stylesheet.
#[derive(Debug, Clone, Default)]
pub struct MatchedStyle {
    /// Layout properties for Taffy.
    pub style: Style,
    /// Which layout fields were actually specified by the matched rules.
    pub mask: StyleMask,
    /// Paint properties Taffy cannot represent (colours, fonts, …).
    pub paint: PaintProps,
    /// Whether any rule matched at all.
    pub matched: bool,
}

/// Parsed CSS stylesheet ready for layout matching
#[derive(Debug, Clone, Default)]
pub struct StyleBook {
    rules: Vec<StyleEntry>,
}

impl StyleBook {
    /// Parse a CSS string into a StyleBook
    pub fn parse(css: &str) -> anyhow::Result<Self> {
        let rules = parse_css(css)?;
        Ok(Self {
            rules: convert_to_style_entries(&rules)?,
        })
    }

    /// Create from pre-converted (selector, Style) pairs — layout only, no paint.
    pub fn from_rules(rules: Vec<(String, Style)>) -> Self {
        Self {
            rules: rules
                .into_iter()
                .map(|(selector, style)| StyleEntry {
                    selector,
                    style,
                    // Legacy callers give no mask information; treat every field
                    // as specified so behaviour matches the old merge_style().
                    mask: ALL_FIELDS_MASK,
                    paint: PaintProps::default(),
                })
                .collect(),
        }
    }

    /// Create from full style entries (layout + mask + paint).
    pub fn from_entries(rules: Vec<StyleEntry>) -> Self {
        Self { rules }
    }

    /// Empty stylebook (no rules)
    pub fn empty() -> Self {
        Self { rules: vec![] }
    }

    /// Match an element and return the merged layout Style plus a "matched" flag.
    ///
    /// Kept for callers that only care about layout.
    pub fn match_element(&self, element: &Element) -> (Style, bool) {
        let m = self.match_full(element);
        (m.style, m.matched)
    }

    /// Match an element against all rules, returning layout + mask + paint.
    ///
    /// Priority: tag < class < id. Only properties a rule actually declared are
    /// written, so a class rule setting just `width` no longer resets `display`
    /// or `padding` inherited from the tag rule.
    pub fn match_full(&self, element: &Element) -> MatchedStyle {
        let mut out = MatchedStyle::default();

        let tag = match &element.node_type {
            NodeType::Element(tag) => tag.as_str(),
            // Text/Component/Raw nodes have no selector of their own; paint
            // reaches them through inheritance in the pipeline.
            NodeType::Text(_) | NodeType::Component(_) | NodeType::Raw(_) => return out,
        };

        // 1. Tag rules (e.g. "div", "h1")
        for entry in &self.rules {
            if entry.selector == tag {
                self.absorb(&mut out, entry);
            }
        }

        // 2. Class rules (e.g. ".container")
        for entry in &self.rules {
            if let Some(class_name) = entry.selector.strip_prefix('.') {
                if element_has_class(element, class_name) {
                    self.absorb(&mut out, entry);
                }
            }
        }

        // 3. Id rules (e.g. "#main")
        for entry in &self.rules {
            if let Some(id_name) = entry.selector.strip_prefix('#') {
                if element_has_id(element, id_name) {
                    self.absorb(&mut out, entry);
                }
            }
        }

        out
    }

    fn absorb(&self, out: &mut MatchedStyle, entry: &StyleEntry) {
        merge_style(&mut out.style, &entry.style, &entry.mask);
        out.mask.or_assign(&entry.mask);
        out.paint.merge(&entry.paint);
        out.matched = true;
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
        self.rules.iter().map(|e| e.selector.as_str()).collect()
    }
}

fn element_has_class(element: &Element, class_name: &str) -> bool {
    element.props.iter().any(|(name, val)| {
        name == "class"
            && matches!(
                val,
                PropValue::String(s)
                    if s == class_name || s.split_whitespace().any(|c| c == class_name)
            )
    })
}

fn element_has_id(element: &Element, id_name: &str) -> bool {
    element
        .props
        .iter()
        .any(|(name, val)| name == "id" && matches!(val, PropValue::String(s) if s == id_name))
}

/// Every field marked as specified — used for legacy `from_rules` entries.
const ALL_FIELDS_MASK: StyleMask = StyleMask {
    display: true,
    flex_direction: true,
    flex_wrap: true,
    justify_content: true,
    align_items: true,
    align_self: true,
    flex_grow: true,
    flex_shrink: true,
    flex_basis: true,
    width: true,
    height: true,
    min_width: true,
    min_height: true,
    max_width: true,
    max_height: true,
    padding: true,
    margin: true,
    border: true,
    position: true,
    inset: true,
    overflow: true,
    gap_width: true,
    gap_height: true,
};

/// Merge `source` into `target`, writing only the fields flagged in `mask`.
fn merge_style(target: &mut Style, source: &Style, mask: &StyleMask) {
    if mask.display {
        target.display = source.display;
    }
    if mask.flex_direction {
        target.flex_direction = source.flex_direction;
    }
    if mask.flex_wrap {
        target.flex_wrap = source.flex_wrap;
    }
    if mask.justify_content {
        target.justify_content = source.justify_content;
    }
    if mask.align_items {
        target.align_items = source.align_items;
    }
    if mask.align_self {
        target.align_self = source.align_self;
    }
    if mask.flex_grow {
        target.flex_grow = source.flex_grow;
    }
    if mask.flex_shrink {
        target.flex_shrink = source.flex_shrink;
    }
    if mask.flex_basis {
        target.flex_basis = source.flex_basis;
    }
    if mask.width {
        target.size.width = source.size.width;
    }
    if mask.height {
        target.size.height = source.size.height;
    }
    if mask.min_width {
        target.min_size.width = source.min_size.width;
    }
    if mask.min_height {
        target.min_size.height = source.min_size.height;
    }
    if mask.max_width {
        target.max_size.width = source.max_size.width;
    }
    if mask.max_height {
        target.max_size.height = source.max_size.height;
    }
    if mask.padding {
        target.padding = source.padding;
    }
    if mask.margin {
        target.margin = source.margin;
    }
    if mask.border {
        target.border = source.border;
    }
    if mask.position {
        target.position = source.position;
    }
    if mask.inset {
        target.inset = source.inset;
    }
    if mask.overflow {
        target.overflow = source.overflow;
    }
    if mask.gap_width {
        target.gap.width = source.gap.width;
    }
    if mask.gap_height {
        target.gap.height = source.gap.height;
    }
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
        let el = make_element("div", vec![("id".into(), PropValue::String("main".into()))]);
        let (style, matched) = sb.match_element(&el);
        assert!(matched);
        assert_eq!(style.padding.top, taffy::LengthPercentage::length(16.0));
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
            vec![("class".into(), PropValue::String("flex gap".into()))],
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

    // ── Cascade regression tests (M3) ───────────────────────────

    #[test]
    fn test_class_rule_does_not_reset_tag_display() {
        // The old merge_style() assigned every field unconditionally, so a class
        // rule setting only `width` wiped the tag rule's display/flex-direction.
        let sb = StyleBook::parse(
            "div { display: flex; flex-direction: column; padding: 12px; } .wide { width: 300px; }",
        )
        .unwrap();
        let el = make_element(
            "div",
            vec![("class".into(), PropValue::String("wide".into()))],
        );
        let (style, _) = sb.match_element(&el);

        assert_eq!(style.size.width, taffy::Dimension::length(300.0));
        assert_eq!(
            style.display,
            taffy::Display::Flex,
            "tag display must survive the class rule"
        );
        assert_eq!(
            style.flex_direction,
            taffy::FlexDirection::Column,
            "tag flex-direction must survive"
        );
        assert_eq!(
            style.padding.top,
            taffy::LengthPercentage::length(12.0),
            "tag padding must survive"
        );
    }

    #[test]
    fn test_three_level_cascade_tag_class_id() {
        let sb = StyleBook::parse(
            "div { display: flex; flex-direction: column; padding: 4px; } \
             .card { width: 200px; } \
             #hero { height: 90px; }",
        )
        .unwrap();
        let el = make_element(
            "div",
            vec![
                ("class".into(), PropValue::String("card".into())),
                ("id".into(), PropValue::String("hero".into())),
            ],
        );
        let (style, _) = sb.match_element(&el);

        // Each level contributes its own property, none clobbers the others.
        assert_eq!(style.display, taffy::Display::Flex);
        assert_eq!(style.flex_direction, taffy::FlexDirection::Column);
        assert_eq!(style.padding.top, taffy::LengthPercentage::length(4.0));
        assert_eq!(style.size.width, taffy::Dimension::length(200.0));
        assert_eq!(style.size.height, taffy::Dimension::length(90.0));
    }

    #[test]
    fn test_later_level_overrides_same_property() {
        let sb = StyleBook::parse("div { padding: 4px; } .tight { padding: 1px; }").unwrap();
        let el = make_element(
            "div",
            vec![("class".into(), PropValue::String("tight".into()))],
        );
        let (style, _) = sb.match_element(&el);
        assert_eq!(style.padding.top, taffy::LengthPercentage::length(1.0));
    }

    #[test]
    fn test_width_only_rule_keeps_height_default() {
        let sb = StyleBook::parse(".w { width: 50px; }").unwrap();
        let el = make_element("div", vec![("class".into(), PropValue::String("w".into()))]);
        let (style, _) = sb.match_element(&el);
        let default: Style = Style::default();
        assert_eq!(style.size.height, default.size.height);
    }

    // ── Paint tests (M2) ────────────────────────────────────────

    #[test]
    fn test_match_full_returns_paint() {
        let sb = StyleBook::parse(".app { background-color: #1a1a2e; color: #e0e0e0; }").unwrap();
        let el = make_element(
            "div",
            vec![("class".into(), PropValue::String("app".into()))],
        );
        let m = sb.match_full(&el);
        assert!(m.matched);
        let bg = m.paint.background.clone().unwrap();
        assert_eq!((bg.r, bg.g, bg.b), (0x1a, 0x1a, 0x2e));
        let color = m.paint.color.clone().unwrap();
        assert_eq!((color.r, color.g, color.b), (0xe0, 0xe0, 0xe0));
    }

    #[test]
    fn test_paint_only_rule_still_counts_as_matched() {
        // A rule with just `color` sets no layout field; the element must still
        // be treated as matched so the paint is not dropped.
        let sb = StyleBook::parse("h1 { color: red; }").unwrap();
        let el = make_element("h1", vec![]);
        let m = sb.match_full(&el);
        assert!(m.matched);
        assert!(m.paint.color.is_some());
        assert!(m.mask.is_empty(), "no layout property was declared");
    }

    #[test]
    fn test_paint_font_size_from_css() {
        let sb = StyleBook::parse("h1 { font-size: 2rem; }").unwrap();
        let el = make_element("h1", vec![]);
        let m = sb.match_full(&el);
        assert_eq!(m.paint.font_size, Some(32.0));
    }

    #[test]
    fn test_paint_cascade_id_over_class() {
        let sb = StyleBook::parse(".a { color: red; } #b { color: blue; }").unwrap();
        let el = make_element(
            "div",
            vec![
                ("class".into(), PropValue::String("a".into())),
                ("id".into(), PropValue::String("b".into())),
            ],
        );
        let m = sb.match_full(&el);
        let c = m.paint.color.unwrap();
        assert_eq!((c.r, c.g, c.b), (0, 0, 255));
    }

    #[test]
    fn test_paint_merge_preserves_unspecified() {
        let sb = StyleBook::parse(
            ".base { background-color: #112233; color: red; } .over { color: blue; }",
        )
        .unwrap();
        let el = make_element(
            "div",
            vec![("class".into(), PropValue::String("base over".into()))],
        );
        let m = sb.match_full(&el);
        let bg = m.paint.background.unwrap();
        assert_eq!((bg.r, bg.g, bg.b), (0x11, 0x22, 0x33));
        let c = m.paint.color.unwrap();
        assert_eq!((c.r, c.g, c.b), (0, 0, 255));
    }

    #[test]
    fn test_text_node_matches_nothing() {
        let sb = StyleBook::parse("div { width: 10px; }").unwrap();
        let text = Element {
            node_type: NodeType::Text("hi".into()),
            props: vec![],
            children: vec![],
        };
        let m = sb.match_full(&text);
        assert!(!m.matched);
    }

    #[test]
    fn test_from_rules_legacy_behaves_as_before() {
        // Legacy entries have no mask, so all fields apply (old semantics).
        let style: Style = Style {
            display: taffy::Display::Flex,
            ..Default::default()
        };
        let sb = StyleBook::from_rules(vec![("div".to_string(), style)]);
        let el = make_element("div", vec![]);
        let (out, matched) = sb.match_element(&el);
        assert!(matched);
        assert_eq!(out.display, taffy::Display::Flex);
    }
}
