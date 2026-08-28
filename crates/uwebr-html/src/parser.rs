use crate::ast::*;
use anyhow::Result;
use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};

/// Parse HTML string into HtmlNode tree using html5ever
/// Wraps content in proper HTML structure, then extracts the body
pub fn parse_html(html: &str) -> Result<HtmlNode> {
    let dom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .one(html.as_bytes());

    // Navigate to <html><body> to get actual content
    let body = find_body(&dom.document);
    match body {
        Some(body_handle) => {
            let children = convert_children(&body_handle);
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
    let dom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .one(html.as_bytes());

    let body = find_body(&dom.document);
    match body {
        Some(body_handle) => Ok(convert_children(&body_handle)),
        None => Ok(vec![]),
    }
}

/// Find the <body> element in the DOM tree
fn find_body(handle: &Handle) -> Option<Handle> {
    match &handle.data {
        NodeData::Element { name, .. } if name.local.as_ref() == "body" => {
            return Some(handle.clone());
        }
        _ => {}
    }
    for child in handle.children.borrow().iter() {
        if let Some(found) = find_body(child) {
            return Some(found);
        }
    }
    None
}

/// Convert all children of a node
fn convert_children(handle: &Handle) -> Vec<HtmlNode> {
    handle
        .children
        .borrow()
        .iter()
        .map(|c| convert_node(c))
        .filter(|n| !matches!(n, HtmlNode::Text(t) if t.is_empty()))
        .collect()
}

fn convert_node(handle: &Handle) -> HtmlNode {
    let children = convert_children(handle);

    match &handle.data {
        NodeData::Document => HtmlNode::Fragment(children),
        NodeData::Comment { .. } => HtmlNode::Comment("".to_string()),
        NodeData::Text { contents } => {
            let text = contents.borrow().to_string();
            HtmlNode::Text(text.trim().to_string())
        }
        NodeData::Element { name, attrs, template_contents, .. } => {
            let tag = name.local.to_string();

            let attributes: Vec<HtmlAttribute> = attrs
                .borrow()
                .iter()
                .map(|attr| {
                    let name = attr.name.local.to_string();
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
                .map(|t| convert_children(t))
                .unwrap_or_default();

            let all_children = if !template_children.is_empty() {
                template_children
            } else {
                children
            };

            let is_component = tag.chars().next().map_or(false, |c| c.is_uppercase());

            if is_component {
                HtmlNode::Component(HtmlComponent {
                    name: tag,
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
}
