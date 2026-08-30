use taffy::Style;
use uwebr_core::component::{Element, NodeType, PropValue};
use uwebr_css::ast::{AttributeOp, CssSelector, NthKind};
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
        matches.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));

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
        CssSelector::Nth {
            selector: inner,
            kind,
            argument,
        } => {
            selector_matches(inner, element, tag, parent_chain, node_id)
                && nth_matches(kind, argument, element, parent_chain)
        }
        CssSelector::Not {
            selector: outer,
            inner,
        } => {
            selector_matches(outer, element, tag, parent_chain, node_id)
                && !selector_matches(inner, element, tag, parent_chain, node_id)
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
            CssSelector::Nth { selector, .. } | CssSelector::Not { selector, .. } => {
                *classes += 1;
                count(selector, ids, classes, tags);
                if let CssSelector::Not { inner, .. } = sel {
                    count(inner, ids, classes, tags);
                }
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
/// Structural pseudo-classes (`:first-child`, `:nth-child`, etc.) are now
/// handled by `nth_matches` via the `CssSelector::Nth` AST variant.
/// This function only handles stateful pseudo-classes (`:hover`, `:focus`)
/// and attribute-based ones (`:disabled`, `:checked`).
fn pseudo_class_matches(
    pseudo: &str,
    element: &Element,
    _parent_chain: &[&Element],
    node_id: usize,
) -> bool {
    match pseudo {
        "hover" => uwebr_core::state::is_hovered(node_id),
        "focus" | "focus-visible" => uwebr_core::state::is_focused(node_id),
        "focus-within" => {
            uwebr_core::state::is_focused(node_id)
                || (!_parent_chain.is_empty() && uwebr_core::state::any_focused())
        }
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

/// Match a structural `:nth-*` pseudo-class against an element.
fn nth_matches(
    kind: &NthKind,
    argument: &Option<String>,
    element: &Element,
    parent_chain: &[&Element],
) -> bool {
    let parent = match parent_chain.first() {
        Some(p) => p,
        None => return kind == &NthKind::Empty,
    };
    let tag = match &element.node_type {
        NodeType::Element(t) => t.as_str(),
        _ => return false,
    };
    match kind {
        NthKind::Empty => element.children.is_empty(),
        NthKind::FirstChild => {
            if argument.is_some() {
                // :nth-child(An+B) — compute position among all children.
                let index = parent
                    .children
                    .iter()
                    .take_while(|c| !std::ptr::eq(*c as *const _, element as *const _))
                    .count()
                    + 1;
                matches_an_plus_b(argument, index)
            } else {
                // :first-child — simply check if first.
                parent
                    .children
                    .first()
                    .is_some_and(|f| std::ptr::eq(f as *const _, element as *const _))
            }
        }
        NthKind::LastChild => {
            if argument.is_some() {
                // :nth-last-child(An+B) — compute reverse position.
                let index = parent
                    .children
                    .iter()
                    .rev()
                    .take_while(|c| !std::ptr::eq(*c as *const _, element as *const _))
                    .count()
                    + 1;
                matches_an_plus_b(argument, index)
            } else {
                // :last-child — simply check if last.
                parent
                    .children
                    .last()
                    .is_some_and(|l| std::ptr::eq(l as *const _, element as *const _))
            }
        }
        NthKind::FirstOfType => {
            if argument.is_some() {
                // :nth-first-of-type(An+B) — compute position among same-type.
                let index = parent
                    .children
                    .iter()
                    .filter(|c| matches!(&c.node_type, NodeType::Element(t) if t == tag))
                    .take_while(|c| !std::ptr::eq(*c as *const _, element as *const _))
                    .count()
                    + 1;
                matches_an_plus_b(argument, index)
            } else {
                // :first-of-type — simply check if first of same type.
                parent
                    .children
                    .iter()
                    .find(|c| matches!(&c.node_type, NodeType::Element(t) if t == tag))
                    .is_some_and(|f| std::ptr::eq(f as *const _, element as *const _))
            }
        }
        NthKind::LastOfType => {
            if argument.is_some() {
                // :nth-last-of-type(An+B) — compute reverse position among same-type.
                let index = parent
                    .children
                    .iter()
                    .rev()
                    .filter(|c| matches!(&c.node_type, NodeType::Element(t) if t == tag))
                    .take_while(|c| !std::ptr::eq(*c as *const _, element as *const _))
                    .count()
                    + 1;
                matches_an_plus_b(argument, index)
            } else {
                parent
                    .children
                    .iter()
                    .rfind(|c| matches!(&c.node_type, NodeType::Element(t) if t == tag))
                    .is_some_and(|l| std::ptr::eq(l as *const _, element as *const _))
            }
        }
        NthKind::OfType => {
            let arg = match argument {
                Some(a) => a,
                None => return false,
            };
            let index = parent
                .children
                .iter()
                .filter(|c| matches!(&c.node_type, NodeType::Element(t) if t == tag))
                .take_while(|c| !std::ptr::eq(*c as *const _, element as *const _))
                .count()
                + 1;
            match uwebr_css::parser::parse_nth(arg) {
                Some((a, b)) => {
                    if a == 0 {
                        index as i32 == b
                    } else {
                        let diff = index as i32 - b;
                        diff % a == 0 && diff / a >= 0
                    }
                }
                None => true,
            }
        }
    }
}

/// Check if `index` matches the An+B formula from `argument`.
fn matches_an_plus_b(argument: &Option<String>, index: usize) -> bool {
    let arg = match argument {
        Some(a) => a,
        None => return true,
    };
    match uwebr_css::parser::parse_nth(arg) {
        Some((a, b)) => {
            if a == 0 {
                index as i32 == b
            } else {
                let diff = index as i32 - b;
                diff % a == 0 && diff / a >= 0
            }
        }
        None => true,
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
            sb.match_full(&text_input, &[], 0)
                .paint
                .border_width
                .is_some(),
            "type=text must match"
        );
        assert!(
            sb.match_full(&checkbox, &[], 0)
                .paint
                .border_width
                .is_none(),
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
        let sb = StyleBook::parse(".a { color: red !important; } #id { color: blue; }").unwrap();
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
        let sb = StyleBook::parse(".a { color: red !important; } .b { color: green !important; }")
            .unwrap();
        let el = make_element(
            "div",
            vec![("class".into(), PropValue::String("a b".into()))],
        );
        let c = sb.match_full(&el, &[], 0).paint.color.unwrap();
        assert_eq!((c.r, c.g, c.b), (0, 128, 0), "later !important wins");
    }

    // ── Structural pseudo-classes (FAZ 15) ──────────────────────

    fn make_el(tag: &str, props: Vec<(String, PropValue)>, children: Vec<Element>) -> Element {
        Element {
            node_type: NodeType::Element(tag.to_string()),
            props,
            children,
        }
    }

    #[test]
    fn test_first_child_matches() {
        let sb = StyleBook::parse("li:first-child { color: red; }").unwrap();
        let first = make_el("li", vec![], vec![]);
        let second = make_el("li", vec![], vec![]);
        let parent = make_el("ul", vec![], vec![first, second]);
        let children: Vec<&Element> = parent.children.iter().collect();

        // First child matches.
        assert!(
            sb.match_full(children[0], &[&parent], 0)
                .paint
                .color
                .is_some(),
            "first-child must match first element"
        );
        // Second child does not.
        assert!(
            sb.match_full(children[1], &[&parent], 1)
                .paint
                .color
                .is_none(),
            "first-child must not match second element"
        );
    }

    #[test]
    fn test_last_child_matches() {
        let sb = StyleBook::parse("li:last-child { color: blue; }").unwrap();
        let first = make_el("li", vec![], vec![]);
        let last = make_el("li", vec![], vec![]);
        let parent = make_el("ul", vec![], vec![first, last]);
        let children: Vec<&Element> = parent.children.iter().collect();

        assert!(
            sb.match_full(children[0], &[&parent], 0)
                .paint
                .color
                .is_none(),
            "last-child must not match first element"
        );
        assert!(
            sb.match_full(children[1], &[&parent], 1)
                .paint
                .color
                .is_some(),
            "last-child must match last element"
        );
    }

    #[test]
    fn test_first_of_type_matches() {
        let sb = StyleBook::parse("span:first-of-type { color: green; }").unwrap();
        let div_child = make_el("div", vec![], vec![]);
        let span1 = make_el("span", vec![], vec![]);
        let span2 = make_el("span", vec![], vec![]);
        let parent = make_el("div", vec![], vec![div_child, span1, span2]);
        let children: Vec<&Element> = parent.children.iter().collect();

        // div is not a span → no match.
        assert!(sb
            .match_full(children[0], &[&parent], 0)
            .paint
            .color
            .is_none());
        // First span matches.
        assert!(
            sb.match_full(children[1], &[&parent], 1)
                .paint
                .color
                .is_some(),
            "first-of-type must match first span"
        );
        // Second span does not.
        assert!(
            sb.match_full(children[2], &[&parent], 2)
                .paint
                .color
                .is_none(),
            "first-of-type must not match second span"
        );
    }

    #[test]
    fn test_last_of_type_matches() {
        let sb = StyleBook::parse("span:last-of-type { color: orange; }").unwrap();
        let span1 = make_el("span", vec![], vec![]);
        let span2 = make_el("span", vec![], vec![]);
        let div_child = make_el("div", vec![], vec![]);
        let parent = make_el("div", vec![], vec![span1, span2, div_child]);
        let children: Vec<&Element> = parent.children.iter().collect();

        assert!(sb
            .match_full(children[0], &[&parent], 0)
            .paint
            .color
            .is_none());
        assert!(
            sb.match_full(children[1], &[&parent], 1)
                .paint
                .color
                .is_some(),
            "last-of-type must match second span"
        );
        assert!(sb
            .match_full(children[2], &[&parent], 2)
            .paint
            .color
            .is_none());
    }

    #[test]
    fn test_nth_child_formula() {
        // li:nth-child(2n) — matches even-positioned children (1-indexed: 2, 4, 6…)
        let sb = StyleBook::parse("li:nth-child(2n) { color: red; }").unwrap();
        let c0 = make_el("li", vec![], vec![]);
        let c1 = make_el("li", vec![], vec![]);
        let c2 = make_el("li", vec![], vec![]);
        let c3 = make_el("li", vec![], vec![]);
        let parent = make_el("ul", vec![], vec![c0, c1, c2, c3]);
        let children: Vec<&Element> = parent.children.iter().collect();

        // Position 1 (odd) → no match.
        assert!(sb
            .match_full(children[0], &[&parent], 0)
            .paint
            .color
            .is_none());
        // Position 2 (even) → match.
        assert!(sb
            .match_full(children[1], &[&parent], 1)
            .paint
            .color
            .is_some());
        // Position 3 (odd) → no match.
        assert!(sb
            .match_full(children[2], &[&parent], 2)
            .paint
            .color
            .is_none());
        // Position 4 (even) → match.
        assert!(sb
            .match_full(children[3], &[&parent], 3)
            .paint
            .color
            .is_some());
    }

    #[test]
    fn test_nth_child_offset() {
        // li:nth-child(2n+1) — matches odd-positioned children (1, 3, 5…)
        let sb = StyleBook::parse("li:nth-child(2n+1) { color: red; }").unwrap();
        let c0 = make_el("li", vec![], vec![]);
        let c1 = make_el("li", vec![], vec![]);
        let c2 = make_el("li", vec![], vec![]);
        let parent = make_el("ul", vec![], vec![c0, c1, c2]);
        let children: Vec<&Element> = parent.children.iter().collect();

        assert!(sb
            .match_full(children[0], &[&parent], 0)
            .paint
            .color
            .is_some()); // pos 1
        assert!(sb
            .match_full(children[1], &[&parent], 1)
            .paint
            .color
            .is_none()); // pos 2
        assert!(sb
            .match_full(children[2], &[&parent], 2)
            .paint
            .color
            .is_some()); // pos 3
    }

    #[test]
    fn test_nth_of_type_formula() {
        // span:nth-of-type(3n) — every 3rd span (positions 3, 6, 9…)
        let sb = StyleBook::parse("span:nth-of-type(3n) { color: red; }").unwrap();
        let s1 = make_el("span", vec![], vec![]);
        let s2 = make_el("span", vec![], vec![]);
        let s3 = make_el("span", vec![], vec![]);
        let parent = make_el("div", vec![], vec![s1, s2, s3]);
        let children: Vec<&Element> = parent.children.iter().collect();

        assert!(sb
            .match_full(children[0], &[&parent], 0)
            .paint
            .color
            .is_none()); // 1st span
        assert!(sb
            .match_full(children[1], &[&parent], 1)
            .paint
            .color
            .is_none()); // 2nd span
        assert!(sb
            .match_full(children[2], &[&parent], 2)
            .paint
            .color
            .is_some()); // 3rd span
    }

    #[test]
    fn test_empty_matches_leaf() {
        let sb = StyleBook::parse(":empty { color: red; }").unwrap();
        let empty = make_el("div", vec![], vec![]);
        let nonempty = make_el("div", vec![], vec![make_el("span", vec![], vec![])]);
        let parent = make_el("div", vec![], vec![empty, nonempty]);
        let children: Vec<&Element> = parent.children.iter().collect();

        assert!(
            sb.match_full(children[0], &[&parent], 0)
                .paint
                .color
                .is_some(),
            ":empty must match childless element"
        );
        assert!(
            sb.match_full(children[1], &[&parent], 1)
                .paint
                .color
                .is_none(),
            ":empty must not match element with children"
        );
    }

    #[test]
    fn test_not_excludes_match() {
        // li:not(.special) should match li without class=special.
        let sb = StyleBook::parse("li:not(.special) { color: red; }").unwrap();
        let normal = make_el("li", vec![], vec![]);
        let special = make_el(
            "li",
            vec![("class".into(), PropValue::String("special".into()))],
            vec![],
        );
        let parent = make_el("ul", vec![], vec![normal, special]);
        let children: Vec<&Element> = parent.children.iter().collect();

        assert!(
            sb.match_full(children[0], &[&parent], 0)
                .paint
                .color
                .is_some(),
            ":not(.special) must match plain li"
        );
        assert!(
            sb.match_full(children[1], &[&parent], 1)
                .paint
                .color
                .is_none(),
            ":not(.special) must not match .special li"
        );
    }

    #[test]
    fn test_not_with_tag_inner() {
        // div:not(p) should match div but not p.
        let sb = StyleBook::parse("div:not(p) { color: red; }").unwrap();
        let div = make_el("div", vec![], vec![]);
        let p = make_el("p", vec![], vec![]);
        let parent = make_el("div", vec![], vec![div, p]);
        let children: Vec<&Element> = parent.children.iter().collect();

        assert!(sb
            .match_full(children[0], &[&parent], 0)
            .paint
            .color
            .is_some());
        assert!(sb
            .match_full(children[1], &[&parent], 1)
            .paint
            .color
            .is_none());
    }

    #[test]
    fn test_first_child_no_parent_does_not_match() {
        let sb = StyleBook::parse("li:first-child { color: red; }").unwrap();
        let el = make_el("li", vec![], vec![]);
        // Empty parent_chain → no parent → cannot determine first-child.
        assert!(
            sb.match_full(&el, &[], 0).paint.color.is_none(),
            "first-child with no parent must not match"
        );
    }

    #[test]
    fn test_nth_child_with_non_element_siblings() {
        // Mixed element and text children — nth-child counts all children.
        let sb = StyleBook::parse("li:nth-child(2) { color: red; }").unwrap();
        let text_child = Element {
            node_type: NodeType::Text("text".into()),
            props: vec![],
            children: vec![],
        };
        let li = make_el("li", vec![], vec![]);
        let parent = make_el("ul", vec![], vec![text_child, li]);
        let children: Vec<&Element> = parent.children.iter().collect();

        // li is at position 2 among all children → match.
        assert!(sb
            .match_full(children[1], &[&parent], 1)
            .paint
            .color
            .is_some());
    }

    #[test]
    fn test_first_of_type_ignores_non_type_siblings() {
        // first-of-type counts only siblings of the same tag.
        let sb = StyleBook::parse("span:first-of-type { color: red; }").unwrap();
        let div1 = make_el("div", vec![], vec![]);
        let div2 = make_el("div", vec![], vec![]);
        let span = make_el("span", vec![], vec![]);
        let parent = make_el("div", vec![], vec![div1, div2, span]);
        let children: Vec<&Element> = parent.children.iter().collect();

        // span is the first span (divs are ignored) → match.
        assert!(
            sb.match_full(children[2], &[&parent], 2)
                .paint
                .color
                .is_some(),
            "first-of-type must be first among same-type siblings"
        );
    }

    #[test]
    fn test_specificity_of_nth() {
        let rules = uwebr_css::parser::parse_css("li:nth-child(2n) { color: red; }").unwrap();
        let spec = super::selector_specificity(&rules[0].selector);
        // class=1 (nth pseudo), tag=1 → 0*10000 + 1*100 + 1 = 101
        assert_eq!(spec, 101, "nth-child specificity should be (0,1,1)");
    }

    #[test]
    fn test_specificity_of_not() {
        let rules = uwebr_css::parser::parse_css("div:not(.foo) { color: red; }").unwrap();
        let spec = super::selector_specificity(&rules[0].selector);
        // class=1 (not) + class=1 (.foo) + tag=1 (div) = (0,2,1) → 0*10000 + 2*100 + 1 = 201
        assert_eq!(spec, 201, "not(.foo) specificity should be (0,2,1)");
    }

    // ── Complex matching edge-case tests ────────────────────────

    #[test]
    fn render_three_level_descendant_selector() {
        let sb = StyleBook::parse("div > p > span > strong { color: red; }").unwrap();
        let strong = make_el("strong", vec![], vec![]);
        let span = make_el("span", vec![], vec![strong]);
        let p = make_el("p", vec![], vec![span]);
        let div = make_el("div", vec![], vec![p]);
        let chain: Vec<&Element> = vec![
            &div.children[0].children[0].children[0],
            &div.children[0].children[0],
            &div.children[0],
            &div,
        ];

        let m = sb.match_full(chain[0], &chain[1..], 0);
        assert!(
            m.paint.color.is_some(),
            "div > p > span > strong should match the strong"
        );
    }

    #[test]
    fn render_four_level_descendant_selector() {
        let sb = StyleBook::parse("div .outer .inner .leaf { color: blue; }").unwrap();
        let leaf = make_el(
            "span",
            vec![("class".into(), PropValue::String("leaf".into()))],
            vec![],
        );
        let inner = make_el(
            "div",
            vec![("class".into(), PropValue::String("inner".into()))],
            vec![leaf],
        );
        let outer = make_el(
            "div",
            vec![("class".into(), PropValue::String("outer".into()))],
            vec![inner],
        );
        let div = make_el("div", vec![], vec![outer]);

        let m = sb.match_full(
            &div.children[0].children[0].children[0],
            &[&div.children[0].children[0], &div.children[0], &div],
            0,
        );
        assert!(
            m.paint.color.is_some(),
            "4-level descendant should match leaf"
        );
    }

    #[test]
    fn render_nth_last_child_an_plus_b() {
        let sb = StyleBook::parse("li:nth-last-child(2n) { color: red; }").unwrap();
        let c0 = make_el("li", vec![], vec![]);
        let c1 = make_el("li", vec![], vec![]);
        let c2 = make_el("li", vec![], vec![]);
        let c3 = make_el("li", vec![], vec![]);
        let c4 = make_el("li", vec![], vec![]);
        let parent = make_el("ul", vec![], vec![c0, c1, c2, c3, c4]);
        let children: Vec<&Element> = parent.children.iter().collect();

        // nth-last-child(2n): position from end: 1=last,2=second-last,...
        // 2n means even from end: positions 2,4 match
        assert!(sb
            .match_full(children[0], &[&parent], 0)
            .paint
            .color
            .is_none()); // 1st from end (pos 5)
        assert!(sb
            .match_full(children[1], &[&parent], 1)
            .paint
            .color
            .is_some()); // 2nd from end (pos 4)
        assert!(sb
            .match_full(children[2], &[&parent], 2)
            .paint
            .color
            .is_none()); // 3rd from end (pos 3)
        assert!(sb
            .match_full(children[3], &[&parent], 3)
            .paint
            .color
            .is_some()); // 4th from end (pos 2)
        assert!(sb
            .match_full(children[4], &[&parent], 4)
            .paint
            .color
            .is_none()); // 5th from end (pos 1)
    }

    #[test]
    fn render_nth_of_type_complex_an_plus_b() {
        let sb = StyleBook::parse("span:nth-of-type(2n+1) { color: green; }").unwrap();
        let div1 = make_el("div", vec![], vec![]);
        let s1 = make_el("span", vec![], vec![]);
        let div2 = make_el("div", vec![], vec![]);
        let s2 = make_el("span", vec![], vec![]);
        let s3 = make_el("span", vec![], vec![]);
        let parent = make_el("div", vec![], vec![div1, s1, div2, s2, s3]);
        let children: Vec<&Element> = parent.children.iter().collect();

        assert!(sb
            .match_full(children[0], &[&parent], 0)
            .paint
            .color
            .is_none()); // div
        assert!(sb
            .match_full(children[1], &[&parent], 1)
            .paint
            .color
            .is_some()); // 1st span (2*1+1=3? no, position 1 among spans)
        assert!(sb
            .match_full(children[2], &[&parent], 2)
            .paint
            .color
            .is_none()); // div
        assert!(sb
            .match_full(children[3], &[&parent], 3)
            .paint
            .color
            .is_none()); // 2nd span (even among spans)
        assert!(sb
            .match_full(children[4], &[&parent], 4)
            .paint
            .color
            .is_some()); // 3rd span (odd among spans)
    }

    #[test]
    fn render_not_with_complex_inner() {
        let sb = StyleBook::parse("div:not(.a > .b) { color: red; }").unwrap();
        let b = make_el(
            "span",
            vec![("class".into(), PropValue::String("b".into()))],
            vec![],
        );
        let a = make_el(
            "div",
            vec![("class".into(), PropValue::String("a".into()))],
            vec![b],
        );
        let other = make_el("div", vec![], vec![]);

        let m_b = sb.match_full(&a.children[0], &[&a], 0);
        assert!(
            m_b.paint.color.is_none(),
            ":not(.a > .b) should not match b inside .a"
        );

        let m_other = sb.match_full(&other, &[], 0);
        assert!(
            m_other.paint.color.is_some(),
            ":not(.a > .b) should match div without .a parent"
        );
    }

    #[test]
    fn render_focus_within_nested() {
        uwebr_core::state::clear_element_state();
        let sb = StyleBook::parse("div:focus-within { background-color: blue; }").unwrap();
        let inner = make_el("span", vec![], vec![]);
        let parent = make_el("div", vec![], vec![inner]);

        // focus-within requires a non-empty parent chain AND any_focused() true
        uwebr_core::state::set_focused(Some(5));
        let m = sb.match_full(&parent, &[&parent], 1);
        assert!(
            m.paint.background.is_some(),
            ":focus-within should match when any element is focused and parent_chain is non-empty"
        );
        uwebr_core::state::clear_element_state();
    }

    #[test]
    fn render_attribute_data_role_button() {
        let sb = StyleBook::parse(r#"[data-role="button"] { opacity: 0.9; }"#).unwrap();
        let btn = make_el(
            "div",
            vec![("data-role".into(), PropValue::String("button".into()))],
            vec![],
        );
        let link = make_el(
            "div",
            vec![("data-role".into(), PropValue::String("link".into()))],
            vec![],
        );
        assert!(sb.match_full(&btn, &[], 0).paint.opacity.is_some());
        assert!(sb.match_full(&link, &[], 0).paint.opacity.is_none());
    }

    #[test]
    fn render_attribute_href_prefix_http() {
        let sb = StyleBook::parse(r#"[href^="https://"] { opacity: 0.8; }"#).unwrap();
        let secure = make_el(
            "a",
            vec![(
                "href".into(),
                PropValue::String("https://example.com".into()),
            )],
            vec![],
        );
        let insecure = make_el(
            "a",
            vec![(
                "href".into(),
                PropValue::String("http://example.com".into()),
            )],
            vec![],
        );
        assert!(sb.match_full(&secure, &[], 0).paint.opacity.is_some());
        assert!(sb.match_full(&insecure, &[], 0).paint.opacity.is_none());
    }

    #[test]
    fn render_combined_class_and_tag() {
        let sb = StyleBook::parse(".active { color: red; }").unwrap();
        let active_div = make_el(
            "div",
            vec![("class".into(), PropValue::String("active".into()))],
            vec![],
        );
        let inactive_div = make_el("div", vec![], vec![]);

        assert!(
            sb.match_full(&active_div, &[], 0).paint.color.is_some(),
            ".active should match active div"
        );
        assert!(
            sb.match_full(&inactive_div, &[], 0).paint.color.is_none(),
            ".active should not match inactive div"
        );
    }

    #[test]
    fn render_combined_pseudo_first_child() {
        let sb = StyleBook::parse("p:first-child { color: red; }").unwrap();
        let first_p = make_el("p", vec![], vec![]);
        let second_p = make_el("p", vec![], vec![]);
        let div = make_el("div", vec![], vec![first_p, second_p]);
        let children: Vec<&Element> = div.children.iter().collect();

        assert!(
            sb.match_full(children[0], &[&div], 0).paint.color.is_some(),
            "p:first-child should match first p"
        );
        assert!(
            sb.match_full(children[1], &[&div], 1).paint.color.is_none(),
            "p:first-child should not match second p"
        );
    }

    #[test]
    fn render_important_same_specificity_later_wins() {
        let sb = StyleBook::parse(".a { color: red !important; } .b { color: green !important; }")
            .unwrap();
        let el = make_el(
            "div",
            vec![("class".into(), PropValue::String("a b".into()))],
            vec![],
        );
        let m = sb.match_full(&el, &[], 0);
        let c = m.paint.color.unwrap();
        assert_eq!(
            (c.r, c.g, c.b),
            (0, 128, 0),
            "later !important with same specificity should win"
        );
    }

    #[test]
    fn render_cascade_order_later_rules_win() {
        let sb = StyleBook::parse(".box { color: red; } .box { color: blue; }").unwrap();
        let el = make_element(
            "div",
            vec![("class".into(), PropValue::String("box".into()))],
        );
        let m = sb.match_full(&el, &[], 0);
        let c = m.paint.color.unwrap();
        assert_eq!(
            (c.r, c.g, c.b),
            (0, 0, 255),
            "later rule should override earlier rule of same specificity"
        );
    }

    #[test]
    fn render_empty_stylesheet_returns_no_styles() {
        let sb = StyleBook::parse("").unwrap();
        assert!(sb.is_empty());
        let el = make_element("div", vec![]);
        let (style, matched) = sb.match_element(&el);
        assert!(!matched);
        let default: taffy::Style = taffy::Style::default();
        assert_eq!(style, default);
    }

    #[test]
    fn render_not_with_empty_pseudo() {
        let sb = StyleBook::parse("div:not(:empty) { color: red; }").unwrap();
        let empty = make_el("div", vec![], vec![]);
        let nonempty = make_el("div", vec![], vec![make_el("span", vec![], vec![])]);
        let parent = make_el("div", vec![], vec![empty, nonempty]);
        let children: Vec<&Element> = parent.children.iter().collect();

        assert!(
            sb.match_full(children[0], &[&parent], 0)
                .paint
                .color
                .is_none(),
            "div:not(:empty) should not match empty div"
        );
        assert!(
            sb.match_full(children[1], &[&parent], 1)
                .paint
                .color
                .is_some(),
            "div:not(:empty) should match non-empty div"
        );
    }

    #[test]
    fn render_descendant_with_tag_and_class() {
        let sb = StyleBook::parse(".card .inner { color: red; }").unwrap();
        let inner = make_el(
            "span",
            vec![("class".into(), PropValue::String("inner".into()))],
            vec![],
        );
        let card = make_el(
            "div",
            vec![("class".into(), PropValue::String("card".into()))],
            vec![inner],
        );
        let m = sb.match_full(&card.children[0], &[&card], 0);
        assert!(m.paint.color.is_some(), ".card .inner should match");
    }

    #[test]
    fn render_child_selector_chain() {
        let sb = StyleBook::parse("div > p > span { color: red; }").unwrap();
        let span = make_el("span", vec![], vec![]);
        let p = make_el("p", vec![], vec![span]);
        let div = make_el("div", vec![], vec![p]);

        let m = sb.match_full(&div.children[0].children[0], &[&div.children[0], &div], 0);
        assert!(m.paint.color.is_some(), "div > p > span should match");
    }

    #[test]
    fn render_nth_child_odd() {
        let sb = StyleBook::parse("li:nth-child(odd) { color: red; }").unwrap();
        let c0 = make_el("li", vec![], vec![]);
        let c1 = make_el("li", vec![], vec![]);
        let c2 = make_el("li", vec![], vec![]);
        let c3 = make_el("li", vec![], vec![]);
        let parent = make_el("ul", vec![], vec![c0, c1, c2, c3]);
        let children: Vec<&Element> = parent.children.iter().collect();

        assert!(sb
            .match_full(children[0], &[&parent], 0)
            .paint
            .color
            .is_some()); // pos 1 (odd)
        assert!(sb
            .match_full(children[1], &[&parent], 1)
            .paint
            .color
            .is_none()); // pos 2 (even)
        assert!(sb
            .match_full(children[2], &[&parent], 2)
            .paint
            .color
            .is_some()); // pos 3 (odd)
        assert!(sb
            .match_full(children[3], &[&parent], 3)
            .paint
            .color
            .is_none()); // pos 4 (even)
    }

    #[test]
    fn render_nth_child_3n_plus_2() {
        let sb = StyleBook::parse("li:nth-child(3n+2) { color: red; }").unwrap();
        let els: Vec<Element> = (0..7).map(|_| make_el("li", vec![], vec![])).collect();
        let parent = make_el("ul", vec![], els);
        let children: Vec<&Element> = parent.children.iter().collect();

        // Matches positions 2, 5, 8, ...
        assert!(sb
            .match_full(children[0], &[&parent], 0)
            .paint
            .color
            .is_none()); // pos 1
        assert!(sb
            .match_full(children[1], &[&parent], 1)
            .paint
            .color
            .is_some()); // pos 2
        assert!(sb
            .match_full(children[2], &[&parent], 2)
            .paint
            .color
            .is_none()); // pos 3
        assert!(sb
            .match_full(children[3], &[&parent], 3)
            .paint
            .color
            .is_none()); // pos 4
        assert!(sb
            .match_full(children[4], &[&parent], 4)
            .paint
            .color
            .is_some()); // pos 5
        assert!(sb
            .match_full(children[5], &[&parent], 5)
            .paint
            .color
            .is_none()); // pos 6
        assert!(sb
            .match_full(children[6], &[&parent], 6)
            .paint
            .color
            .is_none()); // pos 7
    }

    #[test]
    fn render_last_child_with_class() {
        let sb = StyleBook::parse("li:last-child { color: red; }").unwrap();
        let first = make_el(
            "li",
            vec![("class".into(), PropValue::String("special".into()))],
            vec![],
        );
        let last = make_el(
            "li",
            vec![("class".into(), PropValue::String("special".into()))],
            vec![],
        );
        let parent = make_el("ul", vec![], vec![first, last]);
        let children: Vec<&Element> = parent.children.iter().collect();

        assert!(
            sb.match_full(children[0], &[&parent], 0)
                .paint
                .color
                .is_none(),
            "first child should not match :last-child"
        );
        assert!(
            sb.match_full(children[1], &[&parent], 1)
                .paint
                .color
                .is_some(),
            "last child should match :last-child"
        );
    }

    #[test]
    fn render_child_selector_direct_only_no_skip() {
        let sb = StyleBook::parse("section > .btn { color: red; }").unwrap();
        let btn = make_el(
            "span",
            vec![("class".into(), PropValue::String("btn".into()))],
            vec![],
        );
        let article = make_el("article", vec![], vec![]);
        let section = make_el("section", vec![], vec![article]);

        let m = sb.match_full(&btn, &[&section.children[0], &section], 0);
        assert!(
            m.paint.color.is_none(),
            "section > .btn should not match when section is grandparent not parent"
        );
    }

    #[test]
    fn render_disabled_string_false_not_disabled() {
        let sb = StyleBook::parse("button:disabled { opacity: 0.5; }").unwrap();
        let el = make_element(
            "button",
            vec![("disabled".into(), PropValue::String("false".into()))],
        );
        let m = sb.match_full(&el, &[], 0);
        assert!(
            m.paint.opacity.is_none(),
            "disabled='false' should not be treated as disabled"
        );
    }

    // ── Quality tests (test_q_*) ────────────────────────────────

    #[test]
    fn test_q_stylebook_parse_invalid_css_returns_err() {
        let result = StyleBook::parse("{ { { broken {");
        assert!(result.is_err(), "truly broken CSS must return Err, got Ok");
    }

    #[test]
    fn test_q_stylebook_empty_class_no_match() {
        let sb = StyleBook::parse(".active { color: red; }").unwrap();
        let el = make_element("div", vec![("class".into(), PropValue::String("".into()))]);
        let m = sb.match_full(&el, &[], 0);
        assert!(
            !m.paint.color.is_some(),
            "empty class must not match .active"
        );
    }

    #[test]
    fn test_q_stylebook_reparse_replaces_rules() {
        let mut sb = StyleBook::parse("div { width: 100px; }").unwrap();
        let el = make_element("div", vec![]);
        sb.reparse("div { width: 300px; }", 800.0, 600.0).unwrap();
        let (style, _) = sb.match_element(&el);
        assert_eq!(style.size.width, taffy::Dimension::length(300.0));
    }

    #[test]
    fn test_q_style_mask_or_assign_union() {
        use uwebr_css::codegen::StyleMask;
        let mut a = StyleMask::default();
        a.width = true;
        a.display = true;
        let mut b = StyleMask::default();
        b.height = true;
        b.padding = true;
        a.or_assign(&b);
        assert!(a.width);
        assert!(a.display);
        assert!(a.height);
        assert!(a.padding);
        assert!(!a.margin);
    }

    #[test]
    fn test_q_stylebook_specificity_id_over_class() {
        let sb = StyleBook::parse(".box { width: 100px; } #main { width: 500px; }").unwrap();
        let el = make_element(
            "div",
            vec![
                ("class".into(), PropValue::String("box".into())),
                ("id".into(), PropValue::String("main".into())),
            ],
        );
        let (style, _) = sb.match_element(&el);
        assert_eq!(
            style.size.width,
            taffy::Dimension::length(500.0),
            "#id must beat .class"
        );
    }

    #[test]
    fn test_q_stylebook_important_overrides_id() {
        let sb = StyleBook::parse(".a { width: 100px !important; } #b { width: 500px; }").unwrap();
        let el = make_element(
            "div",
            vec![
                ("class".into(), PropValue::String("a".into())),
                ("id".into(), PropValue::String("b".into())),
            ],
        );
        let (style, _) = sb.match_element(&el);
        assert_eq!(
            style.size.width,
            taffy::Dimension::length(100.0),
            "!important must override higher-specificity #id"
        );
    }

    #[test]
    fn test_q_reparse_different_css_changes_match() {
        let mut sb = StyleBook::parse("div { width: 100px; }").unwrap();
        let el = make_element("div", vec![]);
        let (style1, _) = sb.match_element(&el);
        assert_eq!(style1.size.width, taffy::Dimension::length(100.0));

        sb.reparse("div { height: 250px; }", 800.0, 600.0).unwrap();
        let (style2, _) = sb.match_element(&el);
        assert_eq!(
            style2.size.height,
            taffy::Dimension::length(250.0),
            "reparse must replace all rules"
        );
    }

    #[test]
    fn test_q_parse_then_match_then_layout() {
        let sb = StyleBook::parse("div { width: 150px; height: 75px; }").unwrap();
        let el = make_element("div", vec![]);
        let (style, matched) = sb.match_element(&el);
        assert!(matched);
        assert_eq!(style.size.width, taffy::Dimension::length(150.0));
        assert_eq!(style.size.height, taffy::Dimension::length(75.0));
    }

    #[test]
    fn test_q_paint_inherits_three_levels() {
        use crate::paint::ResolvedPaint;
        // grandparent sets color + font_size, parent overrides font_size, child inherits both
        use uwebr_css::codegen::PaintProps;
        let grandparent = ResolvedPaint {
            color: vello::peniko::Color::from_rgb8(0, 0, 255),
            font_size: 32.0,
            ..Default::default()
        };
        let parent_css = PaintProps {
            font_size: Some(24.0),
            ..Default::default()
        };
        let parent =
            ResolvedPaint::resolve(&grandparent, &parent_css, &make_element("div", vec![]));
        let child_el = make_element("span", vec![]);
        let child = ResolvedPaint::resolve(&parent, &PaintProps::default(), &child_el);
        assert_eq!(child.color, vello::peniko::Color::from_rgb8(0, 0, 255));
        assert_eq!(child.font_size, 24.0);
    }

    #[test]
    fn test_q_layout_inline_color_overrides_css() {
        let sb = StyleBook::parse("div { width: 100px; }").unwrap();
        let mut engine = crate::layout::LayoutEngine::new();
        let el = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![("color".into(), PropValue::String("blue".into()))],
            children: vec![],
        };
        let root = engine.build_tree(&el, &sb).unwrap();
        engine.compute(root, 800.0, 600.0).unwrap();
        let nodes = engine.collect_positioned_nodes(root, &el, &sb);
        assert_eq!(
            nodes[0].paint.color,
            vello::peniko::Color::from_rgb8(0, 0, 255),
            "inline color must override default"
        );
    }

    #[test]
    fn test_q_layout_apply_prop_unknown_noop() {
        let mut engine = crate::layout::LayoutEngine::new();
        let el = Element {
            node_type: NodeType::Element("div".into()),
            props: vec![],
            children: vec![],
        };
        let root = engine.build_tree(&el, &StyleBook::empty()).unwrap();
        engine.compute(root, 800.0, 600.0).unwrap();
        let info = engine.get_layout_info(root).unwrap();
        assert!(info.width >= 0.0, "unknown props must not break layout");
    }

    #[test]
    fn test_q_layout_flex_grow_three_way() {
        let css = ".row { display: flex; flex-direction: row; width: 300px; } .a { flex-grow: 1; height: 20px; } .b { flex-grow: 2; height: 20px; } .c { flex-grow: 1; height: 20px; }";
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
        let mut engine = crate::layout::LayoutEngine::new();
        let node = engine.build_tree(&root, &sb).unwrap();
        engine.compute(node, 400.0, 200.0).unwrap();
        let nodes = engine.collect_positioned_nodes(node, &root, &sb);
        let items: Vec<_> = nodes
            .iter()
            .filter(|n| n.depth == 1)
            .map(|n| n.layout)
            .collect();
        assert_eq!(items.len(), 3);
        assert!(
            items[1].width > items[0].width,
            "flex-grow:2 must be wider than flex-grow:1"
        );
    }

    #[test]
    fn test_q_layout_border_color_from_css() {
        let sb = StyleBook::parse("div { border-color: #ff0000; border-width: 2px; }").unwrap();
        let el = make_element("div", vec![]);
        let mut engine = crate::layout::LayoutEngine::new();
        let root = engine.build_tree(&el, &sb).unwrap();
        engine.compute(root, 800.0, 600.0).unwrap();
        let nodes = engine.collect_positioned_nodes(root, &el, &sb);
        assert_eq!(
            nodes[0].paint.border_color,
            vello::peniko::Color::from_rgba8(255, 0, 0, 255),
            "border-color must resolve from CSS"
        );
        assert_eq!(nodes[0].paint.border_width, 2.0);
    }

    #[test]
    fn test_q_layout_percentage_width_relative_to_parent() {
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
        let mut engine = crate::layout::LayoutEngine::new();
        let node = engine.build_tree(&root, &sb).unwrap();
        engine.compute(node, 800.0, 600.0).unwrap();
        let nodes = engine.collect_positioned_nodes(node, &root, &sb);
        let child = nodes.iter().find(|n| n.depth == 1).unwrap();
        assert!(
            (child.layout.width - 200.0).abs() < 2.0,
            "50% of 400px should be ~200, got {}",
            child.layout.width
        );
    }

    #[test]
    fn test_q_stress_stylebook_500_rules() {
        let mut css = String::new();
        for i in 0..500 {
            css.push_str(&format!(".rule{i} {{ width: {i}px; }} "));
        }
        let sb = StyleBook::parse(&css).unwrap();
        assert_eq!(sb.len(), 500);
        let el = make_element(
            "div",
            vec![("class".into(), PropValue::String("rule250".into()))],
        );
        let (style, matched) = sb.match_element(&el);
        assert!(matched);
        assert_eq!(style.size.width, taffy::Dimension::length(250.0));
    }

    #[test]
    fn test_q_stress_many_css_classes_per_element() {
        let mut css = String::new();
        for i in 0..20 {
            css.push_str(&format!(".c{i} {{ flex-direction: row; }} "));
        }
        let sb = StyleBook::parse(&css).unwrap();
        let classes: Vec<String> = (0..20).map(|i| format!("c{i}")).collect();
        let class_str = classes.join(" ");
        let el = make_element("div", vec![("class".into(), PropValue::String(class_str))]);
        let m = sb.match_full(&el, &[], 0);
        assert!(m.matched);
        assert_eq!(
            m.style.flex_direction,
            taffy::FlexDirection::Row,
            "must inherit flex-direction from matching class"
        );
    }
}
