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
}
