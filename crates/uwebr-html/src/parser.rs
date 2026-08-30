use crate::ast::*;
use anyhow::Result;
use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use std::collections::HashSet;

use std::collections::HashMap;

/// Scan HTML string for PascalCase tag names before html5ever lowercases them
/// Returns mapping: lowercase_tag → original_PascalCase_name
pub fn detect_components(html: &str) -> HashMap<String, String> {
    let mut components = HashMap::new();
    let mut chars = html.char_indices().peekable();

    while let Some((i, c)) = chars.next() {
        if c == '<' {
            // Skip </ closing tags
            if chars.peek().is_some_and(|(_, c)| *c == '/') {
                continue;
            }
            // Read tag name
            let start = i + 1;
            let mut end = start;
            while let Some(&(_, c)) = chars.peek() {
                if c.is_alphanumeric() || c == '_' {
                    end += 1;
                    chars.next();
                } else {
                    break;
                }
            }
            if end > start {
                let tag = &html[start..end];
                // PascalCase: starts with uppercase, has at least 2 chars
                if tag.len() >= 2
                    && tag.chars().next().is_some_and(|c| c.is_uppercase())
                    && tag.chars().any(|c| c.is_lowercase())
                {
                    // Store lowercase → original mapping
                    components.insert(tag.to_lowercase(), tag.to_string());
                }
            }
        }
    }
    components
}

/// Parse HTML string into HtmlNode tree using html5ever
/// Wraps content in proper HTML structure, then extracts the body
pub fn parse_html(html: &str) -> Result<HtmlNode> {
    let components = detect_components(html);
    let component_names: HashSet<String> = components.keys().cloned().collect();
    let dom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .one(html.as_bytes());

    let body = find_body_or_head(&dom.document);
    match body {
        Some(body_handle) => {
            let children = convert_children(&body_handle, &component_names, &components);
            if children.len() == 1 {
                Ok(children.into_iter().next().unwrap())
            } else {
                Ok(HtmlNode::Fragment(children))
            }
        }
        None => Ok(HtmlNode::Fragment(vec![])),
    }
}

/// Parse HTML with known component names (PascalCase tags that html5ever lowercases)
pub fn parse_html_with_components(
    html: &str,
    components: &HashMap<String, String>,
) -> Result<HtmlNode> {
    let component_names: HashSet<String> = components.keys().cloned().collect();
    let dom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .one(html.as_bytes());

    let body = find_body_or_head(&dom.document);
    match body {
        Some(body_handle) => {
            let children = convert_children(&body_handle, &component_names, components);
            if children.len() == 1 {
                Ok(children.into_iter().next().unwrap())
            } else {
                Ok(HtmlNode::Fragment(children))
            }
        }
        None => Ok(HtmlNode::Fragment(vec![])),
    }
}

/// Parse a fragment (not full document)
pub fn parse_fragment(html: &str) -> Result<Vec<HtmlNode>> {
    let components = detect_components(html);
    let component_names: HashSet<String> = components.keys().cloned().collect();
    let dom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .one(html.as_bytes());

    let body = find_body_or_head(&dom.document);
    match body {
        Some(body_handle) => Ok(convert_children(
            &body_handle,
            &component_names,
            &components,
        )),
        None => Ok(vec![]),
    }
}

/// Find the best content container: <body>, <head>, or document root.
/// Returns the first one that has children.
fn find_body_or_head(handle: &Handle) -> Option<Handle> {
    // Try <body> first (if it has children)
    if let Some(body) = find_element(handle, "body") {
        if !body.children.borrow().is_empty() {
            return Some(body);
        }
    }
    // Fall back to <head> (if it has children)
    if let Some(head) = find_element(handle, "head") {
        if !head.children.borrow().is_empty() {
            return Some(head);
        }
    }
    // Fall back to document root
    Some(handle.clone())
}

