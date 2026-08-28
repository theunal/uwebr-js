use crate::ast::*;
use anyhow::Result;

/// Parse CSS string into CssRule list
pub fn parse_css(css: &str) -> Result<Vec<CssRule>> {
    let mut parser = CssParser::new(css);
    parser.parse_rules()
}

struct CssParser {
    input: Vec<char>,
    pos: usize,
}

impl CssParser {
    fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
        }
    }

    fn parse_rules(&mut self) -> Result<Vec<CssRule>> {
        let mut rules = Vec::new();
        self.skip_whitespace();
        while self.pos < self.input.len() {
            // Skip @media for now, just extract inner rules
            if self.peek() == Some('@') {
                self.skip_at_rule();
            } else {
                rules.push(self.parse_rule()?);
            }
            self.skip_whitespace();
        }
        Ok(rules)
    }

    fn parse_rule(&mut self) -> Result<CssRule> {
        let selector = self.parse_selector()?;
        self.skip_whitespace();
        self.expect('{')?;
        let properties = self.parse_properties()?;
        self.skip_whitespace();
        self.expect('}')?;

        Ok(CssRule {
            selector,
            properties,
            media_query: None,
        })
    }

    fn parse_selector(&mut self) -> Result<CssSelector> {
        self.skip_whitespace();
        let mut selectors = Vec::new();

        while self.pos < self.input.len() && self.peek() != Some('{') {
            selectors.push(self.parse_single_selector()?);
            self.skip_whitespace();

            if self.peek() == Some(',') {
                self.advance();
                self.skip_whitespace();
            } else if self.peek() == Some(' ') {
                // Descendant combinator
                if selectors.len() > 1 {
                    let last = selectors.pop().unwrap();
                    let first = selectors.pop().unwrap();
                    selectors.push(CssSelector::Descendant(vec![first, last]));
                }
            }
        }

        if selectors.len() == 1 {
            Ok(selectors.into_iter().next().unwrap())
        } else {
            Ok(CssSelector::List(selectors))
        }
    }

    fn parse_single_selector(&mut self) -> Result<CssSelector> {
        match self.peek() {
            Some('.') => {
                self.advance();
                let name = self.read_ident()?;
                Ok(CssSelector::Class(name))
            }
            Some('#') => {
                self.advance();
                let name = self.read_ident()?;
                Ok(CssSelector::Id(name))
            }
            Some('*') => {
                self.advance();
                Ok(CssSelector::Universal)
            }
            Some(_) if self.peek().unwrap().is_alphabetic() => {
                let name = self.read_ident()?;
                Ok(CssSelector::Tag(name))
            }
            _ => Ok(CssSelector::Universal),
        }
    }

    fn parse_properties(&mut self) -> Result<Vec<CssProperty>> {
        let mut props = Vec::new();
        self.skip_whitespace();
        while self.peek() != Some('}') {
            props.push(self.parse_property()?);
            self.skip_whitespace();
        }
        Ok(props)
    }

    fn parse_property(&mut self) -> Result<CssProperty> {
        let name = self.read_ident()?;
        self.skip_whitespace();
        self.expect(':')?;
        self.skip_whitespace();
        let value = self.read_value()?;
        self.skip_whitespace();
        let important = if self.peek() == Some('!') {
            self.advance();
            let bang = self.read_ident()?;
            bang == "important"
        } else {
            false
        };
        self.skip_whitespace();
        if self.peek() == Some(';') {
            self.advance();
        }
        Ok(CssProperty {
            name,
            value,
            important,
        })
    }

    fn read_value(&mut self) -> Result<CssValue> {
        let mut tokens = Vec::new();
        let mut current = String::new();

        while let Some(c) = self.peek() {
            match c {
                ';' | '}' => break,
                ' ' | '\t' | '\n' | '\r' => {
                    if !current.is_empty() {
                        tokens.push(current.clone());
                        current.clear();
                    }
                    self.advance();
                }
                '(' => {
                    current.push(c);
                    self.advance();
                    // Read until closing paren
                    let mut depth = 1;
                    while depth > 0 && self.peek().is_some() {
                        let c = self.peek().unwrap();
                        if c == '(' {
                            depth += 1;
                        } else if c == ')' {
                            depth -= 1;
                        }
                        current.push(c);
                        self.advance();
                    }
                    tokens.push(current.clone());
                    current.clear();
                }
                _ => {
                    current.push(c);
                    self.advance();
                }
            }
        }
        if !current.is_empty() {
            tokens.push(current);
        }

        if tokens.is_empty() {
            return Ok(CssValue::Inherited);
        }

        // Parse color
        if let Some(first) = tokens.first() {
            if first.starts_with('#') || first.starts_with("rgb") || first.starts_with("hsl") {
                let color = parse_color(first)?;
                return Ok(CssValue::Color(color));
            }
        }

        // Parse length
        if tokens.len() == 1 {
            if let Ok(val) = parse_length_value(&tokens[0]) {
                return Ok(val);
            }
        }

        // Keyword
        Ok(CssValue::Keyword(tokens.join(" ")))
    }

    fn read_ident(&mut self) -> Result<String> {
        let mut name = String::new();
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                name.push(c);
                self.advance();
            } else {
                break;
            }
        }
        Ok(name)
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

    fn skip_at_rule(&mut self) {
        while let Some(c) = self.peek() {
            if c == '{' {
                // Skip block
                let mut depth = 1;
                self.advance();
                while depth > 0 && self.peek().is_some() {
                    let c = self.peek().unwrap();
                    if c == '{' {
                        depth += 1;
                    } else if c == '}' {
                        depth -= 1;
                    }
                    self.advance();
                }
                return;
            } else if c == ';' {
                self.advance();
                return;
            }
            self.advance();
        }
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos).copied()
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
            None => Err(anyhow::anyhow!(
                "Expected '{}', found end of input",
                expected
            )),
        }
    }
}

