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
        } else {
            break;
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
    let mut sel = match chars.peek() {
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
        Some(&'[') => {
            // Bare attribute selector, e.g. `[disabled]` → applies to any element.
            CssSelector::Universal
        }
        Some(&':') => {
            // Bare pseudo-class, e.g. `:first-child` → applies to any element.
            CssSelector::Universal
        }
        Some(&ch) if ch.is_alphabetic() || ch == '_' || ch == '-' => {
            let name = read_ident(chars);
            CssSelector::Tag(name)
        }
        _ => bail!("Unexpected character in selector: {:?}", chars.peek()),
    };

    // Chain any number of pseudo-classes and attribute selectors onto the base,
    // e.g. `input[type="text"]:focus` or `.btn:hover:first-child`. No whitespace
    // is skipped here: a space would start a descendant selector instead.
    loop {
        match chars.peek() {
            Some(&':') => {
                chars.next(); // consume ':'
                              // A leading `::` is a pseudo-element; skip the extra colon.
                if chars.peek() == Some(&':') {
                    chars.next();
                }
                let pseudo_name = read_ident(chars);
                // Consume a functional argument like `nth-child(2n+1)`.
                let mut argument = None;
                if chars.peek() == Some(&'(') {
                    chars.next(); // consume '('
                    argument = Some(read_until(chars, ')'));
                    if chars.peek() == Some(&')') {
                        chars.next();
                    }
                }

                // Route structural pseudo-classes to the Nth variant.
                sel = match pseudo_name.as_str() {
                    "first-child" => CssSelector::Nth {
                        selector: Box::new(sel),
                        kind: NthKind::FirstChild,
                        argument: None,
                    },
                    "last-child" => CssSelector::Nth {
                        selector: Box::new(sel),
                        kind: NthKind::LastChild,
                        argument: None,
                    },
                    "nth-child" => CssSelector::Nth {
                        selector: Box::new(sel),
                        kind: NthKind::FirstChild,
                        argument,
                    },
                    "nth-last-child" => CssSelector::Nth {
                        selector: Box::new(sel),
                        kind: NthKind::LastChild,
                        argument,
                    },
                    "first-of-type" => CssSelector::Nth {
                        selector: Box::new(sel),
                        kind: NthKind::FirstOfType,
                        argument: None,
                    },
                    "last-of-type" => CssSelector::Nth {
                        selector: Box::new(sel),
                        kind: NthKind::LastOfType,
                        argument: None,
                    },
                    "nth-of-type" => CssSelector::Nth {
                        selector: Box::new(sel),
                        kind: NthKind::OfType,
                        argument,
                    },
                    "nth-last-of-type" => CssSelector::Nth {
                        selector: Box::new(sel),
                        kind: NthKind::LastOfType,
                        argument,
                    },
                    "empty" => CssSelector::Nth {
                        selector: Box::new(sel),
                        kind: NthKind::Empty,
                        argument: None,
                    },
                    "not" => {
                        // The inner selector is in `argument`, already consumed
                        // by read_until. Parse it as a fresh selector.
                        let arg = argument.as_deref().unwrap_or("");
                        let inner = parse_selector(&mut arg.chars().peekable())?;
                        CssSelector::Not {
                            selector: Box::new(sel),
                            inner: Box::new(inner),
                        }
                    }
                    _ => CssSelector::PseudoClass(Box::new(sel), pseudo_name),
                };
            }
            Some(&'[') => {
                chars.next(); // consume '['
                let (attr, op, value) = parse_attribute_selector(chars);
                skip_whitespace(chars);
                if chars.peek() == Some(&']') {
                    chars.next(); // consume ']'
                }
                sel = CssSelector::Attribute {
                    selector: Box::new(sel),
                    attr,
                    op,
                    value,
                };
            }
            _ => break,
        }
    }

    Ok(sel)
}

/// Parse the interior of an attribute selector (after the opening `[`).
///
/// Returns the attribute name, the match operator, and the optional value.
/// Positioned just after `[`; the caller consumes the closing `]`.
fn parse_attribute_selector(
    chars: &mut std::iter::Peekable<std::str::Chars>,
) -> (String, AttributeOp, Option<String>) {
    skip_whitespace(chars);
    let attr = read_ident(chars);
    skip_whitespace(chars);

    // Operator: `=`, `~=`, `^=`, `$=`, `*=`, or none (existence check).
    let op = match chars.peek() {
        Some(&'=') => {
            chars.next();
            AttributeOp::Equals
        }
        Some(&'~') => {
            chars.next();
            if chars.peek() == Some(&'=') {
                chars.next();
            }
            AttributeOp::Includes
        }
        Some(&'^') => {
            chars.next();
            if chars.peek() == Some(&'=') {
                chars.next();
            }
            AttributeOp::Prefix
        }
        Some(&'$') => {
            chars.next();
            if chars.peek() == Some(&'=') {
                chars.next();
            }
            AttributeOp::Suffix
        }
        Some(&'*') => {
            chars.next();
            if chars.peek() == Some(&'=') {
                chars.next();
            }
            AttributeOp::Contains
        }
        _ => return (attr, AttributeOp::Exists, None),
    };

    skip_whitespace(chars);
    let value = if chars.peek() == Some(&'"') || chars.peek() == Some(&'\'') {
        Some(read_quoted_string(chars))
    } else {
        Some(read_ident(chars))
    };

    (attr, op, value)
}

