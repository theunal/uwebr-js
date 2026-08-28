use serde::{Deserialize, Serialize};

/// CSS rule: selector { properties }
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CssRule {
    pub selector: CssSelector,
    pub properties: Vec<CssProperty>,
    pub media_query: Option<String>,
}

/// CSS selector
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CssSelector {
    /// .classname
    Class(String),
    /// #id
    Id(String),
    /// tagname
    Tag(String),
    /// *
    Universal,
    /// .a .b (descendant)
    Descendant(Vec<CssSelector>),
    /// .a > .b (child)
    Child(Vec<CssSelector>),
    /// .a, .b (list)
    List(Vec<CssSelector>),
}

/// CSS property
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CssProperty {
    pub name: String,
    pub value: CssValue,
    pub important: bool,
}

/// CSS property value
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CssValue {
    /// px, em, rem, %, etc.
    Length(f32, LengthUnit),
    /// #ff0000, rgb(), hsl()
    Color(Color),
    /// flex, block, grid, etc.
    Keyword(String),
    /// flex: 1 0 auto
    Shorthand(Vec<CssValue>),
    /// unset, inherit, initial
    Inherited,
    /// none, auto
    Auto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LengthUnit {
    Px,
    Em,
    Rem,
    Percent,
    Vw,
    Vh,
    Auto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

impl Color {
    pub fn from_hex(hex: &str) -> Self {
        let hex = hex.trim_start_matches('#');
        match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1], 16).unwrap_or(0) * 17;
                let g = u8::from_str_radix(&hex[1..2], 16).unwrap_or(0) * 17;
                let b = u8::from_str_radix(&hex[2..3], 16).unwrap_or(0) * 17;
                Color { r, g, b, a: 1.0 }
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
                let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
                let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
                Color { r, g, b, a: 1.0 }
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
                let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
                let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
                let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255) as f32 / 255.0;
                Color { r, g, b, a }
            }
            _ => Color {
                r: 0,
                g: 0,
                b: 0,
                a: 1.0,
            },
        }
    }

    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 1.0 }
    }

    pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> Self {
        Color { r, g, b, a }
    }
}
