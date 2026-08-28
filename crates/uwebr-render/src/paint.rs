use uwebr_core::component::{Element, NodeType, PropValue};
use uwebr_css::codegen::PaintProps;
use vello::peniko;

use crate::color::{css_color_to_peniko, parse_color_to_peniko};

/// Default text colour when neither CSS nor props specify one.
pub const DEFAULT_TEXT_COLOR: peniko::Color = peniko::color::palette::css::WHITE;
/// Default font size in px (matches the CSS initial value).
pub const DEFAULT_FONT_SIZE: f32 = 16.0;

/// Fully resolved paint for one node — the concrete values the scene needs.
///
/// Taffy models layout only, so `background-color`, `color`, `font-size` and
/// friends would be dropped at the layout boundary. This carries them through
/// so `SceneBuilder` can actually paint them.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedPaint {
    /// Fill for the node's box. `None` means "draw nothing".
    pub background: Option<peniko::Color>,
    /// Text colour — inherited by descendants.
    pub color: peniko::Color,
    /// Font size in px — inherited by descendants.
    pub font_size: f32,
    /// CSS `font-family` list — inherited by descendants.
    pub font_family: Option<String>,
    pub border_color: peniko::Color,
    pub border_width: f32,
    pub border_radius: f32,
    pub opacity: f32,
}

impl Default for ResolvedPaint {
    fn default() -> Self {
        Self {
            background: None,
            color: DEFAULT_TEXT_COLOR,
            font_size: DEFAULT_FONT_SIZE,
            font_family: None,
            border_color: peniko::color::palette::css::WHITE,
            border_width: 0.0,
            border_radius: 0.0,
            opacity: 1.0,
        }
    }
}

impl ResolvedPaint {
    /// Seed for a child node: keep the inheritable text properties, drop the
    /// box-local ones (background, border, opacity are not inherited in CSS).
    pub fn inherited(&self) -> Self {
        Self {
            background: None,
            color: self.color,
            font_size: self.font_size,
            font_family: self.font_family.clone(),
            border_color: peniko::color::palette::css::WHITE,
            border_width: 0.0,
            border_radius: 0.0,
            opacity: 1.0,
        }
    }

    /// Apply the paint declarations from a matched CSS rule.
    pub fn apply_css(&mut self, paint: &PaintProps) {
        if let Some(ref bg) = paint.background {
            self.background = Some(css_color_to_peniko(bg.clone()));
        }
        if let Some(ref c) = paint.color {
            self.color = css_color_to_peniko(c.clone());
        }
        if let Some(size) = paint.font_size {
            self.font_size = size;
        }
        if let Some(ref family) = paint.font_family {
            self.font_family = Some(family.clone());
        }
        if let Some(ref c) = paint.border_color {
            self.border_color = css_color_to_peniko(c.clone());
        }
        if let Some(w) = paint.border_width {
            self.border_width = w;
        }
        if let Some(r) = paint.border_radius {
            self.border_radius = r;
        }
        if let Some(o) = paint.opacity {
            self.opacity = o;
        }
    }

    /// Apply inline element props, which win over CSS rules.
    ///
    /// The transpiler emits every literal HTML attribute as `PropValue::String`,
    /// so numeric props must accept both `Number` and `String`.
    pub fn apply_props(&mut self, props: &[(String, PropValue)]) {
        for (name, value) in props {
            match name.as_str() {
                "background" | "background-color" | "bg" => {
                    if let Some(c) = prop_to_color(value) {
                        self.background = Some(c);
                    }
                }
                "color" | "text_color" | "text-color" => {
                    if let Some(c) = prop_to_color(value) {
                        self.color = c;
                    }
                }
                "font_size" | "font-size" => {
                    if let Some(n) = prop_to_f32(value) {
                        self.font_size = n;
                    }
                }
                "font_family" | "font-family" => {
                    if let PropValue::String(s) = value {
                        self.font_family = Some(s.clone());
                    }
                }
                "border_color" | "border-color" => {
                    if let Some(c) = prop_to_color(value) {
                        self.border_color = c;
                    }
                }
                "border_width" | "border-width" | "border" => {
                    if let Some(n) = prop_to_f32(value) {
                        self.border_width = n;
                    }
                }
                "border_radius" | "border-radius" | "rounded" => {
                    if let Some(n) = prop_to_f32(value) {
                        self.border_radius = n;
                    }
                }
                "opacity" => {
                    if let Some(n) = prop_to_f32(value) {
                        self.opacity = n.clamp(0.0, 1.0);
                    }
                }
                _ => {}
            }
        }
    }

    /// Resolve the paint for one element given its inherited context.
    pub fn resolve(inherited: &ResolvedPaint, css: &PaintProps, element: &Element) -> Self {
        let mut paint = inherited.inherited();
        paint.apply_css(css);
        // Text nodes never carry their own attributes; they only inherit.
        if !matches!(element.node_type, NodeType::Text(_)) {
            paint.apply_props(&element.props);
        }
        paint
    }
}

/// Read a colour from a prop value (named or hex string).
fn prop_to_color(value: &PropValue) -> Option<peniko::Color> {
    match value {
        PropValue::String(s) => parse_color_to_peniko(s),
        _ => None,
    }
}

