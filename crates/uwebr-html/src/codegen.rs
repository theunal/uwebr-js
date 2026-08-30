use crate::ast::*;

/// Generate rsx! macro format from HtmlNode tree
pub fn generate_rsx(node: &HtmlNode, indent: usize) -> String {
    let prefix = "    ".repeat(indent);
    match node {
        HtmlNode::Element(el) => generate_element(el, indent),
        HtmlNode::Text(text) => {
            if text.is_empty() {
                String::new()
            } else {
                format!("{}\"{}\"", prefix, text)
            }
        }
        HtmlNode::Expression(expr) => {
            format!("{}\"{{{}}}\"", prefix, expr)
        }
        HtmlNode::RawHtml(expr) => {
            format!("{}rsx!(Raw({}))", prefix, expr)
        }
        HtmlNode::Component(comp) => generate_component(comp, indent),
        HtmlNode::EachLoop(each) => generate_each(each, indent),
        HtmlNode::IfBlock(if_block) => generate_if(if_block, indent),
        HtmlNode::Fragment(nodes) => {
            let inner: Vec<String> = nodes
                .iter()
                .map(|n| generate_rsx(n, indent + 1))
                .filter(|s| !s.is_empty())
                .collect();
            if inner.is_empty() {
                format!("{}rsx! {{}}", prefix)
            } else {
                format!("{}rsx! {{\n{}\n{}}}", prefix, inner.join("\n"), prefix)
            }
        }
        HtmlNode::Comment(_) => String::new(),
    }
}

fn generate_element(el: &HtmlElement, indent: usize) -> String {
    let prefix = "    ".repeat(indent);
    let mut output = format!("{}{}(", prefix, el.tag);

    // Separate event handlers from regular attributes
    let (events, attrs): (Vec<_>, Vec<_>) = el
        .attributes
        .iter()
        .partition(|a| a.name.starts_with("on:"));

    let all_attrs: Vec<String> = attrs
        .iter()
        .chain(events.iter())
        .map(|a| generate_attribute(a))
        .collect();

    if !all_attrs.is_empty() {
        output.push_str(&all_attrs.join(", "));
    }

    if el.children.is_empty() {
        if el.self_closing {
            output.push(')');
        } else {
            output.push_str(") {}");
        }
    } else if el.children.len() == 1 {
        output.push_str(") {\n");
        output.push_str(&generate_rsx(&el.children[0], indent + 1));
        output.push('\n');
        output.push_str(&prefix);
        output.push('}');
    } else {
        output.push_str(") {\n");
        for child in &el.children {
            let child_rsx = generate_rsx(child, indent + 1);
            if !child_rsx.is_empty() {
                output.push_str(&child_rsx);
                output.push('\n');
            }
        }
        output.push_str(&prefix);
        output.push('}');
    }

    output
}

fn generate_attribute(attr: &HtmlAttribute) -> String {
    // Handle on:click={handler} event syntax
    if attr.name.starts_with("on:") {
        let event_name = attr.name.strip_prefix("on:").unwrap_or(&attr.name);
        return match &attr.value {
            HtmlAttributeValue::Expression(expr) => {
                format!("on:{} = {}", event_name, expr)
            }
            HtmlAttributeValue::Literal(val) => {
                format!("on:{} = \"{}\"", event_name, val)
            }
            _ => format!("on:{}", event_name),
        };
    }

    match &attr.value {
        HtmlAttributeValue::Literal(val) => {
            format!("{}: \"{}\"", attr.name, val)
        }
        HtmlAttributeValue::Expression(expr) => {
            format!("{}: {}", attr.name, expr)
        }
        HtmlAttributeValue::Boolean(true) => attr.name.clone(),
        HtmlAttributeValue::Boolean(false) => {
            format!("{}: false", attr.name)
        }
        HtmlAttributeValue::Shorthand(name) => {
            format!("{}: {}", attr.name, name)
        }
        HtmlAttributeValue::Conditional(cond, then_val, else_val) => {
            format!(
                "{}: if {} {{ \"{}\" }} else {{ \"{}\" }}",
                attr.name, cond, then_val, else_val
            )
        }
    }
}

fn generate_component(comp: &HtmlComponent, indent: usize) -> String {
    let prefix = "    ".repeat(indent);
    let mut output = format!("{}{}(", prefix, comp.name);

    if !comp.attributes.is_empty() {
        let attrs: Vec<String> = comp.attributes.iter().map(generate_attribute).collect();
        output.push_str(&attrs.join(", "));
    }

    if comp.children.is_empty() {
        output.push(')');
    } else {
        output.push_str(") {\n");
        for child in &comp.children {
            let child_rsx = generate_rsx(child, indent + 1);
            if !child_rsx.is_empty() {
                output.push_str(&child_rsx);
                output.push('\n');
            }
        }
        output.push_str(&prefix);
        output.push('}');
    }

    output
}

