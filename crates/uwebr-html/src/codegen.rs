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
                format!(
                    "{}rsx! {{\n{}\n{}}}",
                    prefix,
                    inner.join("\n"),
                    prefix
                )
            }
        }
        HtmlNode::Comment(_) => String::new(),
    }
}

fn generate_element(el: &HtmlElement, indent: usize) -> String {
    let prefix = "    ".repeat(indent);
    let mut output = format!("{}{}(", prefix, el.tag);

    // Attributes
    if !el.attributes.is_empty() {
        let attrs: Vec<String> = el
            .attributes
            .iter()
            .map(|a| generate_attribute(a))
            .collect();
        output.push_str(&attrs.join(", "));
    }

    // Children
    if el.children.is_empty() {
        if el.self_closing {
            output.push_str(")");
        } else {
            output.push_str(") {}");
        }
    } else if el.children.len() == 1 {
        output.push_str(") {\n");
        output.push_str(&generate_rsx(&el.children[0], indent + 1));
        output.push('\n');
        output.push_str(&prefix);
        output.push_str("}");
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
    match &attr.value {
        HtmlAttributeValue::Literal(val) => {
            format!("{}: \"{}\"", attr.name, val)
        }
        HtmlAttributeValue::Expression(expr) => {
            format!("{}: {}", attr.name, expr)
        }
        HtmlAttributeValue::Boolean(true) => {
            attr.name.clone()
        }
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
        let attrs: Vec<String> = comp
            .attributes
            .iter()
            .map(|a| generate_attribute(a))
            .collect();
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
    let mut output = format!("{}for {} in {}.iter() {{\n", prefix, each.item_name, each.iterable);

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
}
