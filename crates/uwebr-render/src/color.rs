use uwebr_css::ast::Color as CssColor;
use vello::peniko;

/// Convert uwebr-css Color to vello peniko::Color (wrapper to avoid orphan rule)
pub fn css_color_to_peniko(c: CssColor) -> peniko::Color {
    let a = (c.a * 255.0) as u8;
    peniko::Color::from_rgba8(c.r, c.g, c.b, a)
}

/// Named CSS colors
const NAMED_COLORS: &[(&str, (u8, u8, u8))] = &[
    ("black", (0, 0, 0)),
    ("white", (255, 255, 255)),
    ("red", (255, 0, 0)),
    ("green", (0, 128, 0)),
    ("blue", (0, 0, 255)),
    ("yellow", (255, 255, 0)),
    ("cyan", (0, 255, 255)),
    ("magenta", (255, 0, 255)),
    ("gray", (128, 128, 128)),
    ("grey", (128, 128, 128)),
    ("orange", (255, 165, 0)),
    ("purple", (128, 0, 128)),
    ("transparent", (0, 0, 0)),
];

/// Parse a CSS color string into a peniko::Color
pub fn parse_color_to_peniko(color_str: &str) -> Option<peniko::Color> {
    let s = color_str.trim().to_lowercase();

    // Named colors
    for &(name, (r, g, b)) in NAMED_COLORS {
        if name == s {
            return Some(peniko::Color::from_rgb8(r, g, b));
        }
    }

    // Hex colors
    let hex = s.strip_prefix('#')?;
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            Some(peniko::Color::from_rgb8(r, g, b))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(peniko::Color::from_rgb8(r, g, b))
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            Some(peniko::Color::from_rgba8(r, g, b, a))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_css_color_to_peniko() {
        let css = CssColor {
            r: 255,
            g: 128,
            b: 0,
            a: 1.0,
        };
        let peniko = css_color_to_peniko(css);
        assert_eq!(peniko, peniko::Color::from_rgba8(255, 128, 0, 255));
    }

    #[test]
    fn test_parse_hex_color_6() {
        let c = parse_color_to_peniko("#ff8000").unwrap();
        assert_eq!(c, peniko::Color::from_rgb8(255, 128, 0));
    }

    #[test]
    fn test_parse_hex_color_3() {
        let c = parse_color_to_peniko("#f00").unwrap();
        assert_eq!(c, peniko::Color::from_rgb8(255, 0, 0));
    }

    #[test]
    fn test_parse_hex_color_8() {
        let c = parse_color_to_peniko("#ff000080").unwrap();
        assert_eq!(c, peniko::Color::from_rgba8(255, 0, 0, 128));
    }

    #[test]
    fn test_parse_named_color() {
        let c = parse_color_to_peniko("red").unwrap();
        assert_eq!(c, peniko::Color::from_rgb8(255, 0, 0));
    }

    #[test]
    fn test_parse_invalid_color() {
        assert!(parse_color_to_peniko("notacolor").is_none());
    }

    // ── Color edge-case tests ───────────────────────────────────

    #[test]
    fn render_parse_hex_color_uppercase() {
        let c = parse_color_to_peniko("#FF8000").unwrap();
        assert_eq!(c, peniko::Color::from_rgb8(255, 128, 0));
    }

    #[test]
    fn render_parse_hex_color_mixed_case() {
        let c = parse_color_to_peniko("#aAbBcC").unwrap();
        assert_eq!(c, peniko::Color::from_rgb8(0xaa, 0xbb, 0xcc));
    }

    #[test]
    fn render_parse_named_color_blue() {
        let c = parse_color_to_peniko("blue").unwrap();
        assert_eq!(c, peniko::Color::from_rgb8(0, 0, 255));
    }

    #[test]
    fn render_parse_named_color_green() {
        let c = parse_color_to_peniko("green").unwrap();
        assert_eq!(c, peniko::Color::from_rgb8(0, 128, 0));
    }

    #[test]
    fn render_parse_named_color_transparent() {
        let c = parse_color_to_peniko("transparent").unwrap();
        assert_eq!(c, peniko::Color::from_rgb8(0, 0, 0));
    }

    #[test]
    fn render_parse_named_color_with_whitespace() {
        let c = parse_color_to_peniko("  red  ").unwrap();
        assert_eq!(c, peniko::Color::from_rgb8(255, 0, 0));
    }

    #[test]
    fn render_parse_hex_color_invalid_length() {
        assert!(parse_color_to_peniko("#1234").is_none());
        assert!(parse_color_to_peniko("#12345").is_none());
        assert!(parse_color_to_peniko("#1234567").is_none());
        assert!(parse_color_to_peniko("#123456789").is_none());
    }

    #[test]
    fn render_parse_hex_color_invalid_chars() {
        assert!(parse_color_to_peniko("#gggggg").is_none());
        assert!(parse_color_to_peniko("#zzzzzzzz").is_none());
    }

    #[test]
    fn render_css_color_alpha_half() {
        let css = CssColor {
            r: 255,
            g: 0,
            b: 0,
            a: 0.5,
        };
        let c = css_color_to_peniko(css);
        assert_eq!(c, peniko::Color::from_rgba8(255, 0, 0, 127));
    }

    #[test]
    fn render_css_color_alpha_zero() {
        let css = CssColor {
            r: 0,
            g: 0,
            b: 0,
            a: 0.0,
        };
        let c = css_color_to_peniko(css);
        assert_eq!(c, peniko::Color::from_rgba8(0, 0, 0, 0));
    }

    #[test]
    fn render_css_color_alpha_full() {
        let css = CssColor {
            r: 100,
            g: 200,
            b: 50,
            a: 1.0,
        };
        let c = css_color_to_peniko(css);
        assert_eq!(c, peniko::Color::from_rgba8(100, 200, 50, 255));
    }

    #[test]
    fn render_parse_empty_string_color() {
        assert!(parse_color_to_peniko("").is_none());
    }

    #[test]
    fn render_parse_hex_without_hash() {
        assert!(parse_color_to_peniko("ff0000").is_none());
    }
}