/// Find a specific element by tag name
fn find_element(handle: &Handle, target_tag: &str) -> Option<Handle> {
    match &handle.data {
        NodeData::Element { name, .. } if name.local.as_ref() == target_tag => {
            return Some(handle.clone());
        }
        _ => {}
    }
    for child in handle.children.borrow().iter() {
        if let Some(found) = find_element(child, target_tag) {
            return Some(found);
        }
    }
    None
}

/// Convert all children of a node
fn convert_children(
    handle: &Handle,
    component_names: &HashSet<String>,
    components: &HashMap<String, String>,
) -> Vec<HtmlNode> {
    handle
        .children
        .borrow()
        .iter()
        .map(|c| convert_node(c, component_names, components))
        .filter(|n| !matches!(n, HtmlNode::Text(t) if t.is_empty()))
        .collect()
}

fn convert_node(
    handle: &Handle,
    component_names: &HashSet<String>,
    components: &HashMap<String, String>,
) -> HtmlNode {
    let children = convert_children(handle, component_names, components);

    match &handle.data {
        NodeData::Document => HtmlNode::Fragment(children),
        NodeData::Comment { .. } => HtmlNode::Comment("".to_string()),
        NodeData::Text { contents } => {
            let text = contents.borrow().to_string();
            HtmlNode::Text(text.trim().to_string())
        }
        NodeData::Element {
            name,
            attrs,
            template_contents,
            ..
        } => {
            let tag = name.local.to_string();

            let attributes: Vec<HtmlAttribute> = attrs
                .borrow()
                .iter()
                .map(|attr| {
                    // Reconstruct full attribute name including namespace prefix
                    let prefix = attr
                        .name
                        .prefix
                        .clone()
                        .map_or(String::new(), |p| format!("{}:", p));
                    let local = attr.name.local.to_string();
                    let name = format!("{}{}", prefix, local);
                    let value = attr.value.to_string();
                    HtmlAttribute {
                        name,
                        value: HtmlAttributeValue::Literal(value),
                    }
                })
                .collect();

            let template_children = template_contents
                .borrow()
                .as_ref()
                .map(|t| convert_children(t, component_names, components))
                .unwrap_or_default();

            let all_children = if !template_children.is_empty() {
                template_children
            } else {
                children
            };

            // html5ever lowercases all tags, so check original HTML for PascalCase
            let (is_component, original_name) = if let Some(original) = components.get(&tag) {
                (true, original.clone())
            } else {
                (
                    tag.chars().next().is_some_and(|c| c.is_uppercase()),
                    tag.clone(),
                )
            };

            if is_component {
                HtmlNode::Component(HtmlComponent {
                    name: original_name,
                    attributes,
                    children: all_children,
                })
            } else {
                HtmlNode::Element(HtmlElement {
                    tag,
                    attributes,
                    children: all_children,
                    self_closing: false,
                })
            }
        }
        NodeData::Doctype { .. } | NodeData::ProcessingInstruction { .. } => {
            HtmlNode::Comment("".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::directives::expand_directives;

    #[test]
    fn test_parse_simple_element() {
        let html = r#"<div class="container">Hello</div>"#;
        let node = parse_html(html).unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "div");
                assert!(el.attributes.iter().any(|a| a.name == "class"));
                assert_eq!(el.children.len(), 1);
                assert_eq!(el.children[0], HtmlNode::Text("Hello".to_string()));
            }
            _ => panic!("Expected element, got {:?}", node),
        }
    }

    #[test]
    fn test_parse_nested_elements() {
        let html = r#"<div><span>Hello</span><span>World</span></div>"#;
        let node = parse_html(html).unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "div");
                assert_eq!(el.children.len(), 2);
                for child in &el.children {
                    match child {
                        HtmlNode::Element(span) => assert_eq!(span.tag, "span"),
                        _ => panic!("Expected span element"),
                    }
                }
            }
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_parse_self_closing_tag() {
        let html = r#"<img src="test.png" />"#;
        let node = parse_html(html).unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "img");
                assert!(el.attributes.iter().any(|a| a.name == "src"));
            }
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_parse_boolean_attribute() {
        let html = r#"<input disabled />"#;
        let node = parse_html(html).unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "input");
                assert!(el.attributes.iter().any(|a| a.name == "disabled"));
            }
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_parse_multiple_attributes() {
        let html = r#"<div class="card" id="main" data-value="42">Content</div>"#;
        let node = parse_html(html).unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "div");
                assert_eq!(el.attributes.len(), 3);
                assert!(el.attributes.iter().any(|a| a.name == "class"));
                assert!(el.attributes.iter().any(|a| a.name == "id"));
                assert!(el.attributes.iter().any(|a| a.name == "data-value"));
            }
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_parse_fragment() {
        let html = r#"<p>One</p><p>Two</p>"#;
        let nodes = parse_fragment(html).unwrap();
        assert_eq!(nodes.len(), 2);
    }

    #[test]
    fn test_parse_deeply_nested() {
        let html = r#"<div><ul><li><a href="/link">Click</a></li></ul></div>"#;
        let node = parse_html(html).unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "div");
                match &el.children[0] {
                    HtmlNode::Element(ul) => {
                        assert_eq!(ul.tag, "ul");
                        match &ul.children[0] {
                            HtmlNode::Element(li) => {
                                assert_eq!(li.tag, "li");
                                match &li.children[0] {
                                    HtmlNode::Element(a) => {
                                        assert_eq!(a.tag, "a");
                                        assert!(a.attributes.iter().any(|a| a.name == "href"));
                                    }
                                    _ => panic!("Expected a"),
                                }
                            }
                            _ => panic!("Expected li"),
                        }
                    }
                    _ => panic!("Expected ul"),
                }
            }
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_parse_empty_element() {
        let html = r#"<div></div>"#;
        let node = parse_html(html).unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "div");
                assert!(el.children.is_empty());
            }
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_parse_event_attribute() {
        let html = r#"<button on:click="handler">Click</button>"#;
        let node = parse_html(html).unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "button");
                // html5ever may split on: into namespace prefix
                let has_event = el
                    .attributes
                    .iter()
                    .any(|a| a.name.contains("click") || a.name.contains("on:"));
                assert!(
                    has_event,
                    "Expected on:click attribute, got: {:?}",
                    el.attributes
                );
            }
            _ => panic!("Expected element"),
        }
    }

    // --- Real-world HTML patterns ---

    #[test]
    fn test_parse_style_attribute() {
        let html = r#"<div style="color: red; font-size: 16px">Styled</div>"#;
        let node = parse_html(html).unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "div");
                let has_style = el.attributes.iter().any(|a| a.name == "style");
                assert!(has_style, "Expected style attribute");
            }
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_parse_svg_self_closing() {
        let html = r#"<svg><circle cx="50" cy="50" r="40" /></svg>"#;
        let node = parse_html(html).unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "svg");
                assert_eq!(el.children.len(), 1);
            }
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_parse_script_tag() {
        let html = r#"<script>console.log("hi")</script>"#;
        let node = parse_html(html).unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "script");
                // html5ever treats script content as raw text (not parsed as HTML)
                // This is correct behavior - script content should not be parsed
            }
            _ => panic!("Expected element, got {:?}", node),
        }
    }

    #[test]
    fn test_parse_table() {
        let html = r#"<table><tr><td>Cell</td></tr></table>"#;
        let node = parse_html(html).unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "table");
            }
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_parse_select_option() {
        let html = r#"<select><option value="a">A</option><option value="b">B</option></select>"#;
        let node = parse_html(html).unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "select");
                assert_eq!(el.children.len(), 2);
            }
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_parse_form_inputs() {
        let html = r#"<form><input type="text" name="user" /><input type="password" name="pass" /></form>"#;
        let node = parse_html(html).unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "form");
                assert_eq!(el.children.len(), 2);
            }
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_parse_template_directives_in_html() {
        let html = r#"<div>{name}</div>"#;
        let mut node = parse_html(html).unwrap();
        expand_directives(&mut node);
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.children.len(), 1);
                match &el.children[0] {
                    HtmlNode::Expression(expr) => assert_eq!(expr, "name"),
                    _ => panic!("Expected expression"),
                }
            }
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_parse_each_in_html() {
        let html = r#"<ul>{#each items as item}<li>{item}</li>{/each}</ul>"#;
        let mut node = parse_html(html).unwrap();
        expand_directives(&mut node);
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "ul");
                assert_eq!(el.children.len(), 1);
                match &el.children[0] {
                    HtmlNode::EachLoop(each) => {
                        assert_eq!(each.iterable, "items");
                        assert_eq!(each.item_name, "item");
                    }
                    _ => panic!("Expected each loop"),
                }
            }
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_parse_if_in_html() {
        let html = r#"<div>{#if show}<p>Visible</p>{/if}</div>"#;
        let mut node = parse_html(html).unwrap();
        expand_directives(&mut node);
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "div");
                assert_eq!(el.children.len(), 1);
                match &el.children[0] {
                    HtmlNode::IfBlock(if_block) => {
                        assert_eq!(if_block.condition, "show");
                    }
                    _ => panic!("Expected if block"),
                }
            }
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_parse_aria_attributes() {
        let html = r#"<div aria-label="Close" role="button" tabindex="0">X</div>"#;
        let node = parse_html(html).unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.attributes.len(), 3);
                assert!(el.attributes.iter().any(|a| a.name == "aria-label"));
                assert!(el.attributes.iter().any(|a| a.name == "role"));
                assert!(el.attributes.iter().any(|a| a.name == "tabindex"));
            }
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_parse_nested_component_with_children() {
        let html = r#"<Card title="Hello"><p>Body content</p></Card>"#;
        let mut node = parse_html(html).unwrap();
        expand_directives(&mut node);
        match node {
            HtmlNode::Component(comp) => {
                assert_eq!(comp.name, "Card");
                assert!(!comp.children.is_empty());
            }
            _ => panic!("Expected component"),
        }
    }

    // --- Edge case tests ---

    #[test]
    fn test_html_empty_string() {
        let node = parse_html("").unwrap();
        match node {
            HtmlNode::Fragment(_) => {}
            HtmlNode::Element(_) => {}
            _ => panic!("Expected fragment or element for empty input"),
        }
    }

    #[test]
    fn test_html_whitespace_only() {
        let node = parse_html("   \n\t  ").unwrap();
        match node {
            HtmlNode::Fragment(_) => {}
            HtmlNode::Text(_) => {}
            HtmlNode::Element(_) => {}
            _ => panic!("Expected fragment, text, or element for whitespace input"),
        }
    }

    #[test]
    fn test_html_text_only_no_tags() {
        let node = parse_html("Hello world").unwrap();
        match node {
            HtmlNode::Text(t) => assert_eq!(t, "Hello world"),
            HtmlNode::Fragment(children) => {
                assert_eq!(children.len(), 1);
                match &children[0] {
                    HtmlNode::Text(t) => assert_eq!(t, "Hello world"),
                    _ => panic!("Expected text"),
                }
            }
            _ => panic!("Expected text or fragment"),
        }
    }

    #[test]
    fn test_html_unclosed_tag() {
        let node = parse_html("<div>Hello").unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "div");
            }
            HtmlNode::Fragment(children) => {
                assert!(!children.is_empty());
            }
            _ => panic!("Expected element or fragment"),
        }
    }

    #[test]
    fn test_html_mismatched_tags() {
        let node = parse_html("<div><span>Hello</div></span>").unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "div");
            }
            HtmlNode::Fragment(_) => {}
            _ => panic!("Expected element or fragment"),
        }
    }

    #[test]
    fn test_html_entity_amp() {
        let node = parse_html("<p>&amp;</p>").unwrap();
        match node {
            HtmlNode::Element(el) => match &el.children[0] {
                HtmlNode::Text(t) => assert!(t.contains('&')),
                _ => panic!("Expected text child"),
            },
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_html_entity_lt_gt() {
        let node = parse_html("<p>&lt;div&gt;</p>").unwrap();
        match node {
            HtmlNode::Element(el) => match &el.children[0] {
                HtmlNode::Text(t) => {
                    assert!(t.contains('<'));
                    assert!(t.contains('>'));
                }
                _ => panic!("Expected text child"),
            },
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_html_entity_numeric() {
        let node = parse_html("<p>&#65;</p>").unwrap();
        match node {
            HtmlNode::Element(el) => match &el.children[0] {
                HtmlNode::Text(t) => assert!(t.contains('A')),
                _ => panic!("Expected text child"),
            },
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_html_entity_quote() {
        let node = parse_html("<p>&quot;hello&quot;</p>").unwrap();
        match node {
            HtmlNode::Element(el) => match &el.children[0] {
                HtmlNode::Text(t) => assert!(t.contains('"')),
                _ => panic!("Expected text child"),
            },
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_html_deeply_nested() {
        let mut html = String::new();
        for _ in 0..25 {
            html.push_str("<div>");
        }
        html.push_str("deep");
        for _ in 0..25 {
            html.push_str("</div>");
        }
        let node = parse_html(&html).unwrap();
        let mut current = &node;
        for _ in 0..25 {
            match current {
                HtmlNode::Element(el) => {
                    assert_eq!(el.tag, "div");
                    if !el.children.is_empty() {
                        current = &el.children[0];
                    }
                }
                _ => panic!("Expected nested div"),
            }
        }
    }

    #[test]
    fn test_html_script_content_preserved() {
        let html = r#"<script>var x = 1 + 2; if (x < 3) { console.log(x); }</script>"#;
        let node = parse_html(html).unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "script");
                assert_eq!(el.children.len(), 1);
                match &el.children[0] {
                    HtmlNode::Text(t) => {
                        assert!(t.contains("var x"));
                        assert!(t.contains("console.log"));
                    }
                    _ => panic!("Expected text child with JS"),
                }
            }
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_html_style_content_preserved() {
        let html = r#"<style>.foo { color: red; }</style>"#;
        let node = parse_html(html).unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "style");
                assert_eq!(el.children.len(), 1);
                match &el.children[0] {
                    HtmlNode::Text(t) => {
                        assert!(t.contains(".foo"));
                        assert!(t.contains("color: red"));
                    }
                    _ => panic!("Expected text child with CSS"),
                }
            }
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_html_self_closing_br() {
        let node = parse_html("<br>").unwrap();
        match node {
            HtmlNode::Element(el) => assert_eq!(el.tag, "br"),
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_html_self_closing_hr() {
        let node = parse_html("<hr>").unwrap();
        match node {
            HtmlNode::Element(el) => assert_eq!(el.tag, "hr"),
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_html_self_closing_meta() {
        let node = parse_html(r#"<meta charset="utf-8">"#).unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "meta");
                assert!(el.attributes.iter().any(|a| a.name == "charset"));
            }
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_html_self_closing_link() {
        let node = parse_html(r#"<link rel="stylesheet" href="style.css">"#).unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "link");
                assert_eq!(el.attributes.len(), 2);
            }
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_html_component_detection() {
        let components = detect_components(r#"<Modal><p>content</p></Modal>"#);
        assert!(components.contains_key("modal"));
        assert_eq!(components.get("modal").unwrap(), "Modal");
    }

    #[test]
    fn test_html_component_not_single_char_uppercase() {
        let components = detect_components(r#"<A>text</A>"#);
        assert!(
            components.is_empty(),
            "Single uppercase char should not be a component"
        );
    }

    #[test]
    fn test_html_leading_trailing_whitespace() {
        let node = parse_html("  <div>  Hello  </div>  ").unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "div");
            }
            HtmlNode::Fragment(children) => {
                assert!(!children.is_empty());
            }
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_html_whitespace_between_elements() {
        let node = parse_html("<div>  <span>A</span>  <span>B</span>  </div>").unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "div");
                let spans: Vec<_> = el
                    .children
                    .iter()
                    .filter(|c| matches!(c, HtmlNode::Element(e) if e.tag == "span"))
                    .collect();
                assert_eq!(spans.len(), 2);
            }
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_html_empty_attributes() {
        let node = parse_html(r#"<input type="text" disabled>"#).unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "input");
                let has_disabled = el.attributes.iter().any(|a| a.name == "disabled");
                assert!(has_disabled);
            }
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_html_single_quoted_attribute() {
        let node = parse_html("<div class='foo'>x</div>").unwrap();
        match node {
            HtmlNode::Element(el) => {
                let class_attr = el.attributes.iter().find(|a| a.name == "class");
                assert!(class_attr.is_some());
            }
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_html_comment_handling() {
        let node = parse_html("<!-- hello --><div>x</div>").unwrap();
        match node {
            HtmlNode::Fragment(children) => {
                let has_element = children.iter().any(|c| matches!(c, HtmlNode::Element(_)));
                assert!(has_element);
            }
            HtmlNode::Element(_) => {}
            _ => panic!("Expected fragment or element"),
        }
    }

    #[test]
    fn test_html_mixed_text_and_elements() {
        let node = parse_html("<div>Text1 <span>inner</span> Text2</div>").unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "div");
                assert!(el.children.len() >= 3);
            }
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_html_unicode_text() {
        let node = parse_html("<p>Привет мир 你好世界</p>").unwrap();
        match node {
            HtmlNode::Element(el) => match &el.children[0] {
                HtmlNode::Text(t) => {
                    assert!(t.contains("Привет"));
                    assert!(t.contains("你好"));
                }
                _ => panic!("Expected text"),
            },
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_html_unicode_attribute() {
        let node = parse_html(r#"<div title="日本語">x</div>"#).unwrap();
        match node {
            HtmlNode::Element(el) => {
                let title = el.attributes.iter().find(|a| a.name == "title");
                assert!(title.is_some());
            }
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_html_nested_components() {
        let html = r#"<Modal><Card title="t"><p>body</p></Card></Modal>"#;
        let components = detect_components(html);
        assert!(components.contains_key("modal"));
        assert!(components.contains_key("card"));
    }

    #[test]
    fn test_parse_fragment_multiple_roots() {
        let nodes = parse_fragment("<div>A</div><span>B</span><p>C</p>").unwrap();
        assert_eq!(nodes.len(), 3);
    }

    #[test]
    fn test_parse_fragment_empty() {
        let nodes = parse_fragment("").unwrap();
        assert!(nodes.len() <= 1);
    }

    #[test]
    fn test_parse_html_with_components_custom() {
        use std::collections::HashMap;
        let mut components = HashMap::new();
        components.insert("mywidget".to_string(), "MyWidget".to_string());
        let html = r#"<MyWidget><p>inner</p></MyWidget>"#;
        let node = parse_html_with_components(html, &components).unwrap();
        match node {
            HtmlNode::Component(comp) => {
                assert_eq!(comp.name, "MyWidget");
            }
            _ => panic!("Expected component"),
        }
    }

    #[test]
    fn test_html_doctype_ignored() {
        let node = parse_html("<!DOCTYPE html><div>x</div>").unwrap();
        match node {
            HtmlNode::Element(el) => assert_eq!(el.tag, "div"),
            HtmlNode::Fragment(children) => {
                assert!(children
                    .iter()
                    .any(|c| matches!(c, HtmlNode::Element(e) if e.tag == "div")));
            }
            _ => panic!("Expected element"),
        }
    }
}