fn generate_each(each: &HtmlEach, indent: usize) -> String {
    let prefix = "    ".repeat(indent);
    let mut output = format!(
        "{}for {} in {}.iter() {{\n",
        prefix, each.item_name, each.iterable
    );

    for child in &each.body {
        let child_rsx = generate_rsx(child, indent + 1);
        if !child_rsx.is_empty() {
            output.push_str(&child_rsx);
            output.push('\n');
        }
    }

    output.push_str(&prefix);
    output.push('}');
    output
}

fn generate_if(if_block: &HtmlIf, indent: usize) -> String {
    let prefix = "    ".repeat(indent);
    let mut output = format!("{}if {} {{\n", prefix, if_block.condition);

    for child in &if_block.then_body {
        let child_rsx = generate_rsx(child, indent + 1);
        if !child_rsx.is_empty() {
            output.push_str(&child_rsx);
            output.push('\n');
        }
    }

    if let Some(else_body) = &if_block.else_body {
        output.push_str(&format!("{} }} else {{\n", prefix));
        for child in else_body {
            let child_rsx = generate_rsx(child, indent + 1);
            if !child_rsx.is_empty() {
                output.push_str(&child_rsx);
                output.push('\n');
            }
        }
    }

    output.push_str(&prefix);
    output.push('}');
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{HtmlComponent, HtmlElement};
    use crate::parser::parse_html;

    #[test]
    fn test_simple_element() {
        let html = r#"<div class="test">Hello</div>"#;
        let node = parse_html(html).unwrap();
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.contains("div("));
        assert!(rsx.contains("Hello"));
    }

    #[test]
    fn test_nested_elements() {
        let html = r#"<div><span>Hello</span></div>"#;
        let node = parse_html(html).unwrap();
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.contains("div("));
        assert!(rsx.contains("span("));
    }

    #[test]
    fn test_component() {
        let node = HtmlNode::Component(HtmlComponent {
            name: "Card".to_string(),
            attributes: vec![HtmlAttribute {
                name: "title".to_string(),
                value: HtmlAttributeValue::Literal("Hello".to_string()),
            }],
            children: vec![HtmlNode::Text("Content".to_string())],
        });
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.contains("Card("));
        assert!(rsx.contains("title: \"Hello\""));
        assert!(rsx.contains("Content"));
    }

    #[test]
    fn test_event_handler() {
        // Test with programmatic AST since html5ever may handle on: differently
        let node = HtmlNode::Element(HtmlElement {
            tag: "button".to_string(),
            attributes: vec![HtmlAttribute {
                name: "on:click".to_string(),
                value: HtmlAttributeValue::Expression("handle_click".to_string()),
            }],
            children: vec![HtmlNode::Text("Click".to_string())],
            self_closing: false,
        });
        let rsx = generate_rsx(&node, 0);
        eprintln!("Generated RSX:\n{}", rsx);
        assert!(rsx.contains("on:click"), "RSX should contain on:click");
        assert!(
            rsx.contains("handle_click"),
            "RSX should contain handle_click"
        );
    }

    #[test]
    fn test_each_loop() {
        let node = HtmlNode::EachLoop(HtmlEach {
            iterable: "items".to_string(),
            item_name: "item".to_string(),
            index_name: None,
            body: vec![HtmlNode::Text("item".to_string())],
        });
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.contains("for item in items.iter()"));
    }

    #[test]
    fn test_if_block() {
        let node = HtmlNode::IfBlock(HtmlIf {
            condition: "show".to_string(),
            then_body: vec![HtmlNode::Text("yes".to_string())],
            else_body: Some(vec![HtmlNode::Text("no".to_string())]),
        });
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.contains("if show"));
        assert!(rsx.contains("else"));
    }

    // --- Codegen edge case tests ---

    #[test]
    fn test_html_shorthand_attribute() {
        let node = HtmlNode::Element(HtmlElement {
            tag: "input".to_string(),
            attributes: vec![HtmlAttribute {
                name: "value".to_string(),
                value: HtmlAttributeValue::Shorthand("my_value".to_string()),
            }],
            children: vec![],
            self_closing: false,
        });
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.contains("value: my_value"));
    }

    #[test]
    fn test_html_conditional_attribute() {
        let node = HtmlNode::Element(HtmlElement {
            tag: "div".to_string(),
            attributes: vec![HtmlAttribute {
                name: "class".to_string(),
                value: HtmlAttributeValue::Conditional(
                    "active".to_string(),
                    "active-class".to_string(),
                    "inactive-class".to_string(),
                ),
            }],
            children: vec![],
            self_closing: false,
        });
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.contains("class: if active"));
        assert!(rsx.contains("active-class"));
        assert!(rsx.contains("inactive-class"));
    }

    #[test]
    fn test_html_raw_html_node() {
        let node = HtmlNode::RawHtml("my_html_content".to_string());
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.contains("rsx!(Raw(my_html_content))"));
    }

    #[test]
    fn test_html_fragment_empty() {
        let node = HtmlNode::Fragment(vec![]);
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.contains("rsx! {}"));
    }

    #[test]
    fn test_html_fragment_multiple_children() {
        let node = HtmlNode::Fragment(vec![
            HtmlNode::Text("a".to_string()),
            HtmlNode::Text("b".to_string()),
            HtmlNode::Text("c".to_string()),
        ]);
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.contains("rsx! {"));
        assert!(rsx.contains("\"a\""));
        assert!(rsx.contains("\"b\""));
        assert!(rsx.contains("\"c\""));
    }

    #[test]
    fn test_html_comment_node_empty() {
        let node = HtmlNode::Comment("ignored".to_string());
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.is_empty(), "Comment should produce empty string");
    }

    #[test]
    fn test_html_indentation_level_0() {
        let node = HtmlNode::Element(HtmlElement {
            tag: "div".to_string(),
            attributes: vec![],
            children: vec![HtmlNode::Text("hi".to_string())],
            self_closing: false,
        });
        let rsx = generate_rsx(&node, 0);
        assert!(!rsx.starts_with("    "));
    }

    #[test]
    fn test_html_indentation_level_2() {
        let node = HtmlNode::Element(HtmlElement {
            tag: "span".to_string(),
            attributes: vec![],
            children: vec![],
            self_closing: false,
        });
        let rsx = generate_rsx(&node, 2);
        assert!(rsx.starts_with("        "));
    }

    #[test]
    fn test_html_deeply_nested_rsx() {
        let mut root = HtmlNode::Text("leaf".to_string());
        for _ in 0..10 {
            root = HtmlNode::Element(HtmlElement {
                tag: "div".to_string(),
                attributes: vec![],
                children: vec![root],
                self_closing: false,
            });
        }
        let rsx = generate_rsx(&root, 0);
        assert!(rsx.contains("div("));
        assert!(rsx.contains("leaf"));
    }

    #[test]
    fn test_html_component_no_children() {
        let node = HtmlNode::Component(HtmlComponent {
            name: "Spacer".to_string(),
            attributes: vec![],
            children: vec![],
        });
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.contains("Spacer()"));
        assert!(!rsx.contains("{"));
    }

    #[test]
    fn test_html_component_multiple_children() {
        let node = HtmlNode::Component(HtmlComponent {
            name: "Card".to_string(),
            attributes: vec![],
            children: vec![
                HtmlNode::Text("first".to_string()),
                HtmlNode::Text("second".to_string()),
            ],
        });
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.contains("Card("));
        assert!(rsx.contains("first"));
        assert!(rsx.contains("second"));
    }

    #[test]
    fn test_html_component_with_props() {
        let node = HtmlNode::Component(HtmlComponent {
            name: "Button".to_string(),
            attributes: vec![
                HtmlAttribute {
                    name: "label".to_string(),
                    value: HtmlAttributeValue::Literal("Click me".to_string()),
                },
                HtmlAttribute {
                    name: "variant".to_string(),
                    value: HtmlAttributeValue::Literal("primary".to_string()),
                },
            ],
            children: vec![],
        });
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.contains("label: \"Click me\""));
        assert!(rsx.contains("variant: \"primary\""));
    }

    #[test]
    fn test_html_event_handler_expression() {
        let node = HtmlNode::Element(HtmlElement {
            tag: "button".to_string(),
            attributes: vec![HtmlAttribute {
                name: "on:click".to_string(),
                value: HtmlAttributeValue::Expression("handle_click".to_string()),
            }],
            children: vec![],
            self_closing: false,
        });
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.contains("on:click = handle_click"));
    }

    #[test]
    fn test_html_event_handler_literal() {
        let node = HtmlNode::Element(HtmlElement {
            tag: "button".to_string(),
            attributes: vec![HtmlAttribute {
                name: "on:click".to_string(),
                value: HtmlAttributeValue::Literal("doSomething".to_string()),
            }],
            children: vec![],
            self_closing: false,
        });
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.contains("on:click = \"doSomething\""));
    }

    #[test]
    fn test_html_event_on_change() {
        let node = HtmlNode::Element(HtmlElement {
            tag: "input".to_string(),
            attributes: vec![HtmlAttribute {
                name: "on:change".to_string(),
                value: HtmlAttributeValue::Expression("on_change".to_string()),
            }],
            children: vec![],
            self_closing: false,
        });
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.contains("on:change = on_change"));
    }

    #[test]
    fn test_html_each_with_element_body() {
        let node = HtmlNode::EachLoop(HtmlEach {
            iterable: "items".to_string(),
            item_name: "item".to_string(),
            index_name: None,
            body: vec![HtmlNode::Element(HtmlElement {
                tag: "li".to_string(),
                attributes: vec![],
                children: vec![HtmlNode::Expression("item".to_string())],
                self_closing: false,
            })],
        });
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.contains("for item in items.iter()"));
        assert!(rsx.contains("li("));
    }

    #[test]
    fn test_html_if_else_codegen() {
        let node = HtmlNode::IfBlock(HtmlIf {
            condition: "is_admin".to_string(),
            then_body: vec![HtmlNode::Text("admin view".to_string())],
            else_body: Some(vec![HtmlNode::Text("user view".to_string())]),
        });
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.contains("if is_admin"));
        assert!(rsx.contains("} else {"));
        assert!(rsx.contains("admin view"));
        assert!(rsx.contains("user view"));
    }

    #[test]
    fn test_html_if_no_else() {
        let node = HtmlNode::IfBlock(HtmlIf {
            condition: "show".to_string(),
            then_body: vec![HtmlNode::Text("visible".to_string())],
            else_body: None,
        });
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.contains("if show"));
        assert!(!rsx.contains("else"));
    }

    #[test]
    fn test_html_expression_node() {
        let node = HtmlNode::Expression("user.name".to_string());
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.contains("\"{user.name}\""));
    }

    #[test]
    fn test_html_text_empty_string() {
        let node = HtmlNode::Text("".to_string());
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.is_empty());
    }

    #[test]
    fn test_html_attribute_literal() {
        let node = HtmlNode::Element(HtmlElement {
            tag: "a".to_string(),
            attributes: vec![HtmlAttribute {
                name: "href".to_string(),
                value: HtmlAttributeValue::Literal("/home".to_string()),
            }],
            children: vec![],
            self_closing: false,
        });
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.contains("href: \"/home\""));
    }

    #[test]
    fn test_html_attribute_expression() {
        let node = HtmlNode::Element(HtmlElement {
            tag: "div".to_string(),
            attributes: vec![HtmlAttribute {
                name: "class".to_string(),
                value: HtmlAttributeValue::Expression("my_class".to_string()),
            }],
            children: vec![],
            self_closing: false,
        });
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.contains("class: my_class"));
    }

    #[test]
    fn test_html_attribute_boolean_true() {
        let node = HtmlNode::Element(HtmlElement {
            tag: "input".to_string(),
            attributes: vec![HtmlAttribute {
                name: "disabled".to_string(),
                value: HtmlAttributeValue::Boolean(true),
            }],
            children: vec![],
            self_closing: false,
        });
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.contains("disabled"));
        assert!(!rsx.contains("disabled:"));
    }

    #[test]
    fn test_html_attribute_boolean_false() {
        let node = HtmlNode::Element(HtmlElement {
            tag: "input".to_string(),
            attributes: vec![HtmlAttribute {
                name: "disabled".to_string(),
                value: HtmlAttributeValue::Boolean(false),
            }],
            children: vec![],
            self_closing: false,
        });
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.contains("disabled: false"));
    }

    #[test]
    fn test_html_element_self_closing() {
        let node = HtmlNode::Element(HtmlElement {
            tag: "br".to_string(),
            attributes: vec![],
            children: vec![],
            self_closing: true,
        });
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.contains("br("));
    }

    #[test]
    fn test_html_component_shorthand_attribute() {
        let node = HtmlNode::Component(HtmlComponent {
            name: "Avatar".to_string(),
            attributes: vec![HtmlAttribute {
                name: "src".to_string(),
                value: HtmlAttributeValue::Shorthand("avatar_url".to_string()),
            }],
            children: vec![],
        });
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.contains("src: avatar_url"));
    }

    #[test]
    fn test_html_element_multiple_children_newlines() {
        let node = HtmlNode::Element(HtmlElement {
            tag: "ul".to_string(),
            attributes: vec![],
            children: vec![
                HtmlNode::Element(HtmlElement {
                    tag: "li".to_string(),
                    attributes: vec![],
                    children: vec![HtmlNode::Text("a".to_string())],
                    self_closing: false,
                }),
                HtmlNode::Element(HtmlElement {
                    tag: "li".to_string(),
                    attributes: vec![],
                    children: vec![HtmlNode::Text("b".to_string())],
                    self_closing: false,
                }),
            ],
            self_closing: false,
        });
        let rsx = generate_rsx(&node, 0);
        assert!(rsx.contains("li("));
        let li_count = rsx.matches("li(").count();
        assert_eq!(li_count, 2);
    }
}
