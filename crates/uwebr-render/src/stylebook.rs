use taffy::Style;
use uwebr_core::component::{Element, NodeType, PropValue};
use uwebr_css::ast::{AttributeOp, CssSelector};
use uwebr_css::codegen::{
    convert_to_style_entries, convert_to_style_entries_vp, PaintProps, StyleEntry, StyleMask,
};
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

    /// Parse CSS, resolving `vw`/`vh` against the given viewport dimensions.
    pub fn parse_vp(css: &str, vw: f32, vh: f32) -> anyhow::Result<Self> {
        let rules = parse_css(css)?;
        Ok(Self {
            rules: convert_to_style_entries_vp(&rules, vw, vh)?,
        })
    }

    /// Re-parse CSS in place with new viewport dimensions.
    ///
    /// Called on resize so `vw`/`vh` track the window without rebuilding the
    /// whole pipeline.
    pub fn reparse(&mut self, css: &str, vw: f32, vh: f32) -> anyhow::Result<()> {
        let rules = parse_css(css)?;
        self.rules = convert_to_style_entries_vp(&rules, vw, vh)?;
        Ok(())
    }

    /// Create from pre-converted (selector, Style) pairs — layout only, no paint.
    pub fn from_rules(rules: Vec<(String, Style)>) -> Self {
        Self {
            rules: rules
                .into_iter()
                .map(|(selector, style)| StyleEntry {
                    selector,
                    selector_ast: None,
                    style,
                    // Legacy callers give no mask information; treat every field
                    // as specified so behaviour matches the old merge_style().
                    mask: ALL_FIELDS_MASK,
                    paint: PaintProps::default(),
                    important: false,
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
    /// Kept for callers that only care about layout. Uses an empty parent chain
    /// and node id 0 (stateful pseudo-classes will not match).
    pub fn match_element(&self, element: &Element) -> (Style, bool) {
        let m = self.match_full(element, &[], 0);
        (m.style, m.matched)
    }

    /// Match an element against all rules, returning layout + mask + paint.
    ///
    /// Priority: tag < class < id. Only properties a rule actually declared are
    /// written, so a class rule setting just `width` no longer resets `display`
    /// or `padding` inherited from the tag rule.
    ///
    /// `parent_chain[0]` is the immediate parent, `[1]` the grandparent, etc.,
    /// used to resolve descendant/child combinators. `node_id` is the layout
    /// tree's pre-order index, used to look up runtime hover/focus state.
    pub fn match_full(
        &self,
        element: &Element,
        parent_chain: &[&Element],
        node_id: usize,
    ) -> MatchedStyle {
        let mut out = MatchedStyle::default();

        let tag = match &element.node_type {
            NodeType::Element(tag) => tag.as_str(),
            // Text/Component/Raw nodes have no selector of their own; paint
            // reaches them through inheritance in the pipeline.
            NodeType::Text(_) | NodeType::Component(_) | NodeType::Raw(_) => return out,
        };

        // Apply matching rules in ascending priority order so higher-priority
        // rules overwrite lower ones. Priority key is (important, specificity,
        // source order): an `!important` rule beats any normal rule regardless
        // of specificity, matching the CSS cascade.
        let mut matches: Vec<(u8, u32, usize, &StyleEntry)> = Vec::new();
        for (idx, entry) in self.rules.iter().enumerate() {
            let matched = match &entry.selector_ast {
                Some(ast) => selector_matches(ast, element, tag, parent_chain, node_id),
                None => self.string_selector_matches(&entry.selector, element, tag),
            };
            if matched {
                let spec = entry
                    .selector_ast
                    .as_ref()
                    .map(selector_specificity)
                    .unwrap_or(0);
                matches.push((entry.important as u8, spec, idx, entry));
            }
        }

        // Stable sort by (important, specificity, source order): equal keys keep
        // declaration order, matching the CSS cascade.
        matches.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(a.1.cmp(&b.1))
                .then(a.2.cmp(&b.2))
        });

        for (_, _, _, entry) in matches {
            self.absorb(&mut out, entry);
        }

        out
    }

    /// Legacy string-key matching for entries without a parsed selector AST.
    fn string_selector_matches(&self, selector: &str, element: &Element, tag: &str) -> bool {
        if selector == tag || selector == "*" {
            return true;
        }
        if let Some(class_name) = selector.strip_prefix('.') {
            return element_has_class(element, class_name);
        }
        if let Some(id_name) = selector.strip_prefix('#') {
            return element_has_id(element, id_name);
        }
        false
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

/// Recursively test whether a selector matches an element.
///
/// Descendant/child combinators walk the real `parent_chain`: `parent_chain[0]`
/// is the immediate parent, `[1]` the grandparent, and so on. `node_id` is the
/// layout tree's pre-order index, used by stateful pseudo-classes to look up
/// hover/focus state.
fn selector_matches(
    sel: &CssSelector,
    element: &Element,
    tag: &str,
    parent_chain: &[&Element],
    node_id: usize,
) -> bool {
    match sel {
        CssSelector::Tag(t) => t == tag,
        CssSelector::Class(c) => element_has_class(element, c),
        CssSelector::Id(id) => element_has_id(element, id),
        CssSelector::Universal => true,
        CssSelector::PseudoClass(inner, pseudo) => {
            selector_matches(inner, element, tag, parent_chain, node_id)
                && pseudo_class_matches(pseudo, element, parent_chain, node_id)
        }
        CssSelector::Attribute {
            selector: inner,
            attr,
            op,
            value,
        } => {
            selector_matches(inner, element, tag, parent_chain, node_id)
                && attribute_matches(element, attr, op, value.as_deref())
        }
        // `.a .b`: the subject (rightmost) must match this element, and each
        // ancestor selector must match *some* ancestor further up the chain,
        // in order.
        CssSelector::Descendant(selectors) => {
            let Some(subject) = selectors.last() else {
                return false;
            };
            if !selector_matches(subject, element, tag, parent_chain, node_id) {
                return false;
            }
            let ancestors = &selectors[..selectors.len() - 1];
            ancestors_match(ancestors, parent_chain, false)
        }
        // `.a > .b`: the subject must match this element and each ancestor
        // selector must match the *immediately* preceding parent.
        CssSelector::Child(selectors) => {
            let Some(subject) = selectors.last() else {
                return false;
            };
            if !selector_matches(subject, element, tag, parent_chain, node_id) {
                return false;
            }
            let ancestors = &selectors[..selectors.len() - 1];
            ancestors_match(ancestors, parent_chain, true)
        }
        CssSelector::List(sels) => sels
            .iter()
            .any(|s| selector_matches(s, element, tag, parent_chain, node_id)),
    }
}

/// Test the ancestor part of a combinator against the parent chain.
///
/// `ancestors` are in document order (outermost … innermost parent); they are
/// walked from the innermost outward. When `direct` is true (child combinator)
/// each step must match the very next parent; otherwise (descendant combinator)
/// a matching ancestor may be found anywhere further up.
fn ancestors_match(ancestors: &[CssSelector], parent_chain: &[&Element], direct: bool) -> bool {
    let mut depth = 0usize;
    // Walk ancestor selectors from innermost (last) to outermost (first).
    for ancestor_sel in ancestors.iter().rev() {
        let mut matched = false;
        while depth < parent_chain.len() {
            let ancestor = parent_chain[depth];
            let a_tag = match &ancestor.node_type {
                NodeType::Element(t) => t.as_str(),
                _ => {
                    depth += 1;
                    if direct {
                        return false;
                    }
                    continue;
                }
            };
            // Ancestors carry no node_id of interest here (stateful pseudo on an
            // ancestor selector is uncommon); pass 0 and its own remaining chain.
            let rest = &parent_chain[depth + 1..];
            if selector_matches(ancestor_sel, ancestor, a_tag, rest, usize::MAX) {
                depth += 1;
                matched = true;
                break;
            }
            if direct {
                // Child combinator: the immediate parent must match.
                return false;
            }
            depth += 1;
        }
        if !matched {
            return false;
        }
    }
    true
}

/// CSS specificity as a single sortable integer: (id, class/attr/pseudo, tag).
///
/// Packed as `id * 10000 + (class+attr+pseudo) * 100 + tag` so that a higher
/// value always wins, matching the cascade ordering used by browsers.
fn selector_specificity(sel: &CssSelector) -> u32 {
    fn count(sel: &CssSelector, ids: &mut u32, classes: &mut u32, tags: &mut u32) {
        match sel {
            CssSelector::Id(_) => *ids += 1,
            CssSelector::Class(_) => *classes += 1,
            CssSelector::Tag(_) => *tags += 1,
            CssSelector::Universal => {}
            CssSelector::PseudoClass(inner, _) => {
                *classes += 1;
                count(inner, ids, classes, tags);
            }
            CssSelector::Attribute { selector, .. } => {
                *classes += 1;
                count(selector, ids, classes, tags);
            }
            CssSelector::Descendant(sels) | CssSelector::Child(sels) => {
                for s in sels {
                    count(s, ids, classes, tags);
                }
            }
            // A list matches on any branch; specificity is taken as the max.
            CssSelector::List(sels) => {
                let mut best = 0;
                for s in sels {
                    best = best.max(selector_specificity(s));
                }
                // Fold the best branch back into the counters via the packed form.
                *ids += best / 10000;
                *classes += (best / 100) % 100;
                *tags += best % 100;
            }
        }
    }

    let (mut ids, mut classes, mut tags) = (0, 0, 0);
    count(sel, &mut ids, &mut classes, &mut tags);
    ids * 10000 + classes * 100 + tags
}

/// Match a pseudo-class name against an element.
///
/// Stateless structural pseudo-classes that need a parent's child list
/// (`:first-child`, …) are simplified to always match. Stateful ones
/// (`:hover`, `:focus`) consult the runtime [`ElementStateStore`] via `node_id`.
/// `:disabled` / `:enabled` are checked against the element's props.
fn pseudo_class_matches(
    pseudo: &str,
    element: &Element,
    parent_chain: &[&Element],
    node_id: usize,
) -> bool {
    match pseudo {
        // Structural — real matching needs the parent's child list (future phase).
        "first-child" | "last-child" | "only-child" | "nth-child" | "nth-of-type" => true,
        // Stateful — resolved against runtime interaction state.
        "hover" => uwebr_core::state::is_hovered(node_id),
        "focus" | "focus-visible" => uwebr_core::state::is_focused(node_id),
        // A tabbed-into ancestor: any element focused counts (approximation —
        // we lack per-subtree focus tracking, so a focused element anywhere
        // satisfies :focus-within only when there is an ancestor at all).
        "focus-within" => {
            uwebr_core::state::is_focused(node_id)
                || (!parent_chain.is_empty() && uwebr_core::state::any_focused())
        }
        // `:active` = mouse-down instant; `:visited` = browsing history. Neither
        // is tracked in a desktop render loop.
        "active" | "visited" => false,
        "disabled" => is_disabled(element),
        "enabled" => !is_disabled(element),
        "checked" => element
            .props
            .iter()
            .any(|(k, v)| k == "checked" && matches!(v, PropValue::Bool(true))),
        _ => false,
    }
}

/// Whether an element carries a truthy `disabled` attribute.
fn is_disabled(element: &Element) -> bool {
    element.props.iter().any(|(k, v)| {
        k == "disabled"
            && match v {
                PropValue::Bool(b) => *b,
                PropValue::String(s) => s != "false",
                _ => false,
            }
    })
}

/// Match an attribute selector against an element's props.
fn attribute_matches(element: &Element, attr: &str, op: &AttributeOp, value: Option<&str>) -> bool {
    let attr_value = element
        .props
        .iter()
        .find(|(k, _)| k == attr)
        .and_then(|(_, v)| match v {
            PropValue::String(s) => Some(s.as_str()),
            PropValue::Bool(true) => Some(""),
            _ => None,
        });

    match op {
        AttributeOp::Exists => attr_value.is_some(),
        AttributeOp::Equals => attr_value == value,
        AttributeOp::Includes => {
            attr_value.is_some_and(|v| v.split_whitespace().any(|w| w == value.unwrap_or("")))
        }
        AttributeOp::Prefix => attr_value.is_some_and(|v| v.starts_with(value.unwrap_or(""))),
        AttributeOp::Suffix => attr_value.is_some_and(|v| v.ends_with(value.unwrap_or(""))),
        AttributeOp::Contains => attr_value.is_some_and(|v| v.contains(value.unwrap_or(""))),
    }
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
        let m = sb.match_full(&el, &[], 0);
        assert!(m.matched);
        let bg = m.paint.background.clone().unwrap();
        match bg {
            uwebr_css::codegen::BackgroundValue::Solid(c) => {
                assert_eq!((c.r, c.g, c.b), (0x1a, 0x1a, 0x2e))
            }
            other => panic!("expected solid background, got {other:?}"),
        }
        let color = m.paint.color.clone().unwrap();
        assert_eq!((color.r, color.g, color.b), (0xe0, 0xe0, 0xe0));
    }

    #[test]
    fn test_paint_only_rule_still_counts_as_matched() {
        // A rule with just `color` sets no layout field; the element must still
        // be treated as matched so the paint is not dropped.
        let sb = StyleBook::parse("h1 { color: red; }").unwrap();
        let el = make_element("h1", vec![]);
        let m = sb.match_full(&el, &[], 0);
        assert!(m.matched);
        assert!(m.paint.color.is_some());
        assert!(m.mask.is_empty(), "no layout property was declared");
    }

    #[test]
    fn test_paint_font_size_from_css() {
        let sb = StyleBook::parse("h1 { font-size: 2rem; }").unwrap();
        let el = make_element("h1", vec![]);
        let m = sb.match_full(&el, &[], 0);
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
        let m = sb.match_full(&el, &[], 0);
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
        let m = sb.match_full(&el, &[], 0);
        let bg = m.paint.background.unwrap();
        match bg {
            uwebr_css::codegen::BackgroundValue::Solid(c) => {
                assert_eq!((c.r, c.g, c.b), (0x11, 0x22, 0x33))
            }
            other => panic!("expected solid background, got {other:?}"),
        }
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
        let m = sb.match_full(&text, &[], 0);
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

    // ── Viewport reparse (vw/vh) ────────────────────────────────

    #[test]
    fn test_reparse_resolves_vw_against_new_viewport() {
        let mut sb = StyleBook::parse_vp(".w { width: 50vw; }", 800.0, 600.0).unwrap();
        let el = make_element("div", vec![("class".into(), PropValue::String("w".into()))]);
        let (style, _) = sb.match_element(&el);
        assert_eq!(style.size.width, taffy::Dimension::length(400.0));

        // Resize: 50vw of 1000px = 500px.
        sb.reparse(".w { width: 50vw; }", 1000.0, 600.0).unwrap();
        let (style, _) = sb.match_element(&el);
        assert_eq!(style.size.width, taffy::Dimension::length(500.0));
    }

    // ── Pseudo-class / attribute selector matching (FAZ 13) ─────

    #[test]
    fn test_pseudo_hover_not_applied_without_state() {
        // :hover is stateful; with no runtime tracking it never matches yet.
        let sb = StyleBook::parse(".btn:hover { background-color: blue; }").unwrap();
        let el = make_element(
            "button",
            vec![("class".into(), PropValue::String("btn".into()))],
        );
        let m = sb.match_full(&el, &[], 0);
        assert!(
            m.paint.background.is_none(),
            ":hover must not apply without hover state"
        );
    }

    #[test]
    fn test_pseudo_disabled_applies_to_disabled_element() {
        let sb = StyleBook::parse("button:disabled { opacity: 0.5; }").unwrap();
        let el = make_element("button", vec![("disabled".into(), PropValue::Bool(true))]);
        let m = sb.match_full(&el, &[], 0);
        assert_eq!(m.paint.opacity, Some(0.5), "disabled element must match");
    }

    #[test]
    fn test_pseudo_disabled_skips_enabled_element() {
        let sb = StyleBook::parse("button:disabled { opacity: 0.5; }").unwrap();
        let el = make_element("button", vec![]);
        let m = sb.match_full(&el, &[], 0);
        assert!(
            m.paint.opacity.is_none(),
            "enabled element must not match :disabled"
        );
    }

    #[test]
    fn test_attribute_exists_matches() {
        let sb = StyleBook::parse("[disabled] { opacity: 0.5; }").unwrap();
        let el = make_element("input", vec![("disabled".into(), PropValue::Bool(true))]);
        let m = sb.match_full(&el, &[], 0);
        assert_eq!(m.paint.opacity, Some(0.5));
    }

    #[test]
    fn test_attribute_equals_matches_specific_value() {
        let sb = StyleBook::parse(r#"input[type="text"] { border-width: 1px; }"#).unwrap();
        let text_input = make_element(
            "input",
            vec![("type".into(), PropValue::String("text".into()))],
        );
        let checkbox = make_element(
            "input",
            vec![("type".into(), PropValue::String("checkbox".into()))],
        );

        assert!(
            sb.match_full(&text_input, &[], 0).paint.border_width.is_some(),
            "type=text must match"
        );
        assert!(
            sb.match_full(&checkbox, &[], 0).paint.border_width.is_none(),
            "type=checkbox must not match"
        );
    }

    #[test]
    fn test_attribute_contains_matches_substring() {
        let sb = StyleBook::parse(r#"[class*="btn"] { opacity: 0.9; }"#).unwrap();
        let el = make_element(
            "div",
            vec![("class".into(), PropValue::String("my-btn-primary".into()))],
        );
        let m = sb.match_full(&el, &[], 0);
        assert_eq!(m.paint.opacity, Some(0.9));
    }

    #[test]
    fn test_attribute_prefix_and_suffix() {
        let prefix = StyleBook::parse(r#"[href^="https"] { opacity: 0.8; }"#).unwrap();
        let secure = make_element(
            "a",
            vec![("href".into(), PropValue::String("https://x.com".into()))],
        );
        let insecure = make_element(
            "a",
            vec![("href".into(), PropValue::String("http://x.com".into()))],
        );
        assert!(prefix.match_full(&secure, &[], 0).paint.opacity.is_some());
        assert!(prefix.match_full(&insecure, &[], 0).paint.opacity.is_none());

        let suffix = StyleBook::parse(r#"[src$=".png"] { opacity: 0.7; }"#).unwrap();
        let png = make_element(
            "img",
            vec![("src".into(), PropValue::String("a.png".into()))],
        );
        let jpg = make_element(
            "img",
            vec![("src".into(), PropValue::String("a.jpg".into()))],
        );
        assert!(suffix.match_full(&png, &[], 0).paint.opacity.is_some());
        assert!(suffix.match_full(&jpg, &[], 0).paint.opacity.is_none());
    }

    #[test]
    fn test_id_specificity_beats_attribute() {
        // #x wins over [data-x] regardless of source order.
        let sb =
            StyleBook::parse(r#"[data-x="1"] { color: red; } #hero { color: blue; }"#).unwrap();
        let el = make_element(
            "div",
            vec![
                ("data-x".into(), PropValue::String("1".into())),
                ("id".into(), PropValue::String("hero".into())),
            ],
        );
        let c = sb.match_full(&el, &[], 0).paint.color.unwrap();
        assert_eq!((c.r, c.g, c.b), (0, 0, 255), "id rule must win");
    }

    // ── Stateful pseudo-classes (FAZ 14) ────────────────────────

    #[test]
    fn test_pseudo_class_hover_matches() {
        uwebr_core::state::clear_element_state();
        let sb = StyleBook::parse(".btn:hover { background-color: blue; }").unwrap();
        let el = make_element(
            "button",
            vec![("class".into(), PropValue::String("btn".into()))],
        );

        // Not hovered → no match.
        assert!(sb.match_full(&el, &[], 7).paint.background.is_none());

        // Mark node 7 hovered → :hover applies.
        uwebr_core::state::set_hovered(7, true);
        assert!(
            sb.match_full(&el, &[], 7).paint.background.is_some(),
            ":hover must apply when the node is hovered"
        );
        // A different node id is unaffected.
        assert!(sb.match_full(&el, &[], 8).paint.background.is_none());
        uwebr_core::state::clear_element_state();
    }

    #[test]
    fn test_pseudo_class_focus_matches() {
        uwebr_core::state::clear_element_state();
        let sb = StyleBook::parse("input:focus { border-width: 2px; }").unwrap();
        let el = make_element("input", vec![]);

        assert!(sb.match_full(&el, &[], 3).paint.border_width.is_none());
        uwebr_core::state::set_focused(Some(3));
        assert_eq!(sb.match_full(&el, &[], 3).paint.border_width, Some(2.0));
        assert!(sb.match_full(&el, &[], 4).paint.border_width.is_none());
        uwebr_core::state::clear_element_state();
    }

    // ── Descendant / child combinators (FAZ 14) ─────────────────

    #[test]
    fn test_descendant_selector_real_match() {
        let sb = StyleBook::parse(".parent .child { color: red; }").unwrap();
        let child = make_element(
            "span",
            vec![("class".into(), PropValue::String("child".into()))],
        );
        let parent = make_element(
            "div",
            vec![("class".into(), PropValue::String("parent".into()))],
        );

        // With .parent as an ancestor → match.
        assert!(sb.match_full(&child, &[&parent], 1).paint.color.is_some());
        // No ancestor → no match.
        assert!(sb.match_full(&child, &[], 1).paint.color.is_none());
    }

    #[test]
    fn test_descendant_matches_deep_ancestor() {
        let sb = StyleBook::parse(".parent .child { color: red; }").unwrap();
        let child = make_element(
            "span",
            vec![("class".into(), PropValue::String("child".into()))],
        );
        let mid = make_element("div", vec![]);
        let parent = make_element(
            "div",
            vec![("class".into(), PropValue::String("parent".into()))],
        );

        // .parent is the grandparent — descendant combinator still matches.
        assert!(sb
            .match_full(&child, &[&mid, &parent], 2)
            .paint
            .color
            .is_some());
    }

    #[test]
    fn test_child_selector_direct_only() {
        let sb = StyleBook::parse("div > .btn { color: red; }").unwrap();
        let btn = make_element(
            "span",
            vec![("class".into(), PropValue::String("btn".into()))],
        );
        let direct_parent = make_element("div", vec![]);
        let grandparent = make_element("div", vec![]);
        let intermediate = make_element("section", vec![]);

        // Direct div parent → match.
        assert!(sb
            .match_full(&btn, &[&direct_parent], 1)
            .paint
            .color
            .is_some());
        // div only as grandparent (immediate parent is <section>) → no match.
        assert!(sb
            .match_full(&btn, &[&intermediate, &grandparent], 2)
            .paint
            .color
            .is_none());
    }

    #[test]
    fn test_descendant_no_match_nested_wrong() {
        let sb = StyleBook::parse(".unrelated .child { color: red; }").unwrap();
        let child = make_element(
            "span",
            vec![("class".into(), PropValue::String("child".into()))],
        );
        let parent = make_element(
            "div",
            vec![("class".into(), PropValue::String("parent".into()))],
        );
        // Ancestor class does not match the selector → no match.
        assert!(sb.match_full(&child, &[&parent], 1).paint.color.is_none());
    }

    // ── !important cascade (FAZ 14) ─────────────────────────────

    #[test]
    fn test_important_wins_over_higher_specificity() {
        // `.a` is important, `#id` has higher specificity but is normal.
        let sb =
            StyleBook::parse(".a { color: red !important; } #id { color: blue; }").unwrap();
        let el = make_element(
            "div",
            vec![
                ("class".into(), PropValue::String("a".into())),
                ("id".into(), PropValue::String("id".into())),
            ],
        );
        let c = sb.match_full(&el, &[], 0).paint.color.unwrap();
        assert_eq!((c.r, c.g, c.b), (255, 0, 0), "!important must win");
    }

    #[test]
    fn test_important_equal_specificity_last_wins() {
        // Two important rules, same specificity → later source order wins.
        let sb = StyleBook::parse(
            ".a { color: red !important; } .b { color: green !important; }",
        )
        .unwrap();
        let el = make_element(
            "div",
            vec![("class".into(), PropValue::String("a b".into()))],
        );
        let c = sb.match_full(&el, &[], 0).paint.color.unwrap();
        assert_eq!((c.r, c.g, c.b), (0, 128, 0), "later !important wins");
    }
}
