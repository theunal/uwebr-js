use anyhow::Result;
use std::fs;
use std::path::Path;
use uwebr_html::ast::{HtmlNode, HtmlAttributeValue};
use uwebr_html::parser::parse_html;

/// Transpile a .uwebr file to Rust source code
pub fn transpile_file(path: &Path) -> Result<String> {
    let content = fs::read_to_string(path)?;
    let file_name = path.file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("Component");
    transpile(&content, file_name)
}

/// Transpile .uwebr content to Rust source code
pub fn transpile(content: &str, component_name: &str) -> Result<String> {
    // Extract <style> blocks
    let css = extract_tag(content, "style");
    // Extract <script> blocks
    let script = extract_tag(content, "script");
    // Get HTML (everything except style and script)
    let html = extract_html(content);

    // Parse HTML
    let root = parse_html(&html)?;

    // Generate Rust code
    let mut output = String::new();

    // Header
    output.push_str("use uwebr_app::App;\n");
    output.push_str("use uwebr_core::component::{Element, NodeType, PropValue};\n");
    output.push_str("use uwebr_app::FnComponent;\n\n");

    // CSS as a const
    if !css.is_empty() {
        output.push_str(&format!(
            "const CSS_{}: &str = r#\"{}\"#;\n\n",
            component_name.to_uppercase(),
            css
        ));
    }

    // Component function
    output.push_str(&format!("pub fn {}_component() -> Element {{\n", to_snake(component_name)));
    output.push_str(&format!("    // HTML from {}.uwebr\n", component_name));
    output.push_str(&generate_element_code(&root, 2));
    output.push_str("\n}\n\n");

    // Script integration (if any)
    if !script.is_empty() {
        output.push_str("// Script from <script> block:\n");
        output.push_str("// NOTE: Script transpilation requires uwebr-js\n");
        output.push_str(&format!("/*\n{}\n*/\n", script));
    }

    // Main function
    output.push_str("pub fn main() -> anyhow::Result<()> {\n");
    output.push_str(&format!("    let mut app = App::new(\"{}\");\n", component_name));
    if !css.is_empty() {
        output.push_str(&format!("    app = app.with_css(CSS_{});\n", component_name.to_uppercase()));
    }
    output.push_str("    app.with_component(FnComponent::new(|| {\n");
    output.push_str(&format!("        {}_component()\n", to_snake(component_name)));
    output.push_str("    }))\n");
    output.push_str("    .run()\n");
    output.push_str("}\n");

    Ok(output)
}

