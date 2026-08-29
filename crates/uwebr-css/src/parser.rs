use crate::ast::*;
use anyhow::{bail, Result};

pub fn parse_css(input: &str) -> Result<Vec<CssRule>> {
    let mut rules = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }
        if ch == '/' {
            skip_comment(&mut chars);
            continue;
        }
        if ch == '@' {
            // @media rule
            let at_rule = parse_at_rule(&mut chars)?;
            if let Some(rule) = at_rule {
                rules.push(rule);
            }
            continue;
        }
        if ch == '}' || ch == '\0' {
            break;
        }
        let rule = parse_rule(&mut chars)?;
        rules.push(rule);
    }

    Ok(rules)
}

fn skip_comment(chars: &mut std::iter::Peekable<std::str::Chars>) {
    chars.next(); // consume '/'
    if chars.peek() == Some(&'*') {
        chars.next(); // consume '*'
        while let Some(&ch) = chars.peek() {
            chars.next();
            if ch == '*' && chars.peek() == Some(&'/') {
                chars.next();
                return;
            }
        }
    }
}

fn skip_whitespace(chars: &mut std::iter::Peekable<std::str::Chars>) {
    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() || ch == '\n' || ch == '\r' || ch == '\t' {
            chars.next();
        } else if ch == '/' {
            skip_comment(chars);
        } else {
            break;
        }
    }
}

fn read_ident(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut s = String::new();
    while let Some(&ch) = chars.peek() {
        if ch.is_alphanumeric() || ch == '-' || ch == '_' {
            s.push(ch);
            chars.next();
        } else {
            break;
        }
    }
    s
}

fn read_until(chars: &mut std::iter::Peekable<std::str::Chars>, end: char) -> String {
    let mut s = String::new();
    let mut depth = 0i32;
    while let Some(&ch) = chars.peek() {
        if ch == end && depth == 0 {
            return s;
        }
        if ch == '{' {
            depth += 1;
        } else if ch == '}' {
            depth -= 1;
        }
        s.push(ch);
        chars.next();
    }
    s
}

fn skip_block(chars: &mut std::iter::Peekable<std::str::Chars>) {
    let mut depth = 0i32;
    while let Some(&ch) = chars.peek() {
        if ch == '{' {
            depth += 1;
        } else if ch == '}' {
            if depth == 0 {
                chars.next();
                return;
            }
            depth -= 1;
        }
        chars.next();
    }
}

