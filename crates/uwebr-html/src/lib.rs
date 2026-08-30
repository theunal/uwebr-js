pub mod ast;
pub mod codegen;
pub mod directives;
pub mod parser;

pub use ast::{
    HtmlAttribute, HtmlAttributeValue, HtmlComponent, HtmlEach, HtmlElement, HtmlIf, HtmlNode,
};
pub use codegen::generate_rsx;
pub use directives::expand_directives;
pub use parser::{parse_fragment, parse_html};

/// Parse HTML and expand template directives
pub fn parse_html_with_directives(html: &str) -> Result<HtmlNode, anyhow::Error> {
    let mut node = parse_html(html)?;
    expand_directives(&mut node);
    Ok(node)
}

/// Parse HTML fragment and expand template directives
pub fn parse_fragment_with_directives(html: &str) -> Result<Vec<HtmlNode>, anyhow::Error> {
    let mut nodes = parse_fragment(html)?;
    for node in &mut nodes {
        expand_directives(node);
    }
    Ok(nodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_e2e_simple_div() {
        let html = r#"<div class="container">Hello</div>"#;
        let node = parse_html(html).unwrap();
        let rsx = codegen::generate_rsx(&node, 0);
        assert!(rsx.contains("div("));
        assert!(rsx.contains("Hello"));
    }

    #[test]
    fn test_e2e_component_parse_codegen() {
        let html = r#"<Card title="Hello"><p>Body</p></Card>"#;
        let mut node = parse_html(html).unwrap();
        expand_directives(&mut node);
        let rsx = codegen::generate_rsx(&node, 0);
        assert!(rsx.contains("Card("));
        assert!(rsx.contains("title: \"Hello\""));
    }

    #[test]
    fn test_e2e_each_loop() {
        let html = r#"<ul>{#each items as item}<li>{item}</li>{/each}</ul>"#;
        let mut node = parse_html(html).unwrap();
        expand_directives(&mut node);
        let rsx = codegen::generate_rsx(&node, 0);
        assert!(rsx.contains("for item in items.iter()"));
        assert!(rsx.contains("li("));
    }

    #[test]
    fn test_e2e_if_else() {
        let html = r#"<div>{#if show}<p>Yes</p>{:else}<p>No</p>{/if}</div>"#;
        let mut node = parse_html(html).unwrap();
        expand_directives(&mut node);
        match node {
            HtmlNode::Element(el) => {
                let has_if = el
                    .children
                    .iter()
                    .any(|c| matches!(c, HtmlNode::IfBlock(_)));
                assert!(has_if, "Expected if block in children");
            }
            _ => {}
        }
    }

    #[test]
    fn test_e2e_nested_structure() {
        let html = r#"<div><ul><li><a href="/link">Click</a></li></ul></div>"#;
        let node = parse_html(html).unwrap();
        let rsx = codegen::generate_rsx(&node, 0);
        assert!(rsx.contains("div("));
        assert!(rsx.contains("ul("));
        assert!(rsx.contains("li("));
        assert!(rsx.contains("a("));
    }

    #[test]
    fn test_e2e_directives_in_component() {
        let html = r#"<Card><p>{name}</p><p>{age}</p></Card>"#;
        let mut node = parse_html(html).unwrap();
        expand_directives(&mut node);
        let rsx = codegen::generate_rsx(&node, 0);
        assert!(rsx.contains("Card("));
        assert!(rsx.contains("{name}"));
        assert!(rsx.contains("{age}"));
    }

    #[test]
    fn test_e2e_fragment_with_multiple_children() {
        let nodes = parse_fragment_with_directives(r#"<p>A</p><p>B</p>"#).unwrap();
        assert_eq!(nodes.len(), 2);
        let rsx = codegen::generate_rsx(&HtmlNode::Fragment(nodes), 0);
        assert!(rsx.contains("p("));
    }

    #[test]
    fn test_e2e_component_inside_each_inside_if() {
        let html = r#"<div>{#if show}<ul>{#each items as item}<Card title={item}>{item}</Card>{/each}</ul>{/if}</div>"#;
        let mut node = parse_html(html).unwrap();
        expand_directives(&mut node);
        match node {
            HtmlNode::Element(el) => {
                let has_if = el
                    .children
                    .iter()
                    .any(|c| matches!(c, HtmlNode::IfBlock(_)));
                assert!(has_if, "Expected if block in children");
            }
            _ => {}
        }
    }

    #[test]
    fn test_e2e_parse_with_directives_combined() {
        let html = r#"<div class="root"><span>{value}</span></div>"#;
        let node = parse_html_with_directives(html).unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "div");
                assert_eq!(el.children.len(), 1);
                match &el.children[0] {
                    HtmlNode::Element(span) => {
                        assert_eq!(span.tag, "span");
                        match &span.children[0] {
                            HtmlNode::Expression(expr) => assert_eq!(expr, "value"),
                            _ => panic!("Expected expression"),
                        }
                    }
                    _ => panic!("Expected span"),
                }
            }
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_e2e_realistic_template() {
        let html = r#"<div class="app"><header><h1>{title}</h1></header><main>{#if loading}<p>Loading...</p>{:else}<ul>{#each items as item}<li>{item.name}</li>{/each}</ul>{/if}</main></div>"#;
        let mut node = parse_html(html).unwrap();
        expand_directives(&mut node);
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "div");
                assert!(el.attributes.iter().any(|a| a.name == "class"));
            }
            _ => {}
        }
    }
}