fn parse_color(s: &str) -> Result<Color> {
    if s.starts_with('#') {
        Ok(Color::from_hex(s))
    } else if s.starts_with("rgb(") {
        let inner = s.trim_start_matches("rgb(").trim_end_matches(')');
        let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
        if parts.len() >= 3 {
            let r = parts[0].parse().unwrap_or(0);
            let g = parts[1].parse().unwrap_or(0);
            let b = parts[2].parse().unwrap_or(0);
            Ok(Color::rgb(r, g, b))
        } else {
            Ok(Color::rgb(0, 0, 0))
        }
    } else {
        Ok(Color::rgb(0, 0, 0))
    }
}

fn parse_length_value(s: &str) -> Result<CssValue> {
    if s == "auto" || s == "none" {
        return Ok(CssValue::Auto);
    }
    if s == "inherit" || s == "initial" || s == "unset" {
        return Ok(CssValue::Inherited);
    }

    if let Some(px) = s.strip_suffix("px") {
        if let Ok(val) = px.parse() {
            return Ok(CssValue::Length(val, LengthUnit::Px));
        }
    }
    if let Some(em) = s.strip_suffix("em") {
        if let Ok(val) = em.parse() {
            return Ok(CssValue::Length(val, LengthUnit::Em));
        }
    }
    if let Some(rem) = s.strip_suffix("rem") {
        if let Ok(val) = rem.parse() {
            return Ok(CssValue::Length(val, LengthUnit::Rem));
        }
    }
    if let Some(percent) = s.strip_suffix('%') {
        if let Ok(val) = percent.parse() {
            return Ok(CssValue::Length(val, LengthUnit::Percent));
        }
    }
    if let Some(vw) = s.strip_suffix("vw") {
        if let Ok(val) = vw.parse() {
            return Ok(CssValue::Length(val, LengthUnit::Vw));
        }
    }
    if let Some(vh) = s.strip_suffix("vh") {
        if let Ok(val) = vh.parse() {
            return Ok(CssValue::Length(val, LengthUnit::Vh));
        }
    }

    // Plain number → px
    if let Ok(val) = s.parse() {
        return Ok(CssValue::Length(val, LengthUnit::Px));
    }

    Ok(CssValue::Keyword(s.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_rule() {
        let css = ".card { padding: 16px; }";
        let rules = parse_css(css).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].properties[0].name, "padding");
    }

    #[test]
    fn test_parse_multiple_properties() {
        let css = ".container { display: flex; padding: 16px; gap: 8px; }";
        let rules = parse_css(css).unwrap();
        assert_eq!(rules[0].properties.len(), 3);
    }

    #[test]
    fn test_parse_color() {
        let css = ".red { color: #ff0000; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::Color(c) => {
                assert_eq!(c.r, 255);
                assert_eq!(c.g, 0);
                assert_eq!(c.b, 0);
            }
            _ => panic!("Expected color"),
        }
    }
}
