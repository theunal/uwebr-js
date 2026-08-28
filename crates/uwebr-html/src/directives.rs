use crate::ast::{HtmlEach, HtmlIf, HtmlNode};

/// Post-process HTML AST to expand template directives in text nodes
/// Handles: {expression}, {#each}, {#if}, {@html}
pub fn expand_directives(node: &mut HtmlNode) {
    match node {
        HtmlNode::Element(el) => {
            expand_children(&mut el.children);
        }
        HtmlNode::Component(comp) => {
            expand_children(&mut comp.children);
        }
        HtmlNode::Text(text) => {
            // Check if this text node contains template directives
            if text.contains('{') {
                *node = parse_text_as_nodes(text);
            }
        }
        _ => {}
    }
}

fn expand_children(children: &mut Vec<HtmlNode>) {
    // First pass: reassemble split block directives
    reassemble_block_directives(children);

    // Second pass: expand directives in each child
    let mut i = 0;
    while i < children.len() {
        expand_directives(&mut children[i]);
        i += 1;
    }
}

/// Reassemble block directives that were split by html5ever into multiple text nodes
/// e.g., [Text("{#each items as item}"), Element(li), Text("{/each}")] → [EachLoop]
fn reassemble_block_directives(children: &mut Vec<HtmlNode>) {
    let mut i = 0;
    while i < children.len() {
        if let HtmlNode::Text(text) = &children[i] {
            let trimmed = text.trim();

            // Check for {#each ...} opening tag
            if trimmed.starts_with("{#each ") {
                // Look for matching {/each} in subsequent siblings
                let mut end_idx = None;
                for j in (i + 1)..children.len() {
                    if let HtmlNode::Text(t) = &children[j] {
                        if t.trim() == "{/each}" {
                            end_idx = Some(j);
                            break;
                        }
                    }
                }

                if let Some(end) = end_idx {
                    // Collect all children between {#each} and {/each}
                    let mut body_children: Vec<HtmlNode> = Vec::new();
                    for k in (i + 1)..end {
                        body_children.push(children[k].clone());
                    }

                    // Parse the opening tag
                    if let Some(each_node) = parse_each_with_body(trimmed, body_children) {
                        // Replace the range with the assembled directive
                        children.drain(i..=end);
                        children.insert(i, each_node);
                        continue;
                    }
                }
            }

            // Check for {#if ...} opening tag
            if trimmed.starts_with("{#if ") {
                // Look for matching {/if} in subsequent siblings
                let mut end_idx = None;
                for j in (i + 1)..children.len() {
                    if let HtmlNode::Text(t) = &children[j] {
                        if t.trim() == "{/if}" {
                            end_idx = Some(j);
                            break;
                        }
                    }
                }

                if let Some(end) = end_idx {
                    // Collect all children between {#if} and {/if}
                    let mut body_children: Vec<HtmlNode> = Vec::new();
                    for k in (i + 1)..end {
                        body_children.push(children[k].clone());
                    }

                    // Parse the opening tag and assemble
                    if let Some(if_node) = parse_if_with_body(trimmed, body_children) {
                        // Replace the range with the assembled directive
                        children.drain(i..=end);
                        children.insert(i, if_node);
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
}

/// Parse {#each ...} with pre-collected body children
fn parse_each_with_body(text: &str, body_children: Vec<HtmlNode>) -> Option<HtmlNode> {
    let rest = text.strip_prefix("{#each ")?;
    let (iterable, rest) = split_at_word(rest, "as")?;
    let (item_name, _) = split_at_brace(rest)?;

    Some(HtmlNode::EachLoop(HtmlEach {
        iterable: iterable.trim().to_string(),
        item_name: item_name.trim().to_string(),
        index_name: None,
        body: if body_children.is_empty() {
            vec![HtmlNode::Text(String::new())]
        } else {
            body_children
        },
    }))
}

/// Parse {#if ...} with pre-collected body children
fn parse_if_with_body(text: &str, body_children: Vec<HtmlNode>) -> Option<HtmlNode> {
    let rest = text.strip_prefix("{#if ")?;
    let (condition, _) = split_at_brace(rest)?;

    Some(HtmlNode::IfBlock(HtmlIf {
        condition: condition.trim().to_string(),
        then_body: if body_children.is_empty() {
            vec![HtmlNode::Text(String::new())]
        } else {
            body_children
        },
        else_body: None,
    }))
}

/// Parse a text string that may contain template directives
/// Returns a single node or a fragment
fn parse_text_as_nodes(text: &str) -> HtmlNode {
    let trimmed = text.trim();

    // Check for block directives: {#each ...}, {#if ...}
    if let Some(result) = parse_block_directive(trimmed) {
        return result;
    }

    // Check for raw HTML: {@html expr}
    if trimmed.starts_with("{@html ") && trimmed.ends_with('}') {
        let expr = trimmed[7..trimmed.len() - 1].trim().to_string();
        return HtmlNode::RawHtml(expr);
    }

    // Check for simple expression: {expr}
    if trimmed.starts_with('{') && trimmed.ends_with('}') && !trimmed.contains("{#") {
        let expr = &trimmed[1..trimmed.len() - 1].trim().to_string();
        if !expr.is_empty() {
            return HtmlNode::Expression(expr.clone());
        }
    }

    // Check for mixed content (text + expressions)
    if trimmed.contains('{') && !trimmed.starts_with('{') {
        return parse_mixed_content(trimmed);
    }

    HtmlNode::Text(trimmed.to_string())
}

/// Parse block directives like {#each} and {#if}
fn parse_block_directive(text: &str) -> Option<HtmlNode> {
    if text.starts_with("{#each ") {
        parse_each_directive(text)
    } else if text.starts_with("{#if ") {
        parse_if_directive(text)
    } else {
        None
    }
}

/// Parse {#each items as item}...{/each}
fn parse_each_directive(text: &str) -> Option<HtmlNode> {
    let rest = text.strip_prefix("{#each ")?;
    let (iterable, rest) = split_at_word(rest, "as")?;
    let (item_name, rest) = split_at_brace(rest)?;

    let body_text = rest.strip_suffix("{/each}")?.trim();
    let body = if body_text.contains('{') {
        vec![parse_text_as_nodes(body_text)]
    } else {
        vec![HtmlNode::Text(body_text.to_string())]
    };

    Some(HtmlNode::EachLoop(HtmlEach {
        iterable: iterable.trim().to_string(),
        item_name: item_name.trim().to_string(),
        index_name: None,
        body,
    }))
}

/// Parse {#if condition}...{:else}...{/if}
fn parse_if_directive(text: &str) -> Option<HtmlNode> {
    let rest = text.strip_prefix("{#if ")?;
    let (condition, rest) = split_at_brace(rest)?;

    let body = rest.strip_suffix("{/if}")?;
    let (then_body, else_body) = if let Some(idx) = body.find("{:else}") {
        let then = body[..idx].trim();
        let else_ = body[idx + 7..].trim();
        (
            vec![parse_text_as_nodes(then)],
            Some(vec![parse_text_as_nodes(else_)]),
        )
    } else {
        (vec![parse_text_as_nodes(body.trim())], None)
    };

    Some(HtmlNode::IfBlock(HtmlIf {
        condition: condition.trim().to_string(),
        then_body,
        else_body,
    }))
}

/// Split at the first occurrence of a word boundary
fn split_at_word<'a>(text: &'a str, word: &str) -> Option<(&'a str, &'a str)> {
    let idx = text.find(word)?;
    let before = text[..idx].trim_end();
    let after = text[idx + word.len()..].trim_start();
    Some((before, after))
}

/// Split at the first closing brace
fn split_at_brace(text: &str) -> Option<(&str, &str)> {
    let idx = text.find('}')?;
    Some((&text[..idx], &text[idx + 1..]))
}

/// Parse mixed content with text and expressions
/// e.g., "Hello {name}!" → Text("Hello ") + Expression("name") + Text("!")
fn parse_mixed_content(text: &str) -> HtmlNode {
    let mut nodes = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '{' {
            // Save accumulated text
            if !current.is_empty() {
                nodes.push(HtmlNode::Text(current.trim().to_string()));
                current.clear();
            }

            // Read expression until '}'
            let mut expr = String::new();
            let mut depth = 1;
            for ec in &mut chars {
                if ec == '{' {
                    depth += 1;
                } else if ec == '}' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                expr.push(ec);
            }

            let expr = expr.trim().to_string();
            if !expr.is_empty() {
                nodes.push(HtmlNode::Expression(expr));
            }
        } else {
            current.push(c);
        }
    }

    // Remaining text
    if !current.is_empty() {
        let trimmed = current.trim().to_string();
        if !trimmed.is_empty() {
            nodes.push(HtmlNode::Text(trimmed));
        }
    }

    if nodes.len() == 1 {
        nodes.into_iter().next().unwrap()
    } else {
        HtmlNode::Fragment(nodes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_expression() {
        let mut node = HtmlNode::Text("{name}".to_string());
        expand_directives(&mut node);
        match node {
            HtmlNode::Expression(expr) => assert_eq!(expr, "name"),
            _ => panic!("Expected expression"),
        }
    }

    #[test]
    fn test_mixed_content() {
        let mut node = HtmlNode::Text("Hello {name}!".to_string());
        expand_directives(&mut node);
        match node {
            HtmlNode::Fragment(nodes) => {
                assert!(nodes.len() >= 2);
                assert_eq!(nodes[0], HtmlNode::Text("Hello".to_string()));
                assert_eq!(nodes[1], HtmlNode::Expression("name".to_string()));
            }
            _ => panic!("Expected fragment"),
        }
    }

    #[test]
    fn test_each_directive() {
        let mut node = HtmlNode::Text("{#each items as item}...{/each}".to_string());
        expand_directives(&mut node);
        match node {
            HtmlNode::EachLoop(each) => {
                assert_eq!(each.iterable, "items");
                assert_eq!(each.item_name, "item");
            }
            _ => panic!("Expected each loop"),
        }
    }

    #[test]
    fn test_if_directive() {
        let mut node = HtmlNode::Text("{#if show}yes{/if}".to_string());
        expand_directives(&mut node);
        match node {
            HtmlNode::IfBlock(if_block) => {
                assert_eq!(if_block.condition, "show");
                assert!(if_block.else_body.is_none());
            }
            _ => panic!("Expected if block"),
        }
    }

    #[test]
    fn test_if_else_directive() {
        let mut node = HtmlNode::Text("{#if show}yes{:else}no{/if}".to_string());
        expand_directives(&mut node);
        match node {
            HtmlNode::IfBlock(if_block) => {
                assert_eq!(if_block.condition, "show");
                assert!(if_block.else_body.is_some());
            }
            _ => panic!("Expected if block"),
        }
    }
}