/// Generate Rust Element code from an HtmlNode
fn generate_element_code(node: &HtmlNode, indent: usize) -> String {
    let pad = "    ".repeat(indent);
    match node {
        HtmlNode::Element(el) => {
            let tag = &el.tag;
            let mut props = Vec::new();
            let mut children_code = Vec::new();

            for attr in &el.attributes {
                match &attr.value {
                    HtmlAttributeValue::Literal(val) => {
                        props.push(format!("(\"{}\".into(), PropValue::String(\"{}\".into()))", attr.name, val));
                    }
                    HtmlAttributeValue::Expression(expr) => {
                        props.push(format!("(\"{}\".into(), PropValue::String({}.into()))", attr.name, expr));
                    }
                    HtmlAttributeValue::Boolean(true) => {
                        props.push(format!("(\"{}\".into(), PropValue::Bool(true))", attr.name));
                    }
                    HtmlAttributeValue::Boolean(false) => {}
                    HtmlAttributeValue::Shorthand(name) => {
                        props.push(format!("(\"{}\".into(), PropValue::String({}.into()))", attr.name, name));
                    }
                    HtmlAttributeValue::Conditional(cond, then_val, else_val) => {
                        props.push(format!(
                            "(\"{}\".into(), PropValue::String(if {} {{ \"{}\" }} else {{ \"{}\" }}.into()))",
                            attr.name, cond, then_val, else_val
                        ));
                    }
                }
            }

            for child in &el.children {
                let child_code = generate_element_code(child, indent + 2);
                if !child_code.trim().is_empty() {
                    children_code.push(child_code);
                }
            }

            let props_str = if props.is_empty() {
                "vec![]".to_string()
            } else {
                format!("vec![\n{}\n{}]", props.join(",\n"), pad)
            };

            let children_str = if children_code.is_empty() {
                "vec![]".to_string()
            } else {
                format!(
                    "vec![\n{}\n{}]",
                    children_code.join(",\n"),
                    pad
                )
            };

            format!(
                "{}Element {{\n{}node_type: NodeType::Element(\"{}\".into()),\n{}props: {},\n{}children: {},\n{}}}",
                pad, pad, tag, pad, props_str, pad, children_str, pad
            )
        }
        HtmlNode::Text(text) => {
            format!(
                "{}Element {{ node_type: NodeType::Text(\"{}\".into()), props: vec![], children: vec![] }}",
                pad, text
            )
        }
        HtmlNode::Expression(expr) => {
            format!(
                "{}Element {{ node_type: NodeType::Text({}.to_string()), props: vec![], children: vec![] }}",
                pad, expr
            )
        }
        HtmlNode::Fragment(nodes) => {
            if nodes.is_empty() {
                return "Element { node_type: NodeType::Element(\"div\".into()), props: vec![], children: vec![] }".to_string();
            }
            if nodes.len() == 1 {
                return generate_element_code(&nodes[0], indent);
            }
            let children: Vec<String> = nodes.iter()
                .map(|n| generate_element_code(n, indent + 1))
                .collect();
            format!(
                "{}Element {{\n{}node_type: NodeType::Element(\"div\".into()),\n{}props: vec![],\n{}children: vec![\n{}\n{}],\n{}}}",
                pad, pad, pad, pad, children.join(",\n"), pad, pad
            )
        }
        HtmlNode::Component(comp) => {
            let name = &comp.name;
            let mut props = Vec::new();
            for attr in &comp.attributes {
                match &attr.value {
                    HtmlAttributeValue::Literal(val) => {
                        props.push(format!("(\"{}\".into(), PropValue::String(\"{}\".into()))", attr.name, val));
                    }
                    HtmlAttributeValue::Expression(expr) => {
                        props.push(format!("(\"{}\".into(), PropValue::String({}.into()))", attr.name, expr));
                    }
                    _ => {}
                }
            }
            let props_str = if props.is_empty() {
                "vec![]".to_string()
            } else {
                format!("vec![{}]", props.join(", "))
            };
            format!(
                "{}Element {{ node_type: NodeType::Component(\"{}\".into()), props: {}, children: vec![] }}",
                pad, name, props_str
            )
        }
        HtmlNode::EachLoop(each) => {
            let item = &each.item_name;
            let iter = &each.iterable;
            if each.body.len() == 1 {
                let child = generate_element_code(&each.body[0], indent + 1);
                format!(
                    "{}// TODO: each loop over {}\n{}// for {} in {} {{\n{}//     {}\n{}// }}",
                    pad, iter, pad, item, iter, pad, child.trim(), pad
                )
            } else {
                let children: Vec<String> = each.body.iter()
                    .map(|n| generate_element_code(n, indent + 2))
                    .collect();
                format!(
                    "{}// TODO: each loop over {}\n{}// for {} in {} {{\n{}\n{}// }}",
                    pad, iter, pad, item, iter, children.join("\n"), pad
                )
            }
        }
        HtmlNode::IfBlock(if_block) => {
            let cond = &if_block.condition;
            let then_children: Vec<String> = if_block.then_body.iter()
                .map(|n| generate_element_code(n, indent + 1))
                .collect();
            let then_child = then_children.join("\n");
            let else_code = if let Some(ref else_body) = if_block.else_body {
                let else_children: Vec<String> = else_body.iter()
                    .map(|n| generate_element_code(n, indent + 1))
                    .collect();
                format!("\n{} else {{\n{}\n{}}}", pad, else_children.join("\n"), pad)
            } else {
                String::new()
            };
            format!(
                "{}// if {} {{\n{}\n{}{}}}",
                pad, cond, then_child, pad, else_code
            )
        }
        HtmlNode::RawHtml(expr) => {
            format!("{}// Raw HTML: {}", pad, expr)
        }
        HtmlNode::Comment(text) => {
            format!("{}// {}", pad, text)
        }
    }
}

/// Extract content between <tag>...</tag> blocks
fn extract_tag(content: &str, tag: &str) -> String {
    let mut result = String::new();
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);

    let mut pos = 0;
    while let Some(start) = content[pos..].find(&open) {
        let start = pos + start + open.len();
        if let Some(end) = content[start..].find(&close) {
            result.push_str(&content[start..start + end]);
            pos = start + end + close.len();
        } else {
            break;
        }
    }

    result.trim().to_string()
}

