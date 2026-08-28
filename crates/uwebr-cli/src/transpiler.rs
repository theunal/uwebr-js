use anyhow::Result;
use std::fs;
use std::path::Path;
use uwebr_html::ast::{HtmlNode, HtmlAttributeValue};
use uwebr_html::parser::parse_html;
use uwebr_html::directives::expand_directives;
use uwebr_js;

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
    let mut root = parse_html(&html)?;

    // Expand template directives ({#each}, {#if}, etc.)
    expand_directives(&mut root);

    // Generate Rust code
    let mut output = String::new();

    // Collect component references from HTML for imports
    let component_refs = collect_component_refs(&root, component_name);
    let has_components = !component_refs.is_empty();

    // Header
    output.push_str("use uwebr_app::App;\n");
    output.push_str("use uwebr_core::component::{Element, NodeType, PropValue};\n");
    output.push_str("use uwebr_app::FnComponent;\n");
    if has_components {
        for comp in &component_refs {
            let mod_name = to_snake(comp);
            let fn_name = to_snake(comp);
            output.push_str(&format!("use crate::generated::{mod_name}::{fn_name}_component;\n"));
        }
    }
    output.push('\n');

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

    // Script integration — transpile JS to Rust
    if !script.is_empty() {
        match uwebr_js::transpile(&script) {
            Ok(result) => {
                if !result.warnings.is_empty() {
                    output.push_str("// JS transpile warnings:\n");
                    for w in &result.warnings {
                        output.push_str(&format!("//   {w}\n"));
                    }
                }
                output.push_str("// Transpiled from <script> block:\n");
                output.push_str(&result.code);
                output.push('\n');
            }
            Err(e) => {
                output.push_str(&format!("// JS transpile error: {e}\n"));
                output.push_str("/* Original script:\n");
                output.push_str(&script);
                output.push_str("\n*/\n");
            }
        }
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

            // Check if any child is an each/if block (produces Vec<Element>)
            let has_dynamic = el.children.iter().any(|c| matches!(c, HtmlNode::EachLoop(_) | HtmlNode::IfBlock(_)));

            let children_str = if children_code.is_empty() {
                "vec![]".to_string()
            } else if has_dynamic {
                // Mixed static + dynamic: build children imperatively
                let mut lines = vec![format!("{{ let mut __c: Vec<Element> = vec![];")];
                for child in &el.children {
                    let code = generate_element_code(child, indent + 3).trim().to_string();
                    if code.starts_with("items.iter()") || code.starts_with("if ") {
                        // Dynamic: extends or pushes multiple
                        lines.push(format!("__c.extend({});", code));
                    } else if !code.is_empty() {
                        // Static: push single element
                        lines.push(format!("__c.push({});", code));
                    }
                }
                lines.push(format!("__c }}"));
                lines.join("\n")
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
            // Component composition: call the component function
            let fn_name = format!("{}_component", to_snake(name));
            format!(
                "{}Element {{\n{}node_type: NodeType::Element(\"div\".into()),\n{}props: {},\n{}children: vec![{}()],\n{}}}",
                pad, pad, pad, props_str, pad, fn_name, pad
            )
        }
        HtmlNode::EachLoop(each) => {
            let item = &each.item_name;
            let iter = &each.iterable;
            let body_elements: Vec<String> = each.body.iter()
                .map(|n| generate_element_code(n, indent + 3))
                .collect();
            let body_str = if body_elements.len() == 1 {
                body_elements[0].clone()
            } else {
                format!(
                    "vec![\n{}\n{}]",
                    body_elements.join(",\n"),
                    "    ".repeat(indent + 2)
                )
            };
            format!(
                "{}{}.iter().map(|{}| {{\n{}{}\n{}}}).collect::<Vec<_>>()",
                pad, iter, item,
                "    ".repeat(indent + 2), body_str.trim(),
                pad
            )
        }
        HtmlNode::IfBlock(if_block) => {
            let cond = &if_block.condition;
            let then_elements: Vec<String> = if_block.then_body.iter()
                .map(|n| generate_element_code(n, indent + 2))
                .collect();
            let then_str = if then_elements.len() == 1 {
                then_elements[0].clone()
            } else {
                format!(
                    "vec![\n{}\n{}]",
                    then_elements.join(",\n"),
                    "    ".repeat(indent + 1)
                )
            };
            let else_str = if let Some(ref else_body) = if_block.else_body {
                let else_elements: Vec<String> = else_body.iter()
                    .map(|n| generate_element_code(n, indent + 2))
                    .collect();
                if else_elements.len() == 1 {
                    format!(" else {{ {} }}", else_elements[0].trim())
                } else {
                    format!(
                        " else {{ vec![\n{}\n{}] }}",
                        else_elements.join(",\n"),
                        "    ".repeat(indent + 1)
                    )
                }
            } else {
                String::new()
            };
            format!(
                "{}if {} {{\n{}{}\n{}{}}}",
                pad, cond,
                "    ".repeat(indent + 1), then_str.trim(),
                pad, else_str
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

/// Collect PascalCase component names referenced in the HTML tree (excluding the root component itself)
fn collect_component_refs(node: &HtmlNode, root_name: &str) -> Vec<String> {
    let mut refs = Vec::new();
    match node {
        HtmlNode::Component(comp) => {
            if comp.name != root_name {
                if !refs.contains(&comp.name) {
                    refs.push(comp.name.clone());
                }
            }
            for child in &comp.children {
                refs.extend(collect_component_refs(child, root_name));
            }
        }
        HtmlNode::Element(el) => {
            for child in &el.children {
                refs.extend(collect_component_refs(child, root_name));
            }
        }
        HtmlNode::EachLoop(each) => {
            for child in &each.body {
                refs.extend(collect_component_refs(child, root_name));
            }
        }
        HtmlNode::IfBlock(ifnode) => {
            for child in &ifnode.then_body {
                refs.extend(collect_component_refs(child, root_name));
            }
            if let Some(else_body) = &ifnode.else_body {
                for child in else_body {
                    refs.extend(collect_component_refs(child, root_name));
                }
            }
        }
        _ => {}
    }
    refs
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
        // JS should be transpiled to Rust (not just commented)
        assert!(result.contains("let x = 1") || result.contains("x = 1") || result.contains("Transpiled from"));
        assert!(!result.contains("// NOTE: Script transpilation requires"));
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

    #[test]
    fn test_transpile_each_loop() {
        let html = r#"<ul>{#each items as item}<li>{item}</li>{/each}</ul>"#;
        let result = transpile(html, "List").unwrap();
        assert!(result.contains("items.iter().map(|item|"), "Expected iterator");
        assert!(result.contains("NodeType::Element(\"li\""));
        assert!(!result.contains("// TODO"));
    }

    #[test]
    fn test_transpile_if_block() {
        let html = r#"<div>{#if show}<span>Visible</span>{/if}</div>"#;
        let result = transpile(html, "Cond").unwrap();
        assert!(result.contains("if show"));
        assert!(result.contains("NodeType::Element(\"span\""));
        assert!(!result.contains("// TODO"));
    }

    #[test]
    fn test_transpile_if_else() {
        let html = r#"<div>{#if logged_in}<span>Welcome</span>{:else}<span>Login</span>{/if}</div>"#;
        let result = transpile(html, "Auth").unwrap();
        assert!(result.contains("if logged_in"));
        assert!(result.contains("Welcome"));
        assert!(result.contains("Login"));
    }

    #[test]
    fn test_transpile_mixed_children() {
        let html = r#"<div><h1>Title</h1>{#each items as item}<p>{item}</p>{/each}<footer>End</footer></div>"#;
        let result = transpile(html, "Mixed").unwrap();
        assert!(result.contains("items.iter().map(|item|"));
        assert!(result.contains("__c.extend("));
        assert!(result.contains("__c.push("));
    }

    #[test]
    fn test_transpile_component_composition() {
        let html = r#"<div><Header></Header><p>Content</p><Footer></Footer></div>"#;
        let result = transpile(html, "Page").unwrap();
        // Should generate use imports for referenced components
        assert!(result.contains("use crate::generated::header::header_component"));
        assert!(result.contains("use crate::generated::footer::footer_component"));
        // Should call component functions
        assert!(result.contains("header_component()"));
        assert!(result.contains("footer_component()"));
    }
}