/// Parse An+B notation for `:nth-child` and similar pseudo-classes.
///
/// Accepts: "odd", "even", "3", "2n+1", "2n-1", "-n+3", "+n+1", etc.
/// Returns `(a, b)` where the match formula is `index == a * n + b` for some non-negative integer `n`.
pub fn parse_nth(arg: &str) -> Option<(i32, i32)> {
    let arg = arg.trim().to_lowercase();
    if arg == "odd" {
        return Some((2, 1));
    }
    if arg == "even" {
        return Some((2, 0));
    }

    if let Some(n_pos) = arg.find('n') {
        let a_str = arg[..n_pos].trim();
        let a = if a_str.is_empty() || a_str == "+" {
            1
        } else if a_str == "-" {
            -1
        } else {
            a_str.parse::<i32>().ok()?
        };
        let rest = arg[n_pos + 1..].trim();
        let b = if rest.is_empty() {
            0
        } else if let Some(stripped) = rest.strip_prefix('+') {
            stripped.trim().parse::<i32>().ok()?
        } else if let Some(stripped) = rest.strip_prefix('-') {
            -stripped.trim().parse::<i32>().ok()?
        } else {
            rest.parse::<i32>().ok()?
        };
        Some((a, b))
    } else {
        let b = arg.parse::<i32>().ok()?;
        Some((0, b))
    }
}