/// Extract HTML content (everything except <style> and <script> blocks)
fn extract_html(content: &str) -> String {
    let mut result = content.to_string();

    // Remove <style>...</style>
    while let Some(start) = result.find("<style") {
        if let Some(tag_end) = result[start..].find('>') {
            let tag_end = start + tag_end + 1;
            if let Some(close_end) = result[tag_end..].find("</style>") {
                result = format!(
                    "{}{}",
                    &result[..start],
                    &result[tag_end + close_end + 8..]
                );
            } else {
                break;
            }
        } else {
            break;
        }
    }

    // Remove <script>...</script>
    while let Some(start) = result.find("<script") {
        if let Some(tag_end) = result[start..].find('>') {
            let tag_end = start + tag_end + 1;
            if let Some(close_end) = result[tag_end..].find("</script>") {
                result = format!(
                    "{}{}",
                    &result[..start],
                    &result[tag_end + close_end + 9..]
                );
            } else {
                break;
            }
        } else {
            break;
        }
    }

    result.trim().to_string()
}

/// Convert PascalCase or kebab-case to snake_case
pub fn to_snake(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        if c == '-' || c == ' ' {
            result.push('_');
        } else {
            result.push(c.to_lowercase().next().unwrap_or(c));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transpile_simple_element() {
        let html = r#"<div class="app"><h1>Hello</h1></div>"#;
        let result = transpile(html, "App").unwrap();
        assert!(result.contains("NodeType::Element(\"div\""));
        assert!(result.contains("NodeType::Element(\"h1\""));
        assert!(result.contains("NodeType::Text(\"Hello\""));
    }

    #[test]
    fn test_transpile_with_css() {
        let input = r#"<div class="box"><span>Hi</span></div>
<style>.box { width: 100px; }</style>"#;
        let result = transpile(input, "Box").unwrap();
        assert!(result.contains("CSS_BOX"));
        assert!(result.contains("width: 100px"));
    }

    #[test]
    fn test_transpile_with_script() {
        let input = r#"<div><p>Content</p></div>
<script>let x = 1;</script>"#;
        let result = transpile(input, "Page").unwrap();
        assert!(result.contains("let x = 1"));
    }

    #[test]
    fn test_transpile_attributes() {
        let html = r#"<button id="btn" class="primary" disabled>Click</button>"#;
        let result = transpile(html, "Btn").unwrap();
        assert!(result.contains("id"));
        assert!(result.contains("btn"));
    }

    #[test]
    fn test_transpile_nested() {
        let html = r#"<div><div><span>Deep</span></div></div>"#;
        let result = transpile(html, "Nested").unwrap();
        assert!(result.contains("NodeType::Element(\"div\""));
        assert!(result.contains("NodeType::Element(\"span\""));
    }

    #[test]
    fn test_to_snake() {
        assert_eq!(to_snake("App"), "app");
        assert_eq!(to_snake("MyComponent"), "my_component");
        assert_eq!(to_snake("my-app"), "my_app");
        assert_eq!(to_snake("Button"), "button");
    }

    #[test]
    fn test_extract_tag() {
        let content = r#"Hello <style>.a { color: red; }</style> World <script>let x = 1;</script> End"#;
        let css = extract_tag(content, "style");
        assert_eq!(css, ".a { color: red; }");
        let js = extract_tag(content, "script");
        assert_eq!(js, "let x = 1;");
    }

    #[test]
    fn test_extract_html() {
        let content = r#"<div>Hi</div><style>.a{}</style><script>let x;</script>"#;
        let html = extract_html(content);
        assert_eq!(html, "<div>Hi</div>");
    }

    #[test]
    fn test_transpile_empty() {
        let result = transpile("", "Empty").unwrap();
        assert!(result.contains("fn main"));
    }

    #[test]
    fn test_transpile_main_function() {
        let html = r#"<div>Hello</div>"#;
        let result = transpile(html, "Hello").unwrap();
        assert!(result.contains("pub fn main"));
        assert!(result.contains("App::new(\"Hello\")"));
        assert!(result.contains("hello_component()"));
    }
}
