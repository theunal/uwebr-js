use crate::ast::*;
use anyhow::Result;

/// Parse HTML string into HtmlNode tree
pub fn parse_html(html: &str) -> Result<HtmlNode> {
    let mut parser = HtmlParser::new(html);
    let nodes = parser.parse_fragment()?;
    if nodes.len() == 1 {
        Ok(nodes.into_iter().next().unwrap())
    } else {
        Ok(HtmlNode::Fragment(nodes))
    }
}

struct HtmlParser {
    input: Vec<char>,
    pos: usize,
}

impl HtmlParser {
    fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
        }
    }

    fn parse_fragment(&mut self) -> Result<Vec<HtmlNode>> {
        let mut nodes = Vec::new();
        self.skip_whitespace();
        while self.pos < self.input.len() {
            if self.peek() == Some('<') && self.peek_at(1) == Some('!') {
                self.skip_comment();
            } else if self.peek() == Some('<') {
                nodes.push(self.parse_element()?);
            } else {
                nodes.push(self.parse_text()?);
            }
            self.skip_whitespace();
        }
        Ok(nodes)
    }

    fn parse_element(&mut self) -> Result<HtmlNode> {
        self.expect('<')?;
        let tag = self.parse_tag_name()?;
        let attributes = self.parse_attributes()?;

        // Self-closing tags
        let self_closing = if self.peek() == Some('/') && self.peek_at(1) == Some('>') {
            self.advance();
            self.advance();
            true
        } else {
            self.expect('>')?;
            false
        };

        let children = if self_closing || is_self_closing_tag(&tag) {
            vec![]
        } else {
            let children = self.parse_children(&tag)?;
            self.expect('<')?;
            self.expect('/')?;
            let closing_tag = self.parse_tag_name()?;
            if closing_tag != tag {
                return Err(anyhow::anyhow!(
                    "Mismatched tags: expected </{}>, found </{}>",
                    tag,
                    closing_tag
                ));
            }
            self.expect('>')?;
            children
        };

        Ok(HtmlNode::Element(HtmlElement {
            tag,
            attributes,
            children,
            self_closing,
        }))
    }

    fn parse_tag_name(&mut self) -> Result<String> {
        let mut name = String::new();
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ':' {
                name.push(c);
                self.advance();
            } else {
                break;
            }
        }
        if name.is_empty() {
            return Err(anyhow::anyhow!("Expected tag name at position {}", self.pos));
        }
        Ok(name)
    }

    fn parse_attributes(&mut self) -> Result<Vec<HtmlAttribute>> {
        let mut attrs = Vec::new();
        self.skip_whitespace();
        while self.peek() != Some('>') && self.peek() != Some('/') {
            let name = self.parse_attr_name()?;
            self.skip_whitespace();
            if self.peek() == Some('=') {
                self.advance();
                self.skip_whitespace();
                let value = self.parse_attr_value()?;
                attrs.push(HtmlAttribute { name, value });
            } else {
                // Boolean attribute
                attrs.push(HtmlAttribute {
                    name,
                    value: HtmlAttributeValue::Boolean(true),
                });
            }
            self.skip_whitespace();
        }
        Ok(attrs)
    }

    fn parse_attr_name(&mut self) -> Result<String> {
        let mut name = String::new();
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ':' || c == '@' {
                name.push(c);
                self.advance();
            } else {
                break;
            }
        }
        Ok(name)
    }

    fn parse_attr_value(&mut self) -> Result<HtmlAttributeValue> {
        match self.peek() {
            Some('"') | Some('\'') => {
                let quote = self.peek().unwrap();
                self.advance();
                let mut value = String::new();
                while self.peek() != Some(quote) {
                    if let Some(c) = self.peek() {
                        value.push(c);
                        self.advance();
                    } else {
                        break;
                    }
                }
                self.advance(); // closing quote
                Ok(HtmlAttributeValue::Literal(value))
            }
            Some('{') => {
                self.advance(); // skip {
                let expr = self.read_until('}')?;
                self.advance(); // skip }
                Ok(HtmlAttributeValue::Expression(expr.trim().to_string()))
            }
            _ => {
                let mut value = String::new();
                while let Some(c) = self.peek() {
                    if c.is_whitespace() || c == '>' || c == '/' {
                        break;
                    }
                    value.push(c);
                    self.advance();
                }
                Ok(HtmlAttributeValue::Literal(value))
            }
        }
    }

    fn parse_children(&mut self, _parent_tag: &str) -> Result<Vec<HtmlNode>> {
        let mut children = Vec::new();
        while self.pos < self.input.len() {
            if self.peek() == Some('<') && self.peek_at(1) == Some('/') {
                // Closing tag for parent
                break;
            } else if self.peek() == Some('{') && self.peek_at(1) == Some('#') {
                children.push(self.parse_template_directive()?);
            } else if self.peek() == Some('{') {
                children.push(self.parse_text_expression()?);
            } else if self.peek() == Some('<') {
                children.push(self.parse_element()?);
            } else {
                children.push(self.parse_text()?);
            }
        }
        Ok(children)
    }

    fn parse_text(&mut self) -> Result<HtmlNode> {
        let mut text = String::new();
        while let Some(c) = self.peek() {
            if c == '<' || c == '{' {
                break;
            }
            text.push(c);
            self.advance();
        }
        Ok(HtmlNode::Text(text.trim().to_string()))
    }

    fn parse_text_expression(&mut self) -> Result<HtmlNode> {
        self.expect('{')?;
        let expr = self.read_until('}')?;
        self.expect('}')?;
        Ok(HtmlNode::Expression(expr.trim().to_string()))
    }

    fn parse_template_directive(&mut self) -> Result<HtmlNode> {
        self.expect('{')?;
        self.expect('#')?;
        let directive = self.parse_tag_name()?;
        self.skip_whitespace();

        match directive.as_str() {
            "each" => self.parse_each_loop(),
            "if" => self.parse_if_block(),
            _ => Err(anyhow::anyhow!("Unknown directive: {}", directive)),
        }
    }

    fn parse_each_loop(&mut self) -> Result<HtmlNode> {
        let iterable = self.parse_tag_name()?;
        self.skip_whitespace();
        let item_name = if self.peek_keyword("as") {
            self.advance_n(2);
            self.skip_whitespace();
            self.parse_tag_name()?
        } else {
            "item".to_string()
        };

        self.skip_whitespace();
        self.expect('}')?;

        let body = self.parse_children("each")?;

        self.expect('{')?;
        self.expect('/')?;
        let closing = self.parse_tag_name()?;
        if closing != "each" {
            return Err(anyhow::anyhow!("Expected {{/each}}, found {{/{}}}", closing));
        }
        self.expect('}')?;
        self.expect('}')?;

        Ok(HtmlNode::EachLoop(HtmlEach {
            iterable,
            item_name,
            index_name: None,
            body,
        }))
    }

    fn parse_if_block(&mut self) -> Result<HtmlNode> {
        let condition = self.read_until('}')?;
        self.expect('}')?;

        let then_body = self.parse_children("if")?;
        let mut else_body = None;

        // Check for {:else}
        if self.peek() == Some('{') && self.peek_at(1) == Some(':') {
            self.advance_n(2);
            let keyword = self.parse_tag_name()?;
            if keyword == "else" {
                self.expect('}')?;
                else_body = Some(self.parse_children("if")?);
            }
        }

        self.expect('{')?;
        self.expect('/')?;
        let closing = self.parse_tag_name()?;
        if closing != "if" {
            return Err(anyhow::anyhow!("Expected {{/if}}, found {{/{}}}", closing));
        }
        self.expect('}')?;
        self.expect('}')?;

        Ok(HtmlNode::IfBlock(HtmlIf {
            condition: condition.trim().to_string(),
            then_body,
            else_body,
        }))
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn skip_comment(&mut self) {
        while self.pos < self.input.len() - 2 {
            if self.peek() == Some('-')
                && self.peek_at(1) == Some('-')
                && self.peek_at(2) == Some('>')
            {
                self.advance_n(3);
                return;
            }
            self.advance();
        }
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.input.get(self.pos + offset).copied()
    }

    fn peek_keyword(&self, keyword: &str) -> bool {
        let remaining: String = self.input[self.pos..].iter().take(keyword.len()).collect();
        remaining == keyword
            && self
                .input
                .get(self.pos + keyword.len())
                .map(|c| !c.is_alphanumeric())
                .unwrap_or(true)
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    fn advance_n(&mut self, n: usize) {
        self.pos += n;
    }

    fn expect(&mut self, expected: char) -> Result<()> {
        match self.peek() {
            Some(c) if c == expected => {
                self.advance();
                Ok(())
            }
            Some(c) => Err(anyhow::anyhow!(
                "Expected '{}', found '{}' at position {}",
                expected,
                c,
                self.pos
            )),
            None => Err(anyhow::anyhow!("Expected '{}', found end of input", expected)),
        }
    }

    fn read_until(&mut self, delimiter: char) -> Result<String> {
        let mut value = String::new();
        while let Some(c) = self.peek() {
            if c == delimiter {
                return Ok(value);
            }
            value.push(c);
            self.advance();
        }
        Err(anyhow::anyhow!(
            "Expected '{}' at position {}",
            delimiter,
            self.pos
        ))
    }
}

fn is_self_closing_tag(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
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
                assert_eq!(el.attributes.len(), 1);
                assert_eq!(el.attributes[0].name, "class");
                assert_eq!(
                    el.attributes[0].value,
                    HtmlAttributeValue::Literal("container".to_string())
                );
                assert_eq!(el.children.len(), 1);
                assert_eq!(el.children[0], HtmlNode::Text("Hello".to_string()));
            }
            _ => panic!("Expected element"),
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
            }
            _ => panic!("Expected element"),
        }
    }

    #[test]
    fn test_parse_self_closing() {
        let html = r#"<img src="test.png" />"#;
        let node = parse_html(html).unwrap();
        match node {
            HtmlNode::Element(el) => {
                assert_eq!(el.tag, "img");
                assert!(el.self_closing);
            }
            _ => panic!("Expected element"),
        }
    }
}