/// Read an f32 from a prop value, accepting both `Number` and numeric strings.
///
/// Also tolerates CSS-ish suffixes ("16px") since props come straight from HTML
/// attributes.
fn prop_to_f32(value: &PropValue) -> Option<f32> {
    match value {
        PropValue::Number(n) => Some(*n as f32),
        PropValue::String(s) => {
            let t = s.trim();
            t.parse::<f32>()
                .ok()
                .or_else(|| t.trim_end_matches("px").trim().parse::<f32>().ok())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uwebr_css::ast::Color as CssColor;

    fn el(tag: &str, props: Vec<(String, PropValue)>) -> Element {
        Element {
            node_type: NodeType::Element(tag.into()),
            props,
            children: vec![],
        }
    }

    fn text_el(content: &str) -> Element {
        Element {
            node_type: NodeType::Text(content.into()),
            props: vec![],
            children: vec![],
        }
    }

    #[test]
    fn test_default_paint() {
        let p = ResolvedPaint::default();
        assert!(p.background.is_none());
        assert_eq!(p.font_size, DEFAULT_FONT_SIZE);
        assert_eq!(p.opacity, 1.0);
    }

    #[test]
    fn test_inherited_drops_background_keeps_color() {
        let parent = ResolvedPaint {
            background: Some(peniko::color::palette::css::RED),
            color: peniko::color::palette::css::BLUE,
            font_size: 32.0,
            ..Default::default()
        };

        let child = parent.inherited();
        assert!(child.background.is_none(), "background must not inherit");
        assert_eq!(child.color, peniko::color::palette::css::BLUE);
        assert_eq!(child.font_size, 32.0);
    }

    #[test]
    fn test_apply_css_background_and_color() {
        let mut p = ResolvedPaint::default();
        p.apply_css(&PaintProps {
            background: Some(CssColor::rgb(0x1a, 0x1a, 0x2e)),
            color: Some(CssColor::rgb(0xe0, 0xe0, 0xe0)),
            ..Default::default()
        });
        assert_eq!(
            p.background,
            Some(peniko::Color::from_rgba8(0x1a, 0x1a, 0x2e, 255))
        );
        assert_eq!(p.color, peniko::Color::from_rgba8(0xe0, 0xe0, 0xe0, 255));
    }

    #[test]
    fn test_apply_css_leaves_unspecified_alone() {
        let mut p = ResolvedPaint {
            font_size: 24.0,
            ..Default::default()
        };
        p.apply_css(&PaintProps {
            color: Some(CssColor::rgb(1, 2, 3)),
            ..Default::default()
        });
        assert_eq!(p.font_size, 24.0, "font-size was not in the rule");
    }

    #[test]
    fn test_props_override_css() {
        let mut p = ResolvedPaint::default();
        p.apply_css(&PaintProps {
            color: Some(CssColor::rgb(255, 0, 0)),
            ..Default::default()
        });
        p.apply_props(&[("color".into(), PropValue::String("blue".into()))]);
        assert_eq!(p.color, peniko::Color::from_rgb8(0, 0, 255));
    }

    #[test]
    fn test_font_size_from_string_prop() {
        // The transpiler emits every literal attribute as a String.
        let mut p = ResolvedPaint::default();
        p.apply_props(&[("font-size".into(), PropValue::String("28".into()))]);
        assert_eq!(p.font_size, 28.0);
    }

    #[test]
    fn test_font_size_from_px_string_prop() {
        let mut p = ResolvedPaint::default();
        p.apply_props(&[("font-size".into(), PropValue::String("28px".into()))]);
        assert_eq!(p.font_size, 28.0);
    }

    #[test]
    fn test_font_size_from_number_prop() {
        let mut p = ResolvedPaint::default();
        p.apply_props(&[("font_size".into(), PropValue::Number(20.0))]);
        assert_eq!(p.font_size, 20.0);
    }

    #[test]
    fn test_hex_background_prop() {
        let mut p = ResolvedPaint::default();
        p.apply_props(&[("bg".into(), PropValue::String("#ff8000".into()))]);
        assert_eq!(p.background, Some(peniko::Color::from_rgb8(255, 128, 0)));
    }

    #[test]
    fn test_opacity_clamped() {
        let mut p = ResolvedPaint::default();
        p.apply_props(&[("opacity".into(), PropValue::Number(5.0))]);
        assert_eq!(p.opacity, 1.0);
    }

    #[test]
    fn test_resolve_element_uses_props() {
        let parent = ResolvedPaint::default();
        let e = el("div", vec![("bg".into(), PropValue::String("red".into()))]);
        let p = ResolvedPaint::resolve(&parent, &PaintProps::default(), &e);
        assert_eq!(p.background, Some(peniko::Color::from_rgb8(255, 0, 0)));
    }

    #[test]
    fn test_resolve_text_node_inherits_only() {
        let parent = ResolvedPaint {
            color: peniko::color::palette::css::GREEN,
            font_size: 40.0,
            background: Some(peniko::color::palette::css::RED),
            ..Default::default()
        };

        let p = ResolvedPaint::resolve(&parent, &PaintProps::default(), &text_el("hi"));
        assert_eq!(p.color, peniko::color::palette::css::GREEN);
        assert_eq!(p.font_size, 40.0);
        assert!(p.background.is_none(), "text draws glyphs, not a box");
    }
}
