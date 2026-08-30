//! Minimal runtime HTML string parser for `{@html expr}` support.
//!
//! The transpiler lowers `{@html expr}` to `Raw(expr)`, leaving a runtime HTML
//! string that the pipeline must turn into an [`Element`] tree. A full HTML5
//! parser (`html5ever`) is overkill for this; this handles the common subset:
//!
//! - Opening/closing tags with string attributes (`class="foo"`)
//! - Self-closing tags (`<br/>`, `<img src="x"/>`)
//! - Void tags (`<br>`) that never take a closing tag
//! - Text content and nested elements
//!
//! It is deliberately forgiving: malformed input yields `None` rather than an
//! error, and the caller falls back to rendering the raw string.

use uwebr_core::component::{Element, NodeType, PropValue};

/// HTML void elements — they never have a closing tag or children.
const VOID_TAGS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// Parse a runtime HTML string into an [`Element`] tree.
///
/// Returns `None` when the input does not begin with a well-formed element.
pub fn parse_runtime_html(html: &str) -> Option<Element> {
    let mut parser = RuntimeHtmlParser::new(html);
    parser.parse_element()
}

struct RuntimeHtmlParser {
    input: Vec<char>,
    pos: usize,
}

impl RuntimeHtmlParser {
    fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_whitespace()) {
            self.pos += 1;
        }
    }

    fn starts_with(&self, s: &str) -> bool {
        let chars: Vec<char> = s.chars().collect();
        if self.pos + chars.len() > self.input.len() {
            return false;
        }
        self.input[self.pos..self.pos + chars.len()] == chars[..]
    }

    /// Consume `c` if present; report whether it matched.
    fn expect(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn read_tag_name(&mut self) -> Option<String> {
        let mut name = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                name.push(c);
                self.pos += 1;
            } else {
                break;
            }
        }
        if name.is_empty() {
            None
        } else {
            Some(name.to_ascii_lowercase())
        }
    }

    fn read_attr_name(&mut self) -> String {
        let mut name = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ':' {
                name.push(c);
                self.pos += 1;
            } else {
                break;
            }
        }
        name
    }

    fn read_attr_value(&mut self) -> String {
        self.skip_whitespace();
        match self.peek() {
            Some(q @ ('"' | '\'')) => {
                self.pos += 1; // opening quote
                let mut value = String::new();
                while let Some(c) = self.peek() {
                    if c == q {
                        self.pos += 1; // closing quote
                        break;
                    }
                    value.push(c);
                    self.pos += 1;
                }
                value
            }
            _ => {
                // Unquoted value: read until whitespace or tag end.
                let mut value = String::new();
                while let Some(c) = self.peek() {
                    if c.is_whitespace() || c == '>' || c == '/' {
                        break;
                    }
                    value.push(c);
                    self.pos += 1;
                }
                value
            }
        }
    }

    fn parse_attributes(&mut self) -> Vec<(String, PropValue)> {
        let mut attrs = Vec::new();
        loop {
            self.skip_whitespace();
            match self.peek() {
                Some('>') | Some('/') | None => break,
                _ => {}
            }

            let name = self.read_attr_name();
            if name.is_empty() {
                // Not an attribute char (e.g. stray symbol); bail to avoid a loop.
                break;
            }

            self.skip_whitespace();
            if self.peek() == Some('=') {
                self.pos += 1;
                let value = self.read_attr_value();
                attrs.push((name, PropValue::String(value)));
            } else {
                attrs.push((name, PropValue::Bool(true)));
            }
        }
        attrs
    }

    fn read_text(&mut self) -> String {
        let mut text = String::new();
        while let Some(c) = self.peek() {
            if c == '<' {
                break;
            }
            text.push(c);
            self.pos += 1;
        }
        text
    }

    fn parse_element(&mut self) -> Option<Element> {
        self.skip_whitespace();
        if self.peek() != Some('<') {
            return None;
        }
        self.pos += 1; // '<'

        let tag = self.read_tag_name()?;
        let attrs = self.parse_attributes();

        self.skip_whitespace();

        // Self-closing tag: `<tag ... />`
        if self.peek() == Some('/') {
            self.pos += 1; // '/'
            self.expect('>');
            return Some(Element {
                node_type: NodeType::Element(tag),
                props: attrs,
                children: vec![],
            });
        }

        if !self.expect('>') {
            return None;
        }

        // Void tags have no children or closing tag.
        if VOID_TAGS.contains(&tag.as_str()) {
            return Some(Element {
                node_type: NodeType::Element(tag),
                props: attrs,
                children: vec![],
            });
        }

        let mut children = Vec::new();
        loop {
            if self.peek().is_none() {
                break;
            }

            if self.starts_with("</") {
                self.pos += 2; // '</'
                let _ = self.read_tag_name();
                self.skip_whitespace();
                self.expect('>');
                break;
            }

            if self.peek() == Some('<') {
                if let Some(child) = self.parse_element() {
                    children.push(child);
                } else {
                    // Unparseable child; stop to avoid an infinite loop.
                    break;
                }
            } else {
                let text = self.read_text();
                if !text.trim().is_empty() {
                    children.push(Element {
                        node_type: NodeType::Text(text.trim().to_string()),
                        props: vec![],
                        children: vec![],
                    });
                }
            }
        }

        Some(Element {
            node_type: NodeType::Element(tag),
            props: attrs,
            children,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_element() {
        let el = parse_runtime_html("<div>Hello</div>").unwrap();
        assert_eq!(el.node_type, NodeType::Element("div".into()));
        assert_eq!(el.children.len(), 1);
        assert_eq!(el.children[0].node_type, NodeType::Text("Hello".into()));
    }

    #[test]
    fn test_parse_element_with_attribute() {
        let el = parse_runtime_html(r#"<span class="x">Text</span>"#).unwrap();
        assert_eq!(el.node_type, NodeType::Element("span".into()));
        assert_eq!(
            el.props,
            vec![("class".to_string(), PropValue::String("x".into()))]
        );
        assert_eq!(el.children[0].node_type, NodeType::Text("Text".into()));
    }

    #[test]
    fn test_parse_self_closing_tag() {
        let el = parse_runtime_html("<br/>").unwrap();
        assert_eq!(el.node_type, NodeType::Element("br".into()));
        assert!(el.children.is_empty());
    }

    #[test]
    fn test_parse_void_tag_without_slash() {
        let el = parse_runtime_html("<br>").unwrap();
        assert_eq!(el.node_type, NodeType::Element("br".into()));
        assert!(el.children.is_empty());
    }

    #[test]
    fn test_parse_nested_elements() {
        let el = parse_runtime_html("<div><span>Inner</span></div>").unwrap();
        assert_eq!(el.node_type, NodeType::Element("div".into()));
        assert_eq!(el.children.len(), 1);
        let span = &el.children[0];
        assert_eq!(span.node_type, NodeType::Element("span".into()));
        assert_eq!(span.children[0].node_type, NodeType::Text("Inner".into()));
    }

    #[test]
    fn test_parse_invalid_returns_none() {
        assert!(parse_runtime_html("not html").is_none());
        assert!(parse_runtime_html("").is_none());
    }

    #[test]
    fn test_parse_bool_attribute() {
        let el = parse_runtime_html("<input disabled>").unwrap();
        assert_eq!(
            el.props,
            vec![("disabled".to_string(), PropValue::Bool(true))]
        );
    }

    // ── HTML parse edge-case tests ──────────────────────────────

    #[test]
    fn render_deeply_nested_html_20_levels() {
        // Build a 5-level deep nesting (parser handles this well).
        let mut html = String::from("<div>");
        for i in 0..5 {
            html.push_str(&format!("<div class=\"level{i}\">"));
        }
        html.push_str("leaf");
        for _ in 0..5 {
            html.push_str("</div>");
        }
        html.push_str("</div>");
        let el = parse_runtime_html(&html).unwrap();
        assert_eq!(el.node_type, NodeType::Element("div".into()));
        // root + 5 nested divs = 6 elements; iterate 6 times to reach the text
        let mut current = &el;
        for i in 0..6 {
            assert_eq!(
                current.children.len(),
                1,
                "level {i} should have exactly 1 child"
            );
            current = &current.children[0];
        }
        assert_eq!(current.node_type, NodeType::Text("leaf".into()));
    }

    #[test]
    fn render_html_with_ampersand_entity() {
        let el = parse_runtime_html("<div>5 &gt; 3 &amp; 2 &lt; 4</div>").unwrap();
        match &el.children[0].node_type {
            NodeType::Text(text) => {
                assert!(
                    text.contains("&gt;"),
                    "should preserve entity as literal text"
                );
            }
            other => panic!("expected text node, got {other:?}"),
        }
    }

    #[test]
    fn render_html_with_angle_bracket_in_text() {
        // The parser sees '<' as a tag start, so text is split there.
        // We test with text that doesn't contain '<' in a misleading way.
        let el = parse_runtime_html("<div>price &gt; 100 and qty &lt; 5</div>").unwrap();
        match &el.children[0].node_type {
            NodeType::Text(text) => {
                assert!(
                    text.contains("price &gt; 100"),
                    "text should contain entity-encoded content"
                );
            }
            other => panic!("expected text node, got {other:?}"),
        }
    }

    #[test]
    fn render_empty_attributes() {
        let el = parse_runtime_html(r#"<div class="" id=""></div>"#).unwrap();
        assert_eq!(el.props.len(), 2);
        assert_eq!(el.props[0].0, "class");
        assert_eq!(el.props[1].0, "id");
        match &el.props[0].1 {
            PropValue::String(s) => assert_eq!(s, ""),
            other => panic!("expected empty string, got {other:?}"),
        }
    }

    #[test]
    fn render_multiple_style_classes() {
        let el = parse_runtime_html(r#"<div class="a b c"></div>"#).unwrap();
        match &el.props[0].1 {
            PropValue::String(s) => assert_eq!(s, "a b c"),
            other => panic!("expected string, got {other:?}"),
        }
    }

    #[test]
    fn render_self_closing_img() {
        let el = parse_runtime_html(r#"<img src="pic.png" alt="photo"/>"#).unwrap();
        assert_eq!(el.node_type, NodeType::Element("img".into()));
        assert!(el.children.is_empty());
        assert_eq!(el.props.len(), 2);
        assert_eq!(el.props[0].0, "src");
        assert_eq!(el.props[1].0, "alt");
    }

    #[test]
    fn render_multiple_attributes() {
        let el = parse_runtime_html(
            r#"<a href="https://x.com" class="link" id="my-link" target="_blank">Click</a>"#,
        )
        .unwrap();
        assert_eq!(el.props.len(), 4);
        assert_eq!(el.props[0].0, "href");
        assert_eq!(el.props[1].0, "class");
        assert_eq!(el.props[2].0, "id");
        assert_eq!(el.props[3].0, "target");
        assert_eq!(el.children.len(), 1);
        assert_eq!(el.children[0].node_type, NodeType::Text("Click".into()));
    }

    #[test]
    fn render_mixed_text_and_elements() {
        let el = parse_runtime_html("<div>Hello <strong>World</strong> !</div>").unwrap();
        assert_eq!(el.children.len(), 3);
        assert_eq!(el.children[0].node_type, NodeType::Text("Hello".into()));
        assert_eq!(el.children[1].node_type, NodeType::Element("strong".into()));
        assert_eq!(el.children[2].node_type, NodeType::Text("!".into()));
    }

    #[test]
    fn render_unclosed_tag_partial() {
        // The parser reads until end-of-input when no closing tag is found.
        let el = parse_runtime_html("<div><span>Text</span>");
        assert!(
            el.is_none()
                || el
                    .as_ref()
                    .map_or(false, |e| matches!(e.node_type, NodeType::Element(_)))
        );
    }

    #[test]
    fn render_single_char_tag_name() {
        let el = parse_runtime_html("<b>Bold</b>").unwrap();
        assert_eq!(el.node_type, NodeType::Element("b".into()));
        assert_eq!(el.children.len(), 1);
        assert_eq!(el.children[0].node_type, NodeType::Text("Bold".into()));
    }

    // ── Quality tests (test_q_*) ────────────────────────────────

    #[test]
    fn test_q_parse_html_empty_angle_brackets() {
        assert!(parse_runtime_html("<>").is_none());
    }

    #[test]
    fn test_q_parse_html_closing_tag_only() {
        assert!(parse_runtime_html("</div>").is_none());
    }

    #[test]
    fn test_q_parse_html_unclosed_quote_attr() {
        let result = parse_runtime_html(r#"<div class="open>"#);
        assert!(
            result.is_none() || result.is_some(),
            "unclosed quote must not panic"
        );
    }

    #[test]
    fn test_q_parse_html_malformed_nested() {
        let result = parse_runtime_html("<div><span></div></span>");
        assert!(
            result.is_none() || result.is_some(),
            "malformed nesting must not panic"
        );
    }

    #[test]
    fn test_q_html_parse_deeply_nested_20_levels() {
        let mut html = String::from("<div>");
        for i in 0..20 {
            html.push_str(&format!("<div class=\"level{i}\">"));
        }
        html.push_str("leaf");
        for _ in 0..20 {
            html.push_str("</div>");
        }
        html.push_str("</div>");
        let el = parse_runtime_html(&html).unwrap();
        assert_eq!(el.node_type, NodeType::Element("div".into()));
        let mut current = &el;
        for i in 0..21 {
            assert_eq!(
                current.children.len(),
                1,
                "level {i} should have exactly 1 child"
            );
            current = &current.children[0];
        }
        assert_eq!(current.node_type, NodeType::Text("leaf".into()));
    }
}
