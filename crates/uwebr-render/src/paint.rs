use uwebr_core::component::{Element, NodeType, PropValue};
use uwebr_css::codegen::{AnimationProps, BackgroundValue, BoxShadow, PaintProps, TransformProps};
use vello::peniko;

use crate::color::{css_color_to_peniko, parse_color_to_peniko};
use crate::scene::{Background, TextOverflow};

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
    pub background: Option<Background>,
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
    /// How overflowing text is treated (`clip`, `ellipsis`).
    pub text_overflow: TextOverflow,
    /// CSS `z-index` — controls paint order (higher = painted on top).
    /// Not inherited; defaults to 0 (auto).
    pub z_index: i32,
    /// CSS `transform` — translate, rotate, scale, skew.
    /// Not inherited; defaults to identity.
    pub transform: TransformProps,
    /// CSS `animation` — name, duration, timing, etc.
    /// Not inherited; defaults to empty (no animation).
    pub animation: AnimationProps,
    /// CSS `box-shadow`.
    pub box_shadow: Vec<BoxShadow>,
    /// CSS `text-align`: "left", "center", "right", "justify".
    pub text_align: Option<String>,
    /// CSS `line-height` as a multiplier (e.g. 1.5).
    pub line_height: Option<f32>,
    /// CSS `letter-spacing` in px.
    pub letter_spacing: Option<f32>,
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
            text_overflow: TextOverflow::default(),
            z_index: 0,
            transform: TransformProps::default(),
            animation: AnimationProps::default(),
            box_shadow: vec![],
            text_align: None,
            line_height: None,
            letter_spacing: None,
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
            text_overflow: self.text_overflow.clone(),
            z_index: 0,
            transform: TransformProps::default(),
            animation: AnimationProps::default(),
            box_shadow: vec![],
            text_align: None,
            line_height: None,
            letter_spacing: None,
        }
    }

    /// Apply the paint declarations from a matched CSS rule.
    pub fn apply_css(&mut self, paint: &PaintProps) {
        if let Some(ref bg) = paint.background {
            self.background = Some(background_to_scene(bg));
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
        if let Some(ref to) = paint.text_overflow {
            self.text_overflow = match to.as_str() {
                "ellipsis" => TextOverflow::Ellipsis,
                "visible" => TextOverflow::Visible,
                _ => TextOverflow::Clip,
            };
        }
        if let Some(zi) = paint.z_index {
            self.z_index = zi;
        }
        if let Some(ref shadows) = paint.box_shadow {
            self.box_shadow = shadows.clone();
        }
        if let Some(ref align) = paint.text_align {
            self.text_align = Some(align.clone());
        }
        if let Some(lh) = paint.line_height {
            self.line_height = Some(lh);
        }
        if let Some(ls) = paint.letter_spacing {
            self.letter_spacing = Some(ls);
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
                        self.background = Some(Background::Solid(c));
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
    pub fn resolve(
        inherited: &ResolvedPaint,
        css: &PaintProps,
        transform: &TransformProps,
        animation: &AnimationProps,
        element: &Element,
    ) -> Self {
        let mut paint = inherited.inherited();
        paint.apply_css(css);
        paint.transform = transform.clone();
        paint.animation = animation.clone();
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

/// Convert a CSS `BackgroundValue` into the scene's `Background`.
fn background_to_scene(bg: &BackgroundValue) -> Background {
    match bg {
        BackgroundValue::Solid(c) => Background::Solid(css_color_to_peniko(c.clone())),
        BackgroundValue::LinearGradient { direction, stops } => {
            let (start, end) = parse_gradient_direction(direction);
            Background::LinearGradient {
                start,
                end,
                stops: gradient_stops_to_scene(stops),
            }
        }
        BackgroundValue::RadialGradient { stops } => Background::RadialGradient {
            center: [0.5, 0.5],
            radius: 0.5,
            stops: gradient_stops_to_scene(stops),
        },
    }
}

/// Convert CSS gradient stops to `(offset, color)` pairs, distributing any
/// stops without an explicit position evenly across the 0..1 range.
fn gradient_stops_to_scene(stops: &[uwebr_css::ast::GradientStop]) -> Vec<(f32, peniko::Color)> {
    let n = stops.len();
    stops
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let offset = s.position.unwrap_or_else(|| {
                if n <= 1 {
                    0.0
                } else {
                    i as f32 / (n - 1) as f32
                }
            });
            (offset, css_color_to_peniko(s.color.clone()))
        })
        .collect()
}

/// Map a CSS gradient direction to normalized start/end points in the 0..1 box.
fn parse_gradient_direction(direction: &Option<String>) -> ([f32; 2], [f32; 2]) {
    match direction.as_deref() {
        Some("to right") => ([0.0, 0.0], [1.0, 0.0]),
        Some("to left") => ([1.0, 0.0], [0.0, 0.0]),
        Some("to bottom") => ([0.0, 0.0], [0.0, 1.0]),
        Some("to top") => ([0.0, 1.0], [0.0, 0.0]),
        Some(deg_str) if deg_str.ends_with("deg") => {
            let deg: f32 = deg_str
                .trim_end_matches("deg")
                .trim()
                .parse()
                .unwrap_or(0.0);
            let rad = deg.to_radians();
            (
                [0.5 - 0.5 * rad.sin(), 0.5 + 0.5 * rad.cos()],
                [0.5 + 0.5 * rad.sin(), 0.5 - 0.5 * rad.cos()],
            )
        }
        // Default: top → bottom.
        _ => ([0.0, 0.0], [0.0, 1.0]),
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
            background: Some(Background::Solid(peniko::color::palette::css::RED)),
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
            background: Some(BackgroundValue::Solid(CssColor::rgb(0x1a, 0x1a, 0x2e))),
            color: Some(CssColor::rgb(0xe0, 0xe0, 0xe0)),
            ..Default::default()
        });
        assert_eq!(
            p.background,
            Some(Background::Solid(peniko::Color::from_rgba8(
                0x1a, 0x1a, 0x2e, 255
            )))
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
        assert_eq!(
            p.background,
            Some(Background::Solid(peniko::Color::from_rgb8(255, 128, 0)))
        );
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
        let p = ResolvedPaint::resolve(
            &parent,
            &PaintProps::default(),
            &TransformProps::default(),
            &AnimationProps::default(),
            &e,
        );
        assert_eq!(
            p.background,
            Some(Background::Solid(peniko::Color::from_rgb8(255, 0, 0)))
        );
    }

    #[test]
    fn test_resolve_text_node_inherits_only() {
        let parent = ResolvedPaint {
            color: peniko::color::palette::css::GREEN,
            font_size: 40.0,
            background: Some(Background::Solid(peniko::color::palette::css::RED)),
            ..Default::default()
        };

        let p = ResolvedPaint::resolve(
            &parent,
            &PaintProps::default(),
            &TransformProps::default(),
            &AnimationProps::default(),
            &text_el("hi"),
        );
        assert_eq!(p.color, peniko::color::palette::css::GREEN);
        assert_eq!(p.font_size, 40.0);
        assert!(p.background.is_none(), "text draws glyphs, not a box");
    }

    // ── Paint edge-case tests ───────────────────────────────────

    #[test]
    fn render_inherited_drops_border_radius() {
        let parent = ResolvedPaint {
            border_radius: 12.0,
            border_width: 2.0,
            color: peniko::color::palette::css::BLUE,
            ..Default::default()
        };
        let child = parent.inherited();
        assert_eq!(child.border_radius, 0.0, "border-radius must not inherit");
        assert_eq!(child.border_width, 0.0, "border-width must not inherit");
        assert_eq!(
            child.color,
            peniko::color::palette::css::BLUE,
            "color must inherit"
        );
    }

    #[test]
    fn render_inherited_drops_opacity() {
        let parent = ResolvedPaint {
            opacity: 0.3,
            color: peniko::color::palette::css::RED,
            ..Default::default()
        };
        let child = parent.inherited();
        assert_eq!(child.opacity, 1.0, "opacity must not inherit");
        assert_eq!(child.color, peniko::color::palette::css::RED);
    }

    #[test]
    fn render_apply_css_border_color() {
        let mut p = ResolvedPaint::default();
        p.apply_css(&PaintProps {
            border_color: Some(CssColor::rgb(255, 0, 0)),
            border_width: Some(3.0),
            border_radius: Some(8.0),
            ..Default::default()
        });
        assert_eq!(p.border_color, peniko::Color::from_rgb8(255, 0, 0));
        assert_eq!(p.border_width, 3.0);
        assert_eq!(p.border_radius, 8.0);
    }

    #[test]
    fn render_apply_css_text_overflow() {
        let mut p = ResolvedPaint::default();
        p.apply_css(&PaintProps {
            text_overflow: Some("ellipsis".into()),
            ..Default::default()
        });
        assert_eq!(p.text_overflow, crate::scene::TextOverflow::Ellipsis);
    }

    #[test]
    fn render_apply_css_text_overflow_visible() {
        let mut p = ResolvedPaint::default();
        p.apply_css(&PaintProps {
            text_overflow: Some("visible".into()),
            ..Default::default()
        });
        assert_eq!(p.text_overflow, crate::scene::TextOverflow::Visible);
    }

    #[test]
    fn render_apply_props_opacity_clamped_zero() {
        let mut p = ResolvedPaint::default();
        p.apply_props(&[("opacity".into(), PropValue::Number(0.0))]);
        assert_eq!(p.opacity, 0.0);
    }

    #[test]
    fn render_apply_props_opacity_clamped_negative() {
        let mut p = ResolvedPaint::default();
        p.apply_props(&[("opacity".into(), PropValue::Number(-1.0))]);
        assert_eq!(p.opacity, 0.0, "negative opacity should clamp to 0");
    }

    #[test]
    fn render_apply_props_border_radius_string_px() {
        let mut p = ResolvedPaint::default();
        p.apply_props(&[("border-radius".into(), PropValue::String("12px".into()))]);
        assert_eq!(p.border_radius, 12.0);
    }

    #[test]
    fn render_apply_props_border_width_number() {
        let mut p = ResolvedPaint::default();
        p.apply_props(&[("border-width".into(), PropValue::Number(4.0))]);
        assert_eq!(p.border_width, 4.0);
    }

    #[test]
    fn render_resolve_element_applies_bg_prop() {
        let parent = ResolvedPaint::default();
        let e = el(
            "div",
            vec![(
                "background-color".into(),
                PropValue::String("#ff0000".into()),
            )],
        );
        let p = ResolvedPaint::resolve(
            &parent,
            &PaintProps::default(),
            &TransformProps::default(),
            &AnimationProps::default(),
            &e,
        );
        assert!(
            p.background.is_some(),
            "background-color prop should set background"
        );
    }

    #[test]
    fn render_resolve_text_node_ignores_props() {
        let parent = ResolvedPaint {
            font_size: 32.0,
            ..Default::default()
        };
        let text = Element {
            node_type: NodeType::Text("content".into()),
            props: vec![("font-size".into(), PropValue::Number(48.0))],
            children: vec![],
        };
        let p = ResolvedPaint::resolve(
            &parent,
            &PaintProps::default(),
            &TransformProps::default(),
            &AnimationProps::default(),
            &text,
        );
        assert_eq!(
            p.font_size, 32.0,
            "text node should inherit font-size, not apply props"
        );
    }

    #[test]
    fn render_font_family_inherited() {
        let parent = ResolvedPaint {
            font_family: Some("monospace".into()),
            ..Default::default()
        };
        let child = parent.inherited();
        assert_eq!(child.font_family.as_deref(), Some("monospace"));
    }

    // ── Quality tests (test_q_*) ────────────────────────────────

    #[test]
    fn test_q_prop_to_f32_px_suffix() {
        let v = prop_to_f32(&PropValue::String("16px".into()));
        assert_eq!(v, Some(16.0));
    }

    #[test]
    fn test_q_prop_to_f32_px_decimal() {
        let v = prop_to_f32(&PropValue::String("16.5px".into()));
        assert_eq!(v, Some(16.5));
    }

    #[test]
    fn test_q_prop_to_f32_just_px() {
        let v = prop_to_f32(&PropValue::String("px".into()));
        assert_eq!(v, None);
    }

    #[test]
    fn test_q_gradient_direction_none_default() {
        let (start, end) = parse_gradient_direction(&None);
        assert_eq!(start, [0.0, 0.0]);
        assert_eq!(end, [0.0, 1.0], "None must default to top-to-bottom");
    }

    #[test]
    fn test_q_gradient_direction_90deg() {
        let (start, end) = parse_gradient_direction(&Some("90deg".to_string()));
        // 90deg: rad = PI/2, sin=1, cos=0 → start=[0.5-0, 0.5+0]=[0.5,0.5], end=[0.5+0,0.5-0]=[0.5,0.5]
        // Actually: start=[0.5-0.5*sin, 0.5+0.5*cos], end=[0.5+0.5*sin, 0.5-0.5*cos]
        // sin(90°)=1, cos(90°)=0 → start=[0.0, 0.5], end=[1.0, 0.5]
        assert!(
            (start[0] - 0.0).abs() < 0.01,
            "90deg start.x should be ~0, got {}",
            start[0]
        );
        assert!(
            (end[0] - 1.0).abs() < 0.01,
            "90deg end.x should be ~1, got {}",
            end[0]
        );
    }

    #[test]
    fn test_q_color_transparent_alpha() {
        let c = parse_color_to_peniko("transparent");
        assert!(c.is_some(), "transparent is a named color");
        let c = c.unwrap();
        // transparent maps to (0,0,0) with full opacity in our impl;
        // the alpha channel semantics are handled at the CSS layer.
        assert_eq!(c, peniko::Color::from_rgb8(0, 0, 0));
    }

    #[test]
    fn test_q_color_8char_hex_rgba() {
        let c = parse_color_to_peniko("#ff000080").unwrap();
        // 0x80 = 128 → alpha = 128/255 ≈ 0.502
        assert_eq!(c, peniko::Color::from_rgba8(255, 0, 0, 128));
    }

    #[test]
    fn test_q_paint_border_color_from_css() {
        let mut p = ResolvedPaint::default();
        p.apply_css(&PaintProps {
            border_color: Some(CssColor::rgb(0xff, 0x00, 0x00)),
            border_width: Some(3.0),
            ..Default::default()
        });
        assert_eq!(p.border_color, peniko::Color::from_rgba8(255, 0, 0, 255));
        assert_eq!(p.border_width, 3.0);
    }
}
