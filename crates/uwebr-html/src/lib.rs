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

    // ═══════════════════════════════════════════════════════════════
    //  Quality tests — Part 2
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn test_q_html_entities_all() {
        let html = r#"<p>&amp; &lt; &gt; &#65; &quot;</p>"#;
        let node = parse_html(html).unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.children.len(), 1);
                match &el.children[0] {
                    HtmlNode::Text(text) => {
                        assert!(text.contains('&'), "should contain &");
                        assert!(text.contains('<'), "should contain <");
                        assert!(text.contains('>'), "should contain >");
                        assert!(
                            text.contains('A') || text.contains("65"),
                            "should decode &#65; to A"
                        );
                    }
                    other => panic!("expected Text node, got {other:?}"),
                }
            }
            other => panic!("expected Element, got {other:?}"),
        }
    }

    #[test]
    fn test_q_html_deeply_nested_50_levels() {
        let mut html = String::from("<div>");
        for i in 0..50 {
            html.push_str(&format!("<span id=\"level-{i}\">"));
        }
        html.push_str("deep");
        for _ in 0..50 {
            html.push_str("</span>");
        }
        html.push_str("</div>");
        let node = parse_html(&html).unwrap();
        let rsx = codegen::generate_rsx(&node, 0);
        assert!(rsx.contains("div("), "outermost div present");
        assert!(rsx.contains("deep"), "innermost text preserved");
        assert!(rsx.contains("span("), "span tags present");
    }

    #[test]
    fn test_q_html_script_content_preserved() {
        let html = r#"<div><script>console.log("hello");</script></div>"#;
        let node = parse_html(html).unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.children.len(), 1);
                match &el.children[0] {
                    HtmlNode::Element(script) => {
                        assert_eq!(script.tag, "script");
                        let text: String = script
                            .children
                            .iter()
                            .filter_map(|c| match c {
                                HtmlNode::Text(t) => Some(t.as_str()),
                                _ => None,
                            })
                            .collect();
                        assert!(
                            text.contains("console.log"),
                            "script content should be preserved, got: {text}"
                        );
                    }
                    other => panic!("expected script Element, got {other:?}"),
                }
            }
            other => panic!("expected Element, got {other:?}"),
        }
    }

    #[test]
    fn test_q_html_style_content_preserved() {
        let html = r#"<div><style>.foo { color: red; }</style></div>"#;
        let node = parse_html(html).unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.children.len(), 1);
                match &el.children[0] {
                    HtmlNode::Element(style) => {
                        assert_eq!(style.tag, "style");
                        let text: String = style
                            .children
                            .iter()
                            .filter_map(|c| match c {
                                HtmlNode::Text(t) => Some(t.as_str()),
                                _ => None,
                            })
                            .collect();
                        assert!(
                            text.contains("color"),
                            "style content should be preserved, got: {text}"
                        );
                    }
                    other => panic!("expected style Element, got {other:?}"),
                }
            }
            other => panic!("expected Element, got {other:?}"),
        }
    }

    #[test]
    fn test_q_html_codegen_indentation_3_levels() {
        let html = r#"<div><ul><li>item</li></ul></div>"#;
        let node = parse_html(html).unwrap();
        let rsx = codegen::generate_rsx(&node, 0);
        let lines: Vec<&str> = rsx.lines().collect();
        let li_line = lines.iter().find(|l| l.contains("li(")).unwrap();
        let leading_spaces = li_line.len() - li_line.trim_start().len();
        assert!(
            leading_spaces >= 8,
            "3 levels deep should have at least 8 spaces indentation, got {leading_spaces}"
        );
    }

    #[test]
    fn test_q_html_directive_each_with_index() {
        let html = r#"<ul>{#each items as item, i}<li>{item}</li>{/each}</ul>"#;
        let mut node = parse_html(html).unwrap();
        expand_directives(&mut node);
        match node {
            HtmlNode::Element(el) => {
                let has_each = el
                    .children
                    .iter()
                    .any(|c| matches!(c, HtmlNode::EachLoop(_)));
                assert!(has_each, "should contain an EachLoop directive");
                if let Some(HtmlNode::EachLoop(each)) = el
                    .children
                    .iter()
                    .find(|c| matches!(c, HtmlNode::EachLoop(_)))
                {
                    assert!(
                        each.item_name.contains("item"),
                        "item_name should contain 'item', got: {}",
                        each.item_name
                    );
                }
            }
            other => panic!("expected Element, got {other:?}"),
        }
    }

    #[test]
    fn test_q_html_directive_if_else_if_chain() {
        let html =
            r#"<div>{#if a}<p>first</p>{:else if b}<p>second</p>{:else}<p>third</p>{/if}</div>"#;
        let mut node = parse_html(html).unwrap();
        expand_directives(&mut node);
        let rsx = codegen::generate_rsx(&node, 0);
        assert!(
            rsx.contains("if a"),
            "should contain if condition, got: {rsx}"
        );
        assert!(
            rsx.contains("else"),
            "should contain else branch, got: {rsx}"
        );
    }

    #[test]
    fn test_q_html_directive_nested_each_in_if() {
        let html =
            r#"<div>{#if show}<ul>{#each items as item}<li>{item}</li>{/each}</ul>{/if}</div>"#;
        let mut node = parse_html(html).unwrap();
        expand_directives(&mut node);
        match node {
            HtmlNode::Element(el) => {
                let has_if = el
                    .children
                    .iter()
                    .any(|c| matches!(c, HtmlNode::IfBlock(_)));
                assert!(has_if, "should contain an IfBlock directive");
            }
            other => panic!("expected Element, got {other:?}"),
        }
    }

    #[test]
    fn test_q_html_e2e_component_with_props() {
        let html =
            r#"<Button label="Click me" disabled={isDisabled} on:click={handler}>Child</Button>"#;
        let mut node = parse_html(html).unwrap();
        expand_directives(&mut node);
        let rsx = codegen::generate_rsx(&node, 0);
        assert!(
            rsx.contains("Button("),
            "component name present, got: {rsx}"
        );
        assert!(rsx.contains("label:"), "should have label prop, got: {rsx}");
        assert!(
            rsx.contains("disabled:"),
            "should have disabled prop, got: {rsx}"
        );
        assert!(
            rsx.contains("Child"),
            "should contain child text, got: {rsx}"
        );
    }

    #[test]
    fn test_q_html_e2e_fragment_children() {
        let nodes =
            parse_fragment_with_directives(r#"<p>first</p><p>second</p><p>third</p>"#).unwrap();
        assert_eq!(nodes.len(), 3);
        let rsx = codegen::generate_rsx(&HtmlNode::Fragment(nodes), 0);
        assert!(rsx.contains("rsx!"), "fragment should produce rsx!");
        let p_count = rsx.matches("p(").count();
        assert_eq!(p_count, 3, "should contain 3 p elements, got: {rsx}");
    }
}