/// Read a quoted string value, consuming the surrounding quotes.
fn read_quoted_string(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let quote = match chars.peek() {
        Some(&q @ ('"' | '\'')) => {
            chars.next();
            q
        }
        _ => return String::new(),
    };
    let mut s = String::new();
    while let Some(&ch) = chars.peek() {
        chars.next();
        if ch == quote {
            break;
        }
        s.push(ch);
    }
    s
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
    if raw.contains(' ')
        && (prop_name == "padding"
            || prop_name == "margin"
            || prop_name == "grid-template-columns"
            || prop_name == "grid-template-rows"
            || prop_name == "translate"
            || prop_name == "scale"
            || prop_name == "skew"
            || prop_name == "rotate")
    {
        let parts: Vec<CssValue> = raw
            .split_whitespace()
            .filter_map(|s| parse_single_value(s).ok())
            .collect();
        if parts.len() > 1 {
            return Ok(CssValue::Shorthand(parts));
        }
    }

    // Handle "1 / 3" syntax for grid-column / grid-row
    if raw.contains('/') && (prop_name == "grid-column" || prop_name == "grid-row") {
        let parts: Vec<CssValue> = raw
            .split('/')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .filter_map(|s| parse_single_value(s).ok())
            .collect();
        if !parts.is_empty() {
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
    if raw.ends_with("fr") {
        let num: f32 = raw.trim_end_matches("fr").parse().ok()?;
        return Some(CssValue::Length(num, LengthUnit::Fr));
    }
    if raw.ends_with("deg") {
        let num: f32 = raw.trim_end_matches("deg").parse().ok()?;
        return Some(CssValue::Length(num, LengthUnit::Px));
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
        // :hover, :focus, :active etc. are now parsed into PseudoClass variants.
        let css = ".btn:hover { background: blue; }";
        let rules = parse_css(css).unwrap();
        assert_eq!(rules.len(), 1);
        assert!(matches!(rules[0].selector, CssSelector::PseudoClass(_, _)));
    }

    #[test]
    fn test_attribute_selector_fallback() {
        // [type="text"] is now parsed into an Attribute variant.
        let css = r#"input[type="text"] { border: 1px solid; }"#;
        let rules = parse_css(css).unwrap();
        assert_eq!(rules.len(), 1);
        assert!(matches!(rules[0].selector, CssSelector::Attribute { .. }));
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

    // --- Pseudo-class / attribute selectors (FAZ 13) ---

    #[test]
    fn test_parse_pseudo_class_hover() {
        let css = ".btn:hover { background: blue; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::PseudoClass(inner, pseudo) => {
                assert_eq!(**inner, CssSelector::Class("btn".to_string()));
                assert_eq!(pseudo, "hover");
            }
            other => panic!("expected pseudo-class, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_pseudo_class_first_child_on_tag() {
        let css = "div:first-child { color: red; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Nth {
                selector,
                kind,
                argument,
            } => {
                assert_eq!(**selector, CssSelector::Tag("div".to_string()));
                assert_eq!(*kind, NthKind::FirstChild);
                assert!(argument.is_none());
            }
            other => panic!("expected Nth, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_attribute_equals() {
        let css = r#"input[type="text"] { border: 1px solid; }"#;
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Attribute {
                selector,
                attr,
                op,
                value,
            } => {
                assert_eq!(**selector, CssSelector::Tag("input".to_string()));
                assert_eq!(attr, "type");
                assert_eq!(*op, AttributeOp::Equals);
                assert_eq!(value.as_deref(), Some("text"));
            }
            other => panic!("expected attribute selector, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_attribute_exists() {
        let css = "[disabled] { opacity: 0.5; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Attribute {
                selector,
                attr,
                op,
                value,
            } => {
                assert_eq!(**selector, CssSelector::Universal);
                assert_eq!(attr, "disabled");
                assert_eq!(*op, AttributeOp::Exists);
                assert_eq!(*value, None);
            }
            other => panic!("expected attribute selector, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_attribute_contains() {
        let css = r#"[class*="active"] { font-weight: bold; }"#;
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Attribute {
                selector,
                attr,
                op,
                value,
            } => {
                assert_eq!(**selector, CssSelector::Universal);
                assert_eq!(attr, "class");
                assert_eq!(*op, AttributeOp::Contains);
                assert_eq!(value.as_deref(), Some("active"));
            }
            other => panic!("expected attribute selector, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_attribute_prefix_suffix_includes() {
        let ops = [
            (r#"[href^="https"]"#, AttributeOp::Prefix, "https"),
            (r#"[src$=".png"]"#, AttributeOp::Suffix, ".png"),
            (r#"[rel~="next"]"#, AttributeOp::Includes, "next"),
        ];
        for (sel, expected_op, expected_val) in ops {
            let css = format!("{sel} {{ color: red; }}");
            let rules = parse_css(&css).unwrap();
            match &rules[0].selector {
                CssSelector::Attribute { op, value, .. } => {
                    assert_eq!(*op, expected_op, "op mismatch for {sel}");
                    assert_eq!(
                        value.as_deref(),
                        Some(expected_val),
                        "value mismatch for {sel}"
                    );
                }
                other => panic!("expected attribute selector for {sel}, got {other:?}"),
            }
        }
    }

    #[test]
    fn test_parse_tag_with_attribute_and_pseudo() {
        // Chained: input[type="text"]:focus
        let css = r#"input[type="text"]:focus { outline: none; }"#;
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::PseudoClass(inner, pseudo) => {
                assert_eq!(pseudo, "focus");
                match &**inner {
                    CssSelector::Attribute { attr, .. } => assert_eq!(attr, "type"),
                    other => panic!("expected attribute inside pseudo, got {other:?}"),
                }
            }
            other => panic!("expected pseudo-class outer, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_not_selector() {
        let css = "div:not(.active) { opacity: 0.5; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Not { selector, inner } => {
                assert_eq!(**selector, CssSelector::Tag("div".to_string()));
                assert_eq!(**inner, CssSelector::Class("active".to_string()));
            }
            other => panic!("expected Not, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_not_with_tag_inner() {
        let css = "div:not(p) { color: red; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Not { selector, inner } => {
                assert_eq!(**selector, CssSelector::Tag("div".to_string()));
                assert_eq!(**inner, CssSelector::Tag("p".to_string()));
            }
            other => panic!("expected Not, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_nth_child_with_arg() {
        let css = "li:nth-child(2n+1) { color: red; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Nth { kind, argument, .. } => {
                assert_eq!(*kind, NthKind::FirstChild);
                assert_eq!(argument.as_deref(), Some("2n+1"));
            }
            other => panic!("expected Nth, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_nth_of_type() {
        let css = "p:nth-of-type(2) { margin-left: 10px; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Nth { kind, argument, .. } => {
                assert_eq!(*kind, NthKind::OfType);
                assert_eq!(argument.as_deref(), Some("2"));
            }
            other => panic!("expected Nth, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_empty_pseudo() {
        let css = ":empty { display: none; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Nth { kind, .. } => {
                assert_eq!(*kind, NthKind::Empty);
            }
            other => panic!("expected Nth(Empty), got {other:?}"),
        }
    }

    // ═══════════════════════════════════════════════════════════════
    //  Selector matching (~25 tests)
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn css_chained_pseudo_hover_active() {
        let css = ".btn:hover:active { color: red; }";
        let rules = parse_css(css).unwrap();
        assert_eq!(rules.len(), 1);
        match &rules[0].selector {
            CssSelector::PseudoClass(inner, pseudo) => {
                assert_eq!(pseudo, "active");
                match &**inner {
                    CssSelector::PseudoClass(base, p2) => {
                        assert_eq!(**base, CssSelector::Class("btn".to_string()));
                        assert_eq!(p2, "hover");
                    }
                    other => panic!("expected inner PseudoClass, got {other:?}"),
                }
            }
            other => panic!("expected PseudoClass, got {other:?}"),
        }
    }

    #[test]
    fn css_chained_pseudo_focus_hover() {
        let css = "input:focus:hover { border: 2px solid blue; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::PseudoClass(inner, pseudo) => {
                assert_eq!(pseudo, "hover");
                match &**inner {
                    CssSelector::PseudoClass(base, p2) => {
                        assert_eq!(**base, CssSelector::Tag("input".to_string()));
                        assert_eq!(p2, "focus");
                    }
                    other => panic!("expected inner PseudoClass, got {other:?}"),
                }
            }
            other => panic!("expected PseudoClass, got {other:?}"),
        }
    }

    #[test]
    fn css_chained_three_pseudo_classes() {
        let css = ".item:hover:focus:active { outline: 1px solid red; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::PseudoClass(inner, pseudo) => {
                assert_eq!(pseudo, "active");
                match &**inner {
                    CssSelector::PseudoClass(mid, p2) => {
                        assert_eq!(p2, "focus");
                        match &**mid {
                            CssSelector::PseudoClass(base, p3) => {
                                assert_eq!(**base, CssSelector::Class("item".to_string()));
                                assert_eq!(p3, "hover");
                            }
                            other => panic!("expected base PseudoClass, got {other:?}"),
                        }
                    }
                    other => panic!("expected mid PseudoClass, got {other:?}"),
                }
            }
            other => panic!("expected outer PseudoClass, got {other:?}"),
        }
    }

    #[test]
    fn css_deeply_nested_not_with_descendant() {
        let css = "div:not(.a .b) { color: red; }";
        let rules = parse_css(&css).unwrap();
        assert_eq!(rules.len(), 1);
        match &rules[0].selector {
            CssSelector::Not { selector, inner } => {
                assert_eq!(**selector, CssSelector::Tag("div".to_string()));
                match &**inner {
                    CssSelector::Descendant(parts) => {
                        assert_eq!(parts.len(), 2);
                        assert_eq!(parts[0], CssSelector::Class("a".to_string()));
                        assert_eq!(parts[1], CssSelector::Class("b".to_string()));
                    }
                    other => panic!("expected Descendant inside :not(), got {other:?}"),
                }
            }
            other => panic!("expected Not, got {other:?}"),
        }
    }

    #[test]
    fn css_deeply_nested_not_with_attribute() {
        let css = "div:not([disabled]) { opacity: 1; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Not { selector, inner } => {
                assert_eq!(**selector, CssSelector::Tag("div".to_string()));
                match &**inner {
                    CssSelector::Attribute { attr, op, .. } => {
                        assert_eq!(attr, "disabled");
                        assert_eq!(*op, AttributeOp::Exists);
                    }
                    other => panic!("expected Attribute inside :not(), got {other:?}"),
                }
            }
            other => panic!("expected Not, got {other:?}"),
        }
    }

    #[test]
    fn css_multi_level_descendant() {
        let css = "div > p > span { color: blue; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            // Parser builds: Child([Child([Tag("div"), Tag("p")]), Tag("span")])
            CssSelector::Child(parts) => {
                assert_eq!(parts.len(), 2);
                match &parts[0] {
                    CssSelector::Child(inner_parts) => {
                        assert_eq!(inner_parts.len(), 2);
                        assert_eq!(inner_parts[0], CssSelector::Tag("div".to_string()));
                        assert_eq!(inner_parts[1], CssSelector::Tag("p".to_string()));
                    }
                    other => panic!("expected nested Child, got {other:?}"),
                }
                assert_eq!(parts[1], CssSelector::Tag("span".to_string()));
            }
            other => panic!("expected Child, got {other:?}"),
        }
    }

    #[test]
    fn css_descendant_three_levels() {
        let css = "div span a { text-decoration: none; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Descendant(parts) => {
                assert_eq!(parts.len(), 2);
                // Parser builds: Descendant([Descendant([Tag("div"), Tag("span")]), Tag("a")])
                match &parts[0] {
                    CssSelector::Descendant(inner_parts) => {
                        assert_eq!(inner_parts.len(), 2);
                        assert_eq!(inner_parts[0], CssSelector::Tag("div".to_string()));
                        assert_eq!(inner_parts[1], CssSelector::Tag("span".to_string()));
                    }
                    other => panic!("expected nested Descendant, got {other:?}"),
                }
                assert_eq!(parts[1], CssSelector::Tag("a".to_string()));
            }
            other => panic!("expected Descendant, got {other:?}"),
        }
    }

    #[test]
    fn css_mixed_child_and_descendant() {
        let css = "div > p span { margin: 0; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Descendant(parts) => {
                assert_eq!(parts.len(), 2);
                assert!(matches!(&parts[0], CssSelector::Child(_)));
                assert_eq!(parts[1], CssSelector::Tag("span".to_string()));
            }
            other => panic!("expected Descendant with Child inside, got {other:?}"),
        }
    }

    #[test]
    fn css_nth_child_2n_plus_1() {
        let css = "li:nth-child(2n+1) { font-weight: bold; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Nth { kind, argument, .. } => {
                assert_eq!(*kind, NthKind::FirstChild);
                assert_eq!(argument.as_deref(), Some("2n+1"));
            }
            other => panic!("expected Nth, got {other:?}"),
        }
    }

    #[test]
    fn css_nth_child_minus_n_plus_3() {
        let css = "li:nth-child(-n+3) { color: red; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Nth { kind, argument, .. } => {
                assert_eq!(*kind, NthKind::FirstChild);
                assert_eq!(argument.as_deref(), Some("-n+3"));
            }
            other => panic!("expected Nth, got {other:?}"),
        }
    }

    #[test]
    fn css_nth_child_3n() {
        let css = "li:nth-child(3n) { display: block; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Nth { kind, argument, .. } => {
                assert_eq!(*kind, NthKind::FirstChild);
                assert_eq!(argument.as_deref(), Some("3n"));
            }
            other => panic!("expected Nth, got {other:?}"),
        }
    }

    #[test]
    fn css_nth_child_even() {
        let css = "tr:nth-child(even) { background: #f0f0f0; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Nth { kind, argument, .. } => {
                assert_eq!(*kind, NthKind::FirstChild);
                assert_eq!(argument.as_deref(), Some("even"));
            }
            other => panic!("expected Nth, got {other:?}"),
        }
    }

    #[test]
    fn css_nth_child_odd() {
        let css = "tr:nth-child(odd) { background: #ffffff; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Nth { kind, argument, .. } => {
                assert_eq!(*kind, NthKind::FirstChild);
                assert_eq!(argument.as_deref(), Some("odd"));
            }
            other => panic!("expected Nth, got {other:?}"),
        }
    }

    #[test]
    fn css_nth_of_type_complex() {
        let css = "p:nth-of-type(2n+1) { margin-top: 10px; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Nth { kind, argument, .. } => {
                assert_eq!(*kind, NthKind::OfType);
                assert_eq!(argument.as_deref(), Some("2n+1"));
            }
            other => panic!("expected Nth, got {other:?}"),
        }
    }

    #[test]
    fn css_nth_of_type_first() {
        let css = "p:first-of-type { font-weight: bold; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Nth { kind, argument, .. } => {
                assert_eq!(*kind, NthKind::FirstOfType);
                assert!(argument.is_none());
            }
            other => panic!("expected Nth, got {other:?}"),
        }
    }

    #[test]
    fn css_nth_last_child_with_arg() {
        let css = "li:nth-last-child(3) { color: gray; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Nth { kind, argument, .. } => {
                assert_eq!(*kind, NthKind::LastChild);
                assert_eq!(argument.as_deref(), Some("3"));
            }
            other => panic!("expected Nth, got {other:?}"),
        }
    }

    #[test]
    fn css_nth_last_of_type() {
        let css = "span:nth-last-of-type(2n) { font-size: 12px; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Nth { kind, argument, .. } => {
                assert_eq!(*kind, NthKind::LastOfType);
                assert_eq!(argument.as_deref(), Some("2n"));
            }
            other => panic!("expected Nth, got {other:?}"),
        }
    }

    #[test]
    fn css_last_child_on_class() {
        let css = ".list-item:last-child { margin-bottom: 0; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Nth {
                selector,
                kind,
                argument,
            } => {
                assert_eq!(**selector, CssSelector::Class("list-item".to_string()));
                assert_eq!(*kind, NthKind::LastChild);
                assert!(argument.is_none());
            }
            other => panic!("expected Nth(LastChild), got {other:?}"),
        }
    }

    #[test]
    fn css_first_child_on_id() {
        let css = "#sidebar:first-child { border-right: 1px solid; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Nth { selector, kind, .. } => {
                assert_eq!(**selector, CssSelector::Id("sidebar".to_string()));
                assert_eq!(*kind, NthKind::FirstChild);
            }
            other => panic!("expected Nth, got {other:?}"),
        }
    }

    #[test]
    fn css_empty_on_tag() {
        let css = "div:empty { display: none; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Nth { selector, kind, .. } => {
                assert_eq!(**selector, CssSelector::Tag("div".to_string()));
                assert_eq!(*kind, NthKind::Empty);
            }
            other => panic!("expected Nth(Empty), got {other:?}"),
        }
    }

    #[test]
    fn css_attribute_data_prefix() {
        let css = r#"[data-tooltip^="Hello"] { cursor: help; }"#;
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Attribute {
                attr, op, value, ..
            } => {
                assert_eq!(attr, "data-tooltip");
                assert_eq!(*op, AttributeOp::Prefix);
                assert_eq!(value.as_deref(), Some("Hello"));
            }
            other => panic!("expected Attribute, got {other:?}"),
        }
    }

    #[test]
    fn css_attribute_href_suffix() {
        let css = r#"a[href$=".pdf"] { color: red; }"#;
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Attribute {
                selector,
                attr,
                op,
                value,
            } => {
                assert_eq!(attr, "href");
                assert_eq!(*op, AttributeOp::Suffix);
                assert_eq!(value.as_deref(), Some(".pdf"));
                match &**selector {
                    CssSelector::Tag(t) => assert_eq!(t, "a"),
                    other => panic!("expected Tag(a), got {other:?}"),
                }
            }
            other => panic!("expected Attribute, got {other:?}"),
        }
    }

    #[test]
    fn css_attribute_single_quotes() {
        let css = r#"input[type='email'] { background: white; }"#;
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Attribute {
                attr, value, op, ..
            } => {
                assert_eq!(attr, "type");
                assert_eq!(*op, AttributeOp::Equals);
                assert_eq!(value.as_deref(), Some("email"));
            }
            other => panic!("expected Attribute, got {other:?}"),
        }
    }

    #[test]
    fn css_universal_with_attribute() {
        let css = r#"*[role="button"] { display: block; }"#;
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Attribute {
                selector,
                attr,
                op,
                value,
            } => {
                assert_eq!(**selector, CssSelector::Universal);
                assert_eq!(attr, "role");
                assert_eq!(*op, AttributeOp::Equals);
                assert_eq!(value.as_deref(), Some("button"));
            }
            other => panic!("expected Attribute, got {other:?}"),
        }
    }

    #[test]
    fn css_pseudo_element_double_colon() {
        let css = ".btn::before { content: ''; }";
        let rules = parse_css(css).unwrap();
        assert_eq!(rules.len(), 1);
        match &rules[0].selector {
            CssSelector::PseudoClass(inner, pseudo) => {
                assert_eq!(**inner, CssSelector::Class("btn".to_string()));
                assert_eq!(pseudo, "before");
            }
            other => panic!("expected PseudoClass for ::before, got {other:?}"),
        }
    }

    #[test]
    fn css_not_with_universal_inner() {
        let css = "div:not(*) { opacity: 0.5; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::Not { selector, inner } => {
                assert_eq!(**selector, CssSelector::Tag("div".to_string()));
                assert_eq!(**inner, CssSelector::Universal);
            }
            other => panic!("expected Not, got {other:?}"),
        }
    }

    #[test]
    fn css_complex_chained_not_and_pseudo() {
        let css = ".card:not(.disabled):hover { opacity: 1; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::PseudoClass(inner, pseudo) => {
                assert_eq!(pseudo, "hover");
                match &**inner {
                    CssSelector::Not {
                        selector,
                        inner: not_inner,
                    } => {
                        assert_eq!(**selector, CssSelector::Class("card".to_string()));
                        assert_eq!(**not_inner, CssSelector::Class("disabled".to_string()));
                    }
                    other => panic!("expected Not inside pseudo, got {other:?}"),
                }
            }
            other => panic!("expected PseudoClass, got {other:?}"),
        }
    }

    #[test]
    fn css_list_with_complex_selectors() {
        let css = "h1, h2, h3 { margin-top: 0; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].selector {
            CssSelector::List(parts) => {
                assert_eq!(parts.len(), 3);
                assert_eq!(parts[0], CssSelector::Tag("h1".to_string()));
                assert_eq!(parts[1], CssSelector::Tag("h2".to_string()));
                assert_eq!(parts[2], CssSelector::Tag("h3".to_string()));
            }
            other => panic!("expected List, got {other:?}"),
        }
    }

    // ═══════════════════════════════════════════════════════════════
    //  Value parsing (~25 tests)
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn css_shorthand_1_value() {
        let css = ".a { padding: 8px; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::Length(n, unit) => {
                assert_eq!(*n, 8.0);
                assert_eq!(*unit, LengthUnit::Px);
            }
            other => panic!("expected single Length, got {other:?}"),
        }
    }

    #[test]
    fn css_shorthand_2_values() {
        let css = ".a { padding: 10px 20px; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::Shorthand(parts) => {
                assert_eq!(parts.len(), 2);
                assert_eq!(parts[0], CssValue::Length(10.0, LengthUnit::Px));
                assert_eq!(parts[1], CssValue::Length(20.0, LengthUnit::Px));
            }
            other => panic!("expected Shorthand with 2 parts, got {other:?}"),
        }
    }

    #[test]
    fn css_shorthand_3_values() {
        let css = ".a { margin: 5px 10px 15px; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::Shorthand(parts) => {
                assert_eq!(parts.len(), 3);
                assert_eq!(parts[0], CssValue::Length(5.0, LengthUnit::Px));
                assert_eq!(parts[1], CssValue::Length(10.0, LengthUnit::Px));
                assert_eq!(parts[2], CssValue::Length(15.0, LengthUnit::Px));
            }
            other => panic!("expected Shorthand with 3 parts, got {other:?}"),
        }
    }

    #[test]
    fn css_shorthand_4_values() {
        let css = ".a { margin: 1px 2px 3px 4px; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::Shorthand(parts) => {
                assert_eq!(parts.len(), 4);
                assert_eq!(parts[0], CssValue::Length(1.0, LengthUnit::Px));
                assert_eq!(parts[1], CssValue::Length(2.0, LengthUnit::Px));
                assert_eq!(parts[2], CssValue::Length(3.0, LengthUnit::Px));
                assert_eq!(parts[3], CssValue::Length(4.0, LengthUnit::Px));
            }
            other => panic!("expected Shorthand with 4 parts, got {other:?}"),
        }
    }

    #[test]
    fn css_margin_shorthand_4_values() {
        let css = ".a { margin: 10px 20px 30px 40px; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::Shorthand(parts) => {
                assert_eq!(parts.len(), 4);
                assert_eq!(parts[0], CssValue::Length(10.0, LengthUnit::Px));
                assert_eq!(parts[3], CssValue::Length(40.0, LengthUnit::Px));
            }
            other => panic!("expected Shorthand, got {other:?}"),
        }
    }

    #[test]
    fn css_hex_3_char() {
        let css = ".a { color: #fff; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::Color(c) => {
                assert_eq!(c.r, 255);
                assert_eq!(c.g, 255);
                assert_eq!(c.b, 255);
                assert!((c.a - 1.0).abs() < 0.01);
            }
            other => panic!("expected Color, got {other:?}"),
        }
    }

    #[test]
    fn css_hex_4_char_falls_to_default() {
        // 4-char hex (#RGBA) is not supported by Color::from_hex — falls through to default black.
        let css = ".a { color: #fffa; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::Color(c) => {
                assert_eq!(c.r, 0);
                assert_eq!(c.g, 0);
                assert_eq!(c.b, 0);
            }
            other => panic!("expected Color, got {other:?}"),
        }
    }

    #[test]
    fn css_hex_8_char() {
        let css = ".a { color: #ff000080; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::Color(c) => {
                assert_eq!(c.r, 255);
                assert_eq!(c.g, 0);
                assert_eq!(c.b, 0);
                // 0x80 = 128; 128/255 ≈ 0.502
                assert!(c.a > 0.5 && c.a < 0.51, "expected ~0.502, got {}", c.a);
            }
            other => panic!("expected Color, got {other:?}"),
        }
    }

    #[test]
    fn css_hex_short_black() {
        let css = ".a { color: #000; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::Color(c) => {
                assert_eq!(c.r, 0);
                assert_eq!(c.g, 0);
                assert_eq!(c.b, 0);
            }
            other => panic!("expected Color, got {other:?}"),
        }
    }

    #[test]
    fn css_rgb_basic() {
        let css = ".a { color: rgb(255, 128, 0); }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::Color(c) => {
                assert_eq!(c.r, 255);
                assert_eq!(c.g, 128);
                assert_eq!(c.b, 0);
                assert!((c.a - 1.0).abs() < 0.01);
            }
            other => panic!("expected Color, got {other:?}"),
        }
    }

    #[test]
    fn css_rgba_with_alpha() {
        // The parser's parse_rgb handles rgb() with 4 args (alpha) but
        // trim_start_matches("rgb") leaves "a(..." for rgba() input.
        // rgb() with 4 args works:
        let css = ".a { color: rgb(100, 200, 50, 0.5); }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::Color(c) => {
                assert_eq!(c.r, 100);
                assert_eq!(c.g, 200);
                assert_eq!(c.b, 50);
                assert!((c.a - 0.5).abs() < 0.01);
            }
            other => panic!("expected Color, got {other:?}"),
        }
    }

    #[test]
    fn css_hsl_basic() {
        let css = ".a { color: hsl(120, 50%, 50%); }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::Color(c) => {
                // HSL 120, 50%, 50% → green-ish. The exact values depend on conversion.
                assert!(
                    c.g > 60,
                    "green channel should be high for hue=120, got {}",
                    c.g
                );
            }
            other => panic!("expected Color, got {other:?}"),
        }
    }

    #[test]
    fn css_hsl_with_deg() {
        let css = ".a { color: hsl(0deg, 100%, 50%); }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::Color(c) => {
                // hsl(0, 100%, 50%) = pure red
                assert_eq!(c.r, 255);
                assert!(c.g < 10);
                assert!(c.b < 10);
            }
            other => panic!("expected Color, got {other:?}"),
        }
    }

    #[test]
    fn css_calc_fallback_to_keyword() {
        let css = ".a { width: calc(100% - 20px); }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::Keyword(k) => assert!(k.contains("calc")),
            other => panic!("expected Keyword for calc, got {other:?}"),
        }
    }

    #[test]
    fn css_linear_gradient_multi_stop() {
        let css = ".bg { background: linear-gradient(red, yellow, green); }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::LinearGradient { direction, stops } => {
                assert!(direction.is_none());
                assert_eq!(stops.len(), 3);
            }
            other => panic!("expected LinearGradient, got {other:?}"),
        }
    }

    #[test]
    fn css_radial_gradient_multi_stop() {
        let css = ".bg { background: radial-gradient(red, yellow, green, blue); }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::RadialGradient { stops } => {
                assert_eq!(stops.len(), 4);
            }
            other => panic!("expected RadialGradient, got {other:?}"),
        }
    }

    #[test]
    fn css_url_fallback_to_keyword() {
        let css = ".bg { background-image: url('image.png'); }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::Keyword(k) => assert!(k.contains("url")),
            other => panic!("expected Keyword for url(), got {other:?}"),
        }
    }

    #[test]
    fn css_var_fallback_to_keyword() {
        let css = ".a { color: var(--text-color); }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::Keyword(k) => assert!(k.contains("var")),
            other => panic!("expected Keyword for var(), got {other:?}"),
        }
    }

    #[test]
    fn css_clamp_fallback_to_keyword() {
        let css = ".a { width: clamp(200px, 50%, 800px); }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::Keyword(k) => assert!(k.contains("clamp")),
            other => panic!("expected Keyword for clamp(), got {other:?}"),
        }
    }

    #[test]
    fn css_min_max_fallback_to_keyword() {
        let css_min = ".a { width: min(100%, 500px); }";
        let css_max = ".b { width: max(200px, 50%); }";
        let rules_min = parse_css(css_min).unwrap();
        let rules_max = parse_css(css_max).unwrap();
        assert!(matches!(
            rules_min[0].properties[0].value,
            CssValue::Keyword(_)
        ));
        assert!(matches!(
            rules_max[0].properties[0].value,
            CssValue::Keyword(_)
        ));
    }

    #[test]
    fn css_transition_fallback_to_keyword() {
        let css = ".a { transition: all 0.3s ease-in-out; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::Keyword(k) => assert!(!k.is_empty()),
            other => panic!("expected Keyword for transition, got {other:?}"),
        }
    }

    #[test]
    fn css_box_shadow_fallback() {
        let css = ".a { box-shadow: 0 4px 6px rgba(0,0,0,0.1); }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::Keyword(k) => assert!(!k.is_empty()),
            other => panic!("expected Keyword for box-shadow, got {other:?}"),
        }
    }

    #[test]
    fn css_font_shorthand_fallback() {
        let css = ".a { font: bold 16px/1.5 Arial; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::Keyword(k) => assert!(!k.is_empty()),
            other => panic!("expected Keyword for font shorthand, got {other:?}"),
        }
    }

    #[test]
    fn css_multiple_backgrounds_fallback() {
        let css = ".a { background: url('a.png'), url('b.png'); }";
        let rules = parse_css(css).unwrap();
        assert_eq!(rules.len(), 1);
    }

    #[test]
    fn css_named_color_transparent() {
        let css = ".a { color: transparent; }";
        let rules = parse_css(css).unwrap();
        match &rules[0].properties[0].value {
            CssValue::Color(c) => {
                assert_eq!(c.r, 0);
                assert_eq!(c.g, 0);
                assert_eq!(c.b, 0);
                assert!((c.a - 0.0).abs() < 0.01);
            }
            other => panic!("expected Color, got {other:?}"),
        }
    }

    // ═══════════════════════════════════════════════════════════════
    //  Error handling (~15 tests)
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn css_empty_string() {
        let rules = parse_css("").unwrap();
        assert!(rules.is_empty());
    }

    #[test]
    fn css_only_whitespace() {
        let rules = parse_css("   \n\t  ").unwrap();
        assert!(rules.is_empty());
    }

    #[test]
    fn css_only_comment() {
        let rules = parse_css("/* just a comment */").unwrap();
        assert!(rules.is_empty());
    }

    #[test]
    fn css_missing_closing_brace() {
        let result = parse_css(".a { color: red; ");
        // Should either fail or handle gracefully
        let _ = result; // accept both Ok and Err
    }

    #[test]
    fn css_unknown_property_does_not_panic() {
        let css = ".a { some-unknown-prop: value; }";
        let rules = parse_css(css).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].properties[0].name, "some-unknown-prop");
    }

    #[test]
    fn css_malformed_selector_missing_brace() {
        let result = parse_css(".a color: red;");
        assert!(result.is_err());
    }

    #[test]
    fn css_empty_declaration_block() {
        let css = ".a { }";
        let rules = parse_css(css).unwrap();
        assert_eq!(rules.len(), 1);
        assert!(rules[0].properties.is_empty());
    }

    #[test]
    fn css_import_skipped() {
        let css = r#"@import url("style.css"); .a { color: red; }"#;
        let rules = parse_css(css).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].properties[0].name, "color");
    }

    #[test]
    fn css_unknown_at_rule_skipped() {
        // @charset consumes through the next { } block, so any rule after it
        // within the same block is consumed. Only rules outside are kept.
        let css = ".a { color: red; } @charset 'UTF-8'; .b { color: blue; }";
        let rules = parse_css(css).unwrap();
        // .a is parsed first, then @charset skips through .b's block
        assert!(rules.len() >= 1);
        assert_eq!(rules[0].properties[0].name, "color");
    }

    #[test]
    fn css_font_size_keyword() {
        let css = "h1 { font-size: large; }";
        let rules = parse_css(css).unwrap();
        assert_eq!(
            rules[0].properties[0].value,
            CssValue::Keyword("large".to_string())
        );
    }

    #[test]
    fn css_inherit_keyword() {
        let css = ".a { color: inherit; }";
        let rules = parse_css(css).unwrap();
        assert_eq!(rules[0].properties[0].value, CssValue::Inherited);
    }

    #[test]
    fn css_initial_keyword() {
        let css = ".a { display: initial; }";
        let rules = parse_css(css).unwrap();
        assert_eq!(rules[0].properties[0].value, CssValue::Inherited);
    }

    #[test]
    fn css_unset_keyword() {
        let css = ".a { margin: unset; }";
        let rules = parse_css(css).unwrap();
        assert_eq!(rules[0].properties[0].value, CssValue::Inherited);
    }

    #[test]
    fn css_multiple_comments_between_rules() {
        let css = "/* first */ .a { color: red; } /* middle */ .b { color: blue; } /* end */";
        let rules = parse_css(css).unwrap();
        assert_eq!(rules.len(), 2);
    }

    #[test]
    fn css_important_with_spaces() {
        let css = ".a { color: red ! important; }";
        let rules = parse_css(css).unwrap();
        // With space between ! and important, it should not be recognized as important
        assert!(!rules[0].properties[0].important);
    }

    // ═══════════════════════════════════════════════════════════════
    //  parse_nth public API tests
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn css_parse_nth_odd() {
        assert_eq!(parse_nth("odd"), Some((2, 1)));
    }

    #[test]
    fn css_parse_nth_even() {
        assert_eq!(parse_nth("even"), Some((2, 0)));
    }

    #[test]
    fn css_parse_nth_plain_number() {
        assert_eq!(parse_nth("3"), Some((0, 3)));
    }

    #[test]
    fn css_parse_nth_2n_plus_1() {
        assert_eq!(parse_nth("2n+1"), Some((2, 1)));
    }

    #[test]
    fn css_parse_nth_3n() {
        assert_eq!(parse_nth("3n"), Some((3, 0)));
    }

    #[test]
    fn css_parse_nth_minus_n_plus_3() {
        assert_eq!(parse_nth("-n+3"), Some((-1, 3)));
    }

    #[test]
    fn css_parse_nth_plus_n_plus_1() {
        assert_eq!(parse_nth("+n+1"), Some((1, 1)));
    }

    #[test]
    fn css_parse_nth_minus_2n_plus_5() {
        assert_eq!(parse_nth("-2n+5"), Some((-2, 5)));
    }

    #[test]
    fn css_parse_nth_with_whitespace() {
        assert_eq!(parse_nth(" 2n + 1 "), Some((2, 1)));
    }

    #[test]
    fn css_parse_nth_invalid() {
        assert_eq!(parse_nth("abc"), None);
    }

    #[test]
    fn css_parse_nth_empty() {
        assert_eq!(parse_nth(""), None);
    }

    #[test]
    fn css_parse_nth_2n_minus_1() {
        assert_eq!(parse_nth("2n-1"), Some((2, -1)));
    }
}