fn parse_at_rule(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<Option<CssRule>> {
    chars.next(); // consume '@'
    let name = read_ident(chars);
    skip_whitespace(chars);

    match name.as_str() {
        "media" => {
            let query = read_until(chars, '{').trim().to_string();
            chars.next(); // consume '{'
            skip_whitespace(chars);
            let content = read_until(chars, '}');
            chars.next(); // consume '}'

            let inner_rules = parse_css(&content)?;
            if let Some(mut rule) = inner_rules.into_iter().next() {
                rule.media_query = Some(query);
                Ok(Some(rule))
            } else {
                Ok(None)
            }
        }
        "import" => {
            // Skip @import
            read_until(chars, ';');
            chars.next();
            Ok(None)
        }
        _ => {
            skip_block(chars);
            Ok(None)
        }
    }
}

fn parse_rule(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<CssRule> {
    let mut all_selectors = Vec::new();

    loop {
        let selector = parse_selector(chars)?;
        all_selectors.push(selector);
        skip_whitespace(chars);

        if chars.peek() == Some(&',') {
            chars.next(); // consume ','
            continue;
        }
        break;
    }

    let selector = if all_selectors.len() == 1 {
        all_selectors.pop().unwrap()
    } else {
        CssSelector::List(all_selectors)
    };

    skip_whitespace(chars);
    if chars.peek() != Some(&'{') {
        bail!("Expected '{{' after selector");
    }
    chars.next(); // consume '{'
    skip_whitespace(chars);

    let properties = parse_declarations(chars)?;

    if chars.peek() == Some(&'}') {
        chars.next();
    }

    Ok(CssRule {
        selector,
        properties,
        media_query: None,
    })
}

fn parse_selector(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<CssSelector> {
    let mut result: Option<CssSelector> = None;

    loop {
        skip_whitespace(chars);
        if let Some(&ch) = chars.peek() {
            if ch == '{' || ch == ',' || ch == '\0' {
                break;
            }
        }

        // Check for combinators between selectors
        if result.is_some() {
            skip_whitespace(chars);
            if chars.peek() == Some(&'>') {
                chars.next(); // consume '>'
                skip_whitespace(chars);
                let child = parse_simple_selector(chars)?;
                let prev = result.take().unwrap();
                result = Some(CssSelector::Child(vec![prev, child]));
                continue;
            }
        }

        let sel = parse_simple_selector(chars)?;
        match result {
            None => result = Some(sel),
            Some(existing) => {
                // Implicit descendant combinator (space between selectors)
                result = Some(CssSelector::Descendant(vec![existing, sel]));
            }
        }
    }

    match result {
        Some(sel) => Ok(sel),
        None => bail!("Empty selector"),
    }
}

fn parse_simple_selector(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<CssSelector> {
    skip_whitespace(chars);
    let sel = match chars.peek() {
        Some(&'.') => {
            chars.next();
            let name = read_ident(chars);
            CssSelector::Class(name)
        }
        Some(&'#') => {
            chars.next();
            let name = read_ident(chars);
            CssSelector::Id(name)
        }
        Some(&'*') => {
            chars.next();
            CssSelector::Universal
        }
        Some(&ch) if ch.is_alphabetic() || ch == '_' || ch == '-' => {
            let name = read_ident(chars);
            CssSelector::Tag(name)
        }
        _ => bail!("Unexpected character in selector: {:?}", chars.peek()),
    };

    // Handle pseudo-classes after any selector: .btn:hover, #id:focus, div:first-child
    skip_whitespace(chars);
    if chars.peek() == Some(&':') {
        chars.next(); // consume ':'
        let _pseudo = read_ident(chars); // e.g. "hover", "focus", "first-child"
    }
    // Handle attribute selectors after any selector: input[type="text"]
    skip_whitespace(chars);
    if chars.peek() == Some(&'[') {
        let _attrs = read_until(chars, ']');
        if chars.peek() == Some(&']') {
            chars.next(); // consume ']'
        }
    }

    Ok(sel)
}

fn parse_declarations(
    chars: &mut std::iter::Peekable<std::str::Chars>,
) -> Result<Vec<CssProperty>> {
    let mut props = Vec::new();

    loop {
        skip_whitespace(chars);
        if let Some(&'}') = chars.peek() {
            break;
        }
        if chars.peek().is_none() {
            break;
        }

        let prop = parse_declaration(chars)?;
        props.push(prop);

        skip_whitespace(chars);
        if chars.peek() == Some(&';') {
            chars.next();
        }
    }

    Ok(props)
}

fn parse_declaration(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<CssProperty> {
    let name = read_ident(chars);
    skip_whitespace(chars);

    if chars.peek() != Some(&':') {
        bail!("Expected ':' after property name '{}'", name);
    }
    chars.next(); // consume ':'
    skip_whitespace(chars);

    let mut value_str = String::new();
    let mut depth = 0i32;
    while let Some(&ch) = chars.peek() {
        if (ch == ';' || ch == '}') && depth == 0 {
            break;
        }
        if ch == '(' {
            depth += 1;
        } else if ch == ')' {
            depth -= 1;
        }
        value_str.push(ch);
        chars.next();
    }
    let value_str = value_str.trim().to_string();

    let important = value_str.ends_with("!important");
    let value_str = if important {
        value_str.trim_end_matches("!important").trim().to_string()
    } else {
        value_str
    };

    let value = parse_value(&name, &value_str)?;

    Ok(CssProperty {
        name,
        value,
        important,
    })
}

fn parse_value(prop_name: &str, raw: &str) -> Result<CssValue> {
    let raw = raw.trim();

    // Handle shorthand properties like "10px 20px"
    if raw.contains(' ') && (prop_name == "padding" || prop_name == "margin") {
        let parts: Vec<CssValue> = raw
            .split_whitespace()
            .filter_map(|s| parse_single_value(s).ok())
            .collect();
        if parts.len() > 1 {
            return Ok(CssValue::Shorthand(parts));
        }
    }

    parse_single_value(raw)
}

fn parse_single_value(raw: &str) -> Result<CssValue> {
    let raw = raw.trim();

    // Auto
    if raw == "auto" {
        return Ok(CssValue::Auto);
    }
    // Inherit / initial / unset
    if raw == "inherit" || raw == "initial" || raw == "unset" {
        return Ok(CssValue::Inherited);
    }

    // None
    if raw == "none" {
        return Ok(CssValue::Keyword("none".to_string()));
    }

    // Color hex
    if raw.starts_with('#') {
        return Ok(CssValue::Color(Color::from_hex(raw)));
    }

    // Named colors
    if let Some(c) = named_color(raw) {
        return Ok(CssValue::Color(c));
    }

    // rgb(...)
    if raw.starts_with("rgb") {
        return parse_rgb(raw);
    }

    // hsl(...)
    if raw.starts_with("hsl") {
        return parse_hsl(raw);
    }

    // Length with unit
    if let Some(value) = parse_length(raw) {
        return Ok(value);
    }

    // Gradients — must precede the keyword fallback, which would otherwise
    // swallow them (or reject them for containing parens/commas).
    if raw.starts_with("linear-gradient(") {
        if let Some(v) = parse_linear_gradient(raw) {
            return Ok(v);
        }
    }
    if raw.starts_with("radial-gradient(") {
        if let Some(v) = parse_radial_gradient(raw) {
            return Ok(v);
        }
    }

    // Keyword
    if raw
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Ok(CssValue::Keyword(raw.to_string()));
    }

    // Fallback: store as keyword (for unknown values like box-shadow, transitions, etc.)
    Ok(CssValue::Keyword(raw.to_string()))
}

fn parse_length(raw: &str) -> Option<CssValue> {
    let raw = raw.trim();

    if raw.ends_with("px") {
        let num: f32 = raw.trim_end_matches("px").parse().ok()?;
        return Some(CssValue::Length(num, LengthUnit::Px));
    }
    // "rem" must be tested before "em", otherwise "2rem" matches the "em"
    // branch, leaves "2r", and fails to parse.
    if raw.ends_with("rem") {
        let num: f32 = raw.trim_end_matches("rem").parse().ok()?;
        return Some(CssValue::Length(num, LengthUnit::Rem));
    }
    if raw.ends_with("em") {
        let num: f32 = raw.trim_end_matches("em").parse().ok()?;
        return Some(CssValue::Length(num, LengthUnit::Em));
    }
    if raw.ends_with('%') {
        let num: f32 = raw.trim_end_matches('%').parse().ok()?;
        return Some(CssValue::Length(num, LengthUnit::Percent));
    }
    if raw.ends_with("vw") {
        let num: f32 = raw.trim_end_matches("vw").parse().ok()?;
        return Some(CssValue::Length(num, LengthUnit::Vw));
    }
    if raw.ends_with("vh") {
        let num: f32 = raw.trim_end_matches("vh").parse().ok()?;
        return Some(CssValue::Length(num, LengthUnit::Vh));
    }

    // Plain number → treat as px
    if let Ok(num) = raw.parse::<f32>() {
        return Some(CssValue::Length(num, LengthUnit::Px));
    }

    None
}

/// Split gradient arguments on top-level commas, respecting nested parens so
/// `rgb(0, 0, 255)` stays intact.
fn split_gradient_args(inner: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    for ch in inner.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 => {
                parts.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }
    parts
}

/// Does this token name a direction rather than a colour stop?
fn is_gradient_direction(token: &str) -> bool {
    token.starts_with("to ") || token.ends_with("deg")
}

/// Parse a single stop like `red`, `#ff0000 50%`, or `rgb(0,0,255) 100%`.
fn parse_gradient_stop(token: &str) -> Option<GradientStop> {
    let token = token.trim();

    // A trailing percentage is the position; the rest is the colour.
    let (color_part, position) = if let Some(idx) = token.rfind('%') {
        // Find the whitespace separating colour from position.
        let before_pct = &token[..idx];
        if let Some(space) = before_pct.rfind(char::is_whitespace) {
            let pos: Option<f32> = before_pct[space + 1..].trim().parse().ok();
            (token[..space].trim(), pos.map(|p| p / 100.0))
        } else {
            // The whole token is a percentage with no colour — invalid.
            (token, None)
        }
    } else {
        (token, None)
    };

    let color = parse_color_token(color_part)?;
    Some(GradientStop { color, position })
}

/// Parse a colour token used inside a gradient (hex, named, rgb(), hsl()).
fn parse_color_token(token: &str) -> Option<Color> {
    let token = token.trim();
    if token.starts_with('#') {
        return Some(Color::from_hex(token));
    }
    if let Some(c) = named_color(token) {
        return Some(c);
    }
    if token.starts_with("rgb") {
        if let Ok(CssValue::Color(c)) = parse_rgb(token) {
            return Some(c);
        }
    }
    if token.starts_with("hsl") {
        if let Ok(CssValue::Color(c)) = parse_hsl(token) {
            return Some(c);
        }
    }
    None
}

fn parse_linear_gradient(raw: &str) -> Option<CssValue> {
    let inner = raw
        .trim()
        .strip_prefix("linear-gradient(")?
        .strip_suffix(')')?;
    let args = split_gradient_args(inner);
    if args.is_empty() {
        return None;
    }

    let mut direction = None;
    let mut stop_tokens = &args[..];
    if is_gradient_direction(&args[0]) {
        direction = Some(args[0].clone());
        stop_tokens = &args[1..];
    }

    let stops: Vec<GradientStop> = stop_tokens
        .iter()
        .filter_map(|t| parse_gradient_stop(t))
        .collect();

    // A gradient needs at least two stops to be meaningful.
    if stops.len() < 2 {
        return None;
    }

    Some(CssValue::LinearGradient { direction, stops })
}

fn parse_radial_gradient(raw: &str) -> Option<CssValue> {
    let inner = raw
        .trim()
        .strip_prefix("radial-gradient(")?
        .strip_suffix(')')?;
    let args = split_gradient_args(inner);

    // Drop a leading shape/size/position token if it is not a colour stop
    // (e.g. "circle", "circle at center"). Keep it simple: skip the first arg
    // when it doesn't parse as a stop.
    let stops: Vec<GradientStop> = args.iter().filter_map(|t| parse_gradient_stop(t)).collect();

    if stops.len() < 2 {
        return None;
    }

    Some(CssValue::RadialGradient { stops })
}

fn parse_rgb(raw: &str) -> Result<CssValue> {
    let inner = raw
        .trim_start_matches("rgb")
        .trim_start_matches('(')
        .trim_end_matches(')');
    let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
    match parts.len() {
        3 => {
            let r: u8 = parts[0].parse()?;
            let g: u8 = parts[1].parse()?;
            let b: u8 = parts[2].parse()?;
            Ok(CssValue::Color(Color::rgb(r, g, b)))
        }
        4 => {
            let r: u8 = parts[0].parse()?;
            let g: u8 = parts[1].parse()?;
            let b: u8 = parts[2].parse()?;
            let a: f32 = parts[3].parse()?;
            Ok(CssValue::Color(Color::rgba(r, g, b, a)))
        }
        _ => bail!("Invalid rgb()"),
    }
}

fn parse_hsl(raw: &str) -> Result<CssValue> {
    let inner = raw
        .trim_start_matches("hsl")
        .trim_start_matches('(')
        .trim_end_matches(')');
    let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
    if parts.len() >= 3 {
        let h: f32 = parts[0].trim_end_matches("deg").trim().parse()?;
        let s: f32 = parts[1].trim_end_matches('%').trim().parse()?;
        let l: f32 = parts[2].trim_end_matches('%').trim().parse()?;
        let (r, g, b) = hsl_to_rgb(h, s / 100.0, l / 100.0);
        Ok(CssValue::Color(Color::rgb(r, g, b)))
    } else {
        bail!("Invalid hsl()")
    }
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match h as u32 {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

fn named_color(name: &str) -> Option<Color> {
    match name {
        "black" => Some(Color::rgb(0, 0, 0)),
        "white" => Some(Color::rgb(255, 255, 255)),
        "red" => Some(Color::rgb(255, 0, 0)),
        "green" => Some(Color::rgb(0, 128, 0)),
        "blue" => Some(Color::rgb(0, 0, 255)),
        "yellow" => Some(Color::rgb(255, 255, 0)),
        "cyan" => Some(Color::rgb(0, 255, 255)),
        "magenta" => Some(Color::rgb(255, 0, 255)),
        "gray" | "grey" => Some(Color::rgb(128, 128, 128)),
        "orange" => Some(Color::rgb(255, 165, 0)),
        "purple" => Some(Color::rgb(128, 0, 128)),
        "transparent" => Some(Color::rgba(0, 0, 0, 0.0)),
        _ => None,
    }
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
        assert_eq!(
            rules[0].properties[0].value,
            CssValue::Length(16.0, LengthUnit::Px)
        );
    }

    #[test]
    fn test_parse_multiple_properties() {
        let css = ".container { display: flex; padding: 16px; gap: 8px; }";
        let rules = parse_css(css).unwrap();
        assert_eq!(rules[0].properties.len(), 3);
    }

    #[test]
    fn test_parse_class_selector() {
        let css = ".my-class { color: red; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Class(name) => assert_eq!(name, "my-class"),
            _ => panic!("Expected class selector"),
        }
    }

    #[test]
    fn test_parse_id_selector() {
        let css = "#main { width: 100%; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Id(name) => assert_eq!(name, "main"),
            _ => panic!("Expected id selector"),
        }
    }

    #[test]
    fn test_parse_tag_selector() {
        let css = "div { margin: 0; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Tag(name) => assert_eq!(name, "div"),
            _ => panic!("Expected tag selector"),
        }
    }

    #[test]
    fn test_parse_universal_selector() {
        let css = "* { box-sizing: border-box; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Universal => {}
            _ => panic!("Expected universal selector"),
        }
    }

    #[test]
    fn test_parse_padding_values() {
        let css = ".box { padding: 10px 20px; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::Shorthand(parts) => assert_eq!(parts.len(), 2),
            _ => panic!("Expected shorthand"),
        }
    }

    #[test]
    fn test_parse_margin_auto() {
        let css = ".box { margin: auto; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::Auto => {}
            _ => panic!("Expected auto value"),
        }
    }

    #[test]
    fn test_parse_position() {
        let css = ".box { position: absolute; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::Keyword(k) => assert_eq!(k, "absolute"),
            _ => panic!("Expected keyword value"),
        }
    }

    #[test]
    fn test_parse_width_height() {
        let css = ".box { width: 100px; height: 50vh; }";
        let rules = parse_css(css).unwrap();
        assert_eq!(rules[0].properties.len(), 2);
        assert_eq!(
            rules[0].properties[0].value,
            CssValue::Length(100.0, LengthUnit::Px)
        );
        assert_eq!(
            rules[0].properties[1].value,
            CssValue::Length(50.0, LengthUnit::Vh)
        );
    }

    #[test]
    fn test_parse_gap() {
        let css = ".grid { gap: 16px; }";
        let rules = parse_css(css).unwrap();
        assert_eq!(rules[0].properties[0].name, "gap");
    }

    #[test]
    fn test_parse_color_hex() {
        let css = ".box { color: #ff0000; }";
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

    #[test]
    fn test_parse_color_named() {
        let css = ".box { color: red; background-color: blue; }";
        let rules = parse_css(css).unwrap();
        assert_eq!(rules[0].properties.len(), 2);
    }

    #[test]
    fn test_parse_important() {
        let css = ".box { color: red !important; }";
        let rules = parse_css(css).unwrap();
        assert!(rules[0].properties[0].important);
    }

    #[test]
    fn test_parse_multiple_rules() {
        let css = ".a { color: red; } .b { color: blue; }";
        let rules = parse_css(css).unwrap();
        assert_eq!(rules.len(), 2);
    }

    #[test]
    fn test_parse_comment() {
        let css = "/* comment */ .a { color: red; }";
        let rules = parse_css(css).unwrap();
        assert_eq!(rules.len(), 1);
    }

    #[test]
    fn test_parse_child_selector() {
        let css = "div > span { color: red; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Child(parts) => assert_eq!(parts.len(), 2),
            _ => panic!("Expected child selector"),
        }
    }

    #[test]
    fn test_parse_list_selector() {
        let css = ".a, .b { color: red; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::List(parts) => assert_eq!(parts.len(), 2),
            _ => panic!("Expected list selector"),
        }
    }

    #[test]
    fn test_parse_media_query() {
        let css = "@media (max-width: 768px) { .a { color: red; } }";
        let rules = parse_css(css).unwrap();
        assert_eq!(rules.len(), 1);
        assert!(rules[0].media_query.is_some());
    }

    // --- Real-world CSS patterns ---

    #[test]
    fn test_tailwind_like_utilities() {
        let css = ".flex { display: flex; } .p-4 { padding: 16px; } .m-auto { margin: auto; }";
        let rules = parse_css(css).unwrap();
        assert_eq!(rules.len(), 3);
    }

    #[test]
    fn test_nested_selectors() {
        let css = ".card .title { font-size: 24px; } .nav > a { color: blue; }";
        let rules = parse_css(css).unwrap();
        assert_eq!(rules.len(), 2);
    }

    #[test]
    fn test_complex_properties() {
        let css = r#"
            .box {
                background-color: #ff0000;
                border-radius: 8px;
                box-shadow: 0 4px 6px rgba(0,0,0,0.1);
                transition: all 0.3s ease;
            }
        "#;
        let rules = parse_css(css).unwrap();
        assert_eq!(rules.len(), 1);
        // Only known properties are converted to Taffy
    }

    #[test]
    fn test_font_properties() {
        let css = r#"
            .text {
                font-size: 16px;
                font-weight: bold;
                line-height: 1.5;
                letter-spacing: 0.5px;
                text-align: center;
                text-decoration: underline;
                color: #333;
            }
        "#;
        let rules = parse_css(css).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].properties.len(), 7);
    }

    #[test]
    fn test_pseudo_class_fallback() {
        // :hover, :focus, :active etc. — stored as keyword, Taffy ignores
        let css = ".btn:hover { background: blue; }";
        let rules = parse_css(css).unwrap();
        assert_eq!(rules.len(), 1);
    }

    #[test]
    fn test_attribute_selector_fallback() {
        // [type="text"] — stored as keyword, Taffy ignores
        let css = r#"input[type="text"] { border: 1px solid; }"#;
        let rules = parse_css(css).unwrap();
        assert_eq!(rules.len(), 1);
    }

    #[test]
    fn test_keyframes_not_supported() {
        let css = r#"
            @keyframes slide {
                from { transform: translateX(0); }
                to { transform: translateX(100px); }
            }
        "#;
        let result = parse_css(css);
        // @keyframes inner rules fail because they use from/to, not selectors
        assert!(result.is_err() || result.unwrap().is_empty());
    }

    #[test]
    fn test_calc_fallback_to_keyword() {
        let css = ".box { width: calc(100% - 20px); }";
        let rules = parse_css(css).unwrap();
        assert_eq!(rules.len(), 1);
        // calc() stored as keyword — Taffy ignores it
    }

    #[test]
    fn test_gradient_fallback_to_keyword() {
        // A malformed gradient (single stop) still falls back to a keyword so it
        // is ignored rather than crashing the parse.
        let css = ".bg { background: linear-gradient(red); }";
        let rules = parse_css(css).unwrap();
        assert_eq!(rules.len(), 1);
        assert!(matches!(rules[0].properties[0].value, CssValue::Keyword(_)));
    }

    #[test]
    fn test_parse_linear_gradient_two_colors() {
        let css = ".bg { background: linear-gradient(red, blue); }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::LinearGradient { direction, stops } => {
                assert!(direction.is_none());
                assert_eq!(stops.len(), 2);
                assert_eq!(
                    (stops[0].color.r, stops[0].color.g, stops[0].color.b),
                    (255, 0, 0)
                );
                assert_eq!(
                    (stops[1].color.r, stops[1].color.g, stops[1].color.b),
                    (0, 0, 255)
                );
            }
            other => panic!("expected linear gradient, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_linear_gradient_with_direction_and_positions() {
        let css = ".bg { background: linear-gradient(to right, red 0%, blue 100%); }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::LinearGradient { direction, stops } => {
                assert_eq!(direction.as_deref(), Some("to right"));
                assert_eq!(stops.len(), 2);
                assert_eq!(stops[0].position, Some(0.0));
                assert_eq!(stops[1].position, Some(1.0));
            }
            other => panic!("expected linear gradient, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_linear_gradient_deg_and_rgb() {
        let css = ".bg { background: linear-gradient(45deg, #ff0000, rgb(0, 0, 255)); }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::LinearGradient { direction, stops } => {
                assert_eq!(direction.as_deref(), Some("45deg"));
                assert_eq!(stops.len(), 2);
                assert_eq!(
                    (stops[1].color.r, stops[1].color.g, stops[1].color.b),
                    (0, 0, 255)
                );
            }
            other => panic!("expected linear gradient, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_radial_gradient() {
        let css = ".bg { background: radial-gradient(red, blue); }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::RadialGradient { stops } => {
                assert_eq!(stops.len(), 2);
            }
            other => panic!("expected radial gradient, got {other:?}"),
        }
    }

    #[test]
    fn test_shorthand_all_sides() {
        let css = ".a { padding: 10px; } .b { padding: 10px 20px; } .c { padding: 10px 20px 30px; } .d { padding: 10px 20px 30px 40px; }";
        let rules = parse_css(css).unwrap();
        assert_eq!(rules.len(), 4);
    }

    #[test]
    fn test_percent_values() {
        let css = ".w { width: 50%; } .h { height: 100vh; }";
        let rules = parse_css(css).unwrap();
        assert_eq!(rules.len(), 2);
    }

    #[test]
    fn test_parse_rem_not_swallowed_by_em() {
        // `ends_with("em")` matches "2rem" too; rem must be checked first.
        let css = "h1 { font-size: 2rem; }";
        let rules = parse_css(css).unwrap();
        assert_eq!(
            rules[0].properties[0].value,
            CssValue::Length(2.0, LengthUnit::Rem)
        );
    }

    #[test]
    fn test_parse_em_still_works() {
        let css = "p { font-size: 1.5em; }";
        let rules = parse_css(css).unwrap();
        assert_eq!(
            rules[0].properties[0].value,
            CssValue::Length(1.5, LengthUnit::Em)
        );
    }
}
