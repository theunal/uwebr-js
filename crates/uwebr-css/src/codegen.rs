use crate::ast::*;
use anyhow::Result;
use taffy::geometry::Point;
use taffy::prelude::*;
use taffy::style::Overflow;

/// Tracks which taffy `Style` fields a CSS rule actually specified.
///
/// Without this, merging two rules would reset unspecified fields back to
/// `Style::default()` — a class rule setting only `width` would wipe out the
/// `display` / `flex-direction` coming from a tag rule.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StyleMask {
    pub display: bool,
    pub flex_direction: bool,
    pub flex_wrap: bool,
    pub justify_content: bool,
    pub align_items: bool,
    pub align_self: bool,
    pub flex_grow: bool,
    pub flex_shrink: bool,
    pub flex_basis: bool,
    pub width: bool,
    pub height: bool,
    pub min_width: bool,
    pub min_height: bool,
    pub max_width: bool,
    pub max_height: bool,
    pub padding: bool,
    pub margin: bool,
    pub border: bool,
    pub position: bool,
    pub inset: bool,
    pub overflow: bool,
    pub gap_width: bool,
    pub gap_height: bool,
}

impl StyleMask {
    /// Union of two masks (used while cascading tag → class → id).
    pub fn or_assign(&mut self, other: &StyleMask) {
        self.display |= other.display;
        self.flex_direction |= other.flex_direction;
        self.flex_wrap |= other.flex_wrap;
        self.justify_content |= other.justify_content;
        self.align_items |= other.align_items;
        self.align_self |= other.align_self;
        self.flex_grow |= other.flex_grow;
        self.flex_shrink |= other.flex_shrink;
        self.flex_basis |= other.flex_basis;
        self.width |= other.width;
        self.height |= other.height;
        self.min_width |= other.min_width;
        self.min_height |= other.min_height;
        self.max_width |= other.max_width;
        self.max_height |= other.max_height;
        self.padding |= other.padding;
        self.margin |= other.margin;
        self.border |= other.border;
        self.position |= other.position;
        self.inset |= other.inset;
        self.overflow |= other.overflow;
        self.gap_width |= other.gap_width;
        self.gap_height |= other.gap_height;
    }

    /// True when the rule specified no layout property at all.
    pub fn is_empty(&self) -> bool {
        *self == StyleMask::default()
    }
}

/// Background paint — a solid colour or a gradient.
#[derive(Debug, Clone, PartialEq)]
pub enum BackgroundValue {
    Solid(Color),
    LinearGradient {
        direction: Option<String>,
        stops: Vec<GradientStop>,
    },
    RadialGradient {
        stops: Vec<GradientStop>,
    },
}

/// CSS paint properties that Taffy has no place for.
///
/// Taffy only models layout; `background-color`, `color`, `font-size` etc. would
/// otherwise be dropped at the layout boundary and never reach the scene.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PaintProps {
    pub background: Option<BackgroundValue>,
    pub color: Option<Color>,
    pub font_size: Option<f32>,
    pub font_family: Option<String>,
    pub border_color: Option<Color>,
    pub border_width: Option<f32>,
    pub border_radius: Option<f32>,
    pub opacity: Option<f32>,
    pub text_overflow: Option<String>,
}

impl PaintProps {
    /// True when the rule specified no paint property at all.
    pub fn is_empty(&self) -> bool {
        *self == PaintProps::default()
    }

    /// Overwrite only the fields the `other` rule actually specified.
    pub fn merge(&mut self, other: &PaintProps) {
        if other.background.is_some() {
            self.background = other.background.clone();
        }
        if other.color.is_some() {
            self.color = other.color.clone();
        }
        if other.font_size.is_some() {
            self.font_size = other.font_size;
        }
        if other.font_family.is_some() {
            self.font_family = other.font_family.clone();
        }
        if other.border_color.is_some() {
            self.border_color = other.border_color.clone();
        }
        if other.border_width.is_some() {
            self.border_width = other.border_width;
        }
        if other.border_radius.is_some() {
            self.border_radius = other.border_radius;
        }
        if other.opacity.is_some() {
            self.opacity = other.opacity;
        }
        if other.text_overflow.is_some() {
            self.text_overflow = other.text_overflow.clone();
        }
    }
}

/// A CSS rule converted for runtime use: layout + paint + "what was specified".
#[derive(Debug, Clone)]
pub struct StyleEntry {
    pub selector: String,
    /// The parsed selector AST, used for pseudo-class / attribute matching.
    /// `None` for legacy string-only entries built via `from_rules`.
    pub selector_ast: Option<CssSelector>,
    pub style: Style,
    pub mask: StyleMask,
    pub paint: PaintProps,
    /// Whether any declaration in this rule carried `!important`. Used by the
    /// cascade so an important rule beats a higher-specificity normal rule.
    pub important: bool,
}

/// Convert CssRule list to Vec<(String, Style)> for runtime use
pub fn convert_to_taffy_styles(rules: &[CssRule]) -> Result<Vec<(String, Style)>> {
    Ok(convert_to_style_entries(rules)?
        .into_iter()
        .map(|entry| (entry.selector, entry.style))
        .collect())
}

/// Default viewport used when the caller has no real window size yet.
const DEFAULT_VIEWPORT: (f32, f32) = (1920.0, 1080.0);

/// Convert CssRule list into full style entries (layout + mask + paint).
///
/// Uses a default viewport; call [`convert_to_style_entries_vp`] to resolve
/// `vw`/`vh` against real window dimensions.
pub fn convert_to_style_entries(rules: &[CssRule]) -> Result<Vec<StyleEntry>> {
    convert_to_style_entries_vp(rules, DEFAULT_VIEWPORT.0, DEFAULT_VIEWPORT.1)
}

/// Convert CssRule list into full style entries, resolving `vw`/`vh` against the
/// given viewport dimensions.
pub fn convert_to_style_entries_vp(rules: &[CssRule], vw: f32, vh: f32) -> Result<Vec<StyleEntry>> {
    let mut entries = Vec::with_capacity(rules.len());

    for rule in rules {
        let selector = selector_key(&rule.selector);
        let mut style = Style::default();
        let mut mask = StyleMask::default();

        for prop in &rule.properties {
            apply_property(&mut style, &mut mask, &prop.name, &prop.value, vw, vh);
        }

        entries.push(StyleEntry {
            selector,
            selector_ast: Some(rule.selector.clone()),
            style,
            mask,
            paint: extract_paint(&rule.properties),
            important: rule.properties.iter().any(|p| p.important),
        });
    }

    Ok(entries)
}

/// Pull the non-layout (paint) properties out of a rule's declarations.
pub fn extract_paint(properties: &[CssProperty]) -> PaintProps {
    let mut paint = PaintProps::default();

    for prop in properties {
        match prop.name.as_str() {
            "background" | "background-color" => match &prop.value {
                CssValue::Color(c) => {
                    paint.background = Some(BackgroundValue::Solid(c.clone()));
                }
                CssValue::LinearGradient { direction, stops } => {
                    paint.background = Some(BackgroundValue::LinearGradient {
                        direction: direction.clone(),
                        stops: stops.clone(),
                    });
                }
                CssValue::RadialGradient { stops } => {
                    paint.background = Some(BackgroundValue::RadialGradient {
                        stops: stops.clone(),
                    });
                }
                _ => {}
            },
            "color" => {
                if let CssValue::Color(c) = &prop.value {
                    paint.color = Some(c.clone());
                }
            }
            "border-color" => {
                if let CssValue::Color(c) = &prop.value {
                    paint.border_color = Some(c.clone());
                }
            }
            "font-size" => {
                if let Some(px) = to_px(&prop.value) {
                    paint.font_size = Some(px);
                }
            }
            "font-family" => {
                if let CssValue::Keyword(k) = &prop.value {
                    paint.font_family = Some(k.clone());
                }
            }
            "border-width" => {
                if let Some(px) = to_px(&prop.value) {
                    paint.border_width = Some(px);
                }
            }
            "border-radius" => {
                if let Some(px) = to_px(&prop.value) {
                    paint.border_radius = Some(px);
                }
            }
            "opacity" => {
                if let CssValue::Length(n, _) = &prop.value {
                    paint.opacity = Some(n.clamp(0.0, 1.0));
                }
            }
            "text-overflow" => {
                if let CssValue::Keyword(k) = &prop.value {
                    paint.text_overflow = Some(k.clone());
                }
            }
            _ => {}
        }
    }

    paint
}

/// Resolve a CSS length to absolute pixels. `em`/`rem` assume a 16px root.
fn to_px(value: &CssValue) -> Option<f32> {
    match value {
        CssValue::Length(n, unit) => match unit {
            LengthUnit::Px => Some(*n),
            LengthUnit::Em | LengthUnit::Rem => Some(*n * 16.0),
            _ => None,
        },
        _ => None,
    }
}

fn selector_key(sel: &CssSelector) -> String {
    match sel {
        CssSelector::Class(name) => format!(".{}", name),
        CssSelector::Id(name) => format!("#{}", name),
        CssSelector::Tag(name) => name.clone(),
        CssSelector::Universal => "*".to_string(),
        CssSelector::Descendant(parts) => {
            let keys: Vec<String> = parts.iter().map(selector_key).collect();
            keys.join(" ")
        }
        CssSelector::Child(parts) => {
            let keys: Vec<String> = parts.iter().map(selector_key).collect();
            keys.join(" > ")
        }
        CssSelector::List(parts) => {
            let keys: Vec<String> = parts.iter().map(selector_key).collect();
            keys.join(", ")
        }
        CssSelector::PseudoClass(inner, pseudo) => {
            format!("{}:{}", selector_key(inner), pseudo)
        }
        CssSelector::Attribute {
            selector,
            attr,
            op,
            value,
        } => {
            let base = selector_key(selector);
            match (op, value) {
                (AttributeOp::Exists, _) => format!("{base}[{attr}]"),
                (AttributeOp::Equals, Some(v)) => format!("{base}[{attr}=\"{v}\"]"),
                (AttributeOp::Includes, Some(v)) => format!("{base}[{attr}~=\"{v}\"]"),
                (AttributeOp::Prefix, Some(v)) => format!("{base}[{attr}^=\"{v}\"]"),
                (AttributeOp::Suffix, Some(v)) => format!("{base}[{attr}$=\"{v}\"]"),
                (AttributeOp::Contains, Some(v)) => format!("{base}[{attr}*=\"{v}\"]"),
                (_, None) => format!("{base}[{attr}]"),
            }
        }
    }
}

fn apply_property(
    style: &mut Style,
    mask: &mut StyleMask,
    name: &str,
    value: &CssValue,
    vw: f32,
    vh: f32,
) {
    match name {
        "display" => {
            if let Some(v) = to_display(value) {
                style.display = v;
                mask.display = true;
            }
        }
        "flex-direction" => {
            if let Some(v) = to_flex_direction(value) {
                style.flex_direction = v;
                mask.flex_direction = true;
            }
        }
        "flex-wrap" => {
            if let Some(v) = to_flex_wrap(value) {
                style.flex_wrap = v;
                mask.flex_wrap = true;
            }
        }
        "justify-content" => {
            if let Some(v) = to_justify_content(value) {
                style.justify_content = Some(v);
                mask.justify_content = true;
            }
        }
        "align-items" => {
            if let Some(v) = to_align_items(value) {
                style.align_items = Some(v);
                mask.align_items = true;
            }
        }
        "align-self" => {
            if let Some(v) = to_align_items(value) {
                style.align_self = Some(v);
                mask.align_self = true;
            }
        }
        "flex-grow" => {
            if let CssValue::Length(n, _) = value {
                style.flex_grow = *n;
                mask.flex_grow = true;
            }
        }
        "flex-shrink" => {
            if let CssValue::Length(n, _) = value {
                style.flex_shrink = *n;
                mask.flex_shrink = true;
            }
        }
        "gap" => {
            if let Some(lp) = to_length_percentage(value, vw, vh) {
                style.gap.width = lp;
                style.gap.height = lp;
                mask.gap_width = true;
                mask.gap_height = true;
            }
        }
        "row-gap" => {
            if let Some(lp) = to_length_percentage(value, vw, vh) {
                style.gap.height = lp;
                mask.gap_height = true;
            }
        }
        "column-gap" => {
            if let Some(lp) = to_length_percentage(value, vw, vh) {
                style.gap.width = lp;
                mask.gap_width = true;
            }
        }
        "padding" => {
            apply_rect_lp(&mut style.padding, value, vw, vh);
            mask.padding = true;
        }
        "padding-top" => {
            if let Some(lp) = to_length_percentage(value, vw, vh) {
                style.padding.top = lp;
                mask.padding = true;
            }
        }
        "padding-right" => {
            if let Some(lp) = to_length_percentage(value, vw, vh) {
                style.padding.right = lp;
                mask.padding = true;
            }
        }
        "padding-bottom" => {
            if let Some(lp) = to_length_percentage(value, vw, vh) {
                style.padding.bottom = lp;
                mask.padding = true;
            }
        }
        "padding-left" => {
            if let Some(lp) = to_length_percentage(value, vw, vh) {
                style.padding.left = lp;
                mask.padding = true;
            }
        }
        "margin" => {
            apply_rect_lpa(&mut style.margin, value, vw, vh);
            mask.margin = true;
        }
        "margin-top" => {
            if let Some(v) = to_length_percentage_auto(value, vw, vh) {
                style.margin.top = v;
                mask.margin = true;
            }
        }
        "margin-right" => {
            if let Some(v) = to_length_percentage_auto(value, vw, vh) {
                style.margin.right = v;
                mask.margin = true;
            }
        }
        "margin-bottom" => {
            if let Some(v) = to_length_percentage_auto(value, vw, vh) {
                style.margin.bottom = v;
                mask.margin = true;
            }
        }
        "margin-left" => {
            if let Some(v) = to_length_percentage_auto(value, vw, vh) {
                style.margin.left = v;
                mask.margin = true;
            }
        }
        "width" => {
            if let Some(d) = to_dimension(value, vw, vh) {
                style.size.width = d;
                mask.width = true;
            }
        }
        "height" => {
            if let Some(d) = to_dimension(value, vw, vh) {
                style.size.height = d;
                mask.height = true;
            }
        }
        "min-width" => {
            if let Some(v) = to_length_percentage_auto(value, vw, vh) {
                style.min_size.width = v;
                mask.min_width = true;
            }
        }
        "min-height" => {
            if let Some(v) = to_length_percentage_auto(value, vw, vh) {
                style.min_size.height = v;
                mask.min_height = true;
            }
        }
        "max-width" => {
            if let Some(v) = to_length_percentage_auto(value, vw, vh) {
                style.max_size.width = v;
                mask.max_width = true;
            }
        }
        "max-height" => {
            if let Some(v) = to_length_percentage_auto(value, vw, vh) {
                style.max_size.height = v;
                mask.max_height = true;
            }
        }
        "position" => {
            if let Some(v) = to_position(value) {
                style.position = v;
                mask.position = true;
            }
        }
        "top" => {
            if let Some(v) = to_length_percentage_auto(value, vw, vh) {
                style.inset.top = v;
                mask.inset = true;
            }
        }
        "right" => {
            if let Some(v) = to_length_percentage_auto(value, vw, vh) {
                style.inset.right = v;
                mask.inset = true;
            }
        }
        "bottom" => {
            if let Some(v) = to_length_percentage_auto(value, vw, vh) {
                style.inset.bottom = v;
                mask.inset = true;
            }
        }
        "left" => {
            if let Some(v) = to_length_percentage_auto(value, vw, vh) {
                style.inset.left = v;
                mask.inset = true;
            }
        }
        "overflow" => {
            if let Some(v) = to_overflow(value) {
                style.overflow = Point { x: v, y: v };
                mask.overflow = true;
            }
        }
        "border-radius" => {
            if let Some(lp) = to_length_percentage(value, vw, vh) {
                style.border.top = lp;
                style.border.right = lp;
                style.border.bottom = lp;
                style.border.left = lp;
                mask.border = true;
            }
        }
        "border-width" => {
            if let Some(lp) = to_length_percentage(value, vw, vh) {
                style.border.top = lp;
                style.border.right = lp;
                style.border.bottom = lp;
                style.border.left = lp;
                mask.border = true;
            }
        }
        _ => {}
    }
}

fn to_display(val: &CssValue) -> Option<Display> {
    match val {
        CssValue::Keyword(k) => match k.as_str() {
            "flex" => Some(Display::Flex),
            "grid" => Some(Display::Grid),
            "none" => Some(Display::None),
            _ => None,
        },
        _ => None,
    }
}

fn to_flex_direction(val: &CssValue) -> Option<FlexDirection> {
    match val {
        CssValue::Keyword(k) => match k.as_str() {
            "row" => Some(FlexDirection::Row),
            "row-reverse" => Some(FlexDirection::RowReverse),
            "column" => Some(FlexDirection::Column),
            "column-reverse" => Some(FlexDirection::ColumnReverse),
            _ => None,
        },
        _ => None,
    }
}

fn to_flex_wrap(val: &CssValue) -> Option<FlexWrap> {
    match val {
        CssValue::Keyword(k) => match k.as_str() {
            "nowrap" => Some(FlexWrap::NoWrap),
            "wrap" => Some(FlexWrap::Wrap),
            "wrap-reverse" => Some(FlexWrap::WrapReverse),
            _ => None,
        },
        _ => None,
    }
}

fn to_justify_content(val: &CssValue) -> Option<JustifyContent> {
    match val {
        CssValue::Keyword(k) => match k.as_str() {
            "flex-start" | "start" => Some(JustifyContent::FLEX_START),
            "flex-end" | "end" => Some(JustifyContent::FLEX_END),
            "center" => Some(JustifyContent::CENTER),
            "space-between" => Some(JustifyContent::SPACE_BETWEEN),
            "space-around" => Some(JustifyContent::SPACE_AROUND),
            "space-evenly" => Some(JustifyContent::SPACE_EVENLY),
            _ => None,
        },
        _ => None,
    }
}

fn to_align_items(val: &CssValue) -> Option<AlignItems> {
    match val {
        CssValue::Keyword(k) => match k.as_str() {
            "flex-start" | "start" => Some(AlignItems::FLEX_START),
            "flex-end" | "end" => Some(AlignItems::FLEX_END),
            "center" => Some(AlignItems::CENTER),
            "stretch" => Some(AlignItems::STRETCH),
            "baseline" => Some(AlignItems::BASELINE),
            _ => None,
        },
        _ => None,
    }
}

fn to_position(val: &CssValue) -> Option<Position> {
    match val {
        CssValue::Keyword(k) => match k.as_str() {
            "relative" => Some(Position::Relative),
            "absolute" => Some(Position::Absolute),
            _ => None,
        },
        _ => None,
    }
}

fn to_overflow(val: &CssValue) -> Option<Overflow> {
    match val {
        CssValue::Keyword(k) => match k.as_str() {
            "hidden" => Some(Overflow::Hidden),
            "scroll" => Some(Overflow::Scroll),
            "visible" => Some(Overflow::Visible),
            "clip" => Some(Overflow::Clip),
            _ => None,
        },
        _ => None,
    }
}

fn to_length_percentage(val: &CssValue, vw: f32, vh: f32) -> Option<LengthPercentage> {
    match val {
        CssValue::Length(n, unit) => match unit {
            // Viewport units resolve to absolute pixels against the given
            // viewport, so nested elements no longer mis-resolve `50vw` against
            // their parent. `%` stays parent-relative, as CSS requires.
            LengthUnit::Vw => Some(LengthPercentage::length(*n / 100.0 * vw)),
            LengthUnit::Vh => Some(LengthPercentage::length(*n / 100.0 * vh)),
            LengthUnit::Percent => Some(LengthPercentage::percent(*n / 100.0)),
            _ => Some(LengthPercentage::length(*n)),
        },
        _ => None,
    }
}

fn to_length_percentage_auto(val: &CssValue, vw: f32, vh: f32) -> Option<LengthPercentageAuto> {
    match val {
        CssValue::Auto => Some(LengthPercentageAuto::auto()),
        CssValue::Length(n, unit) => match unit {
            LengthUnit::Vw => Some(LengthPercentageAuto::length(*n / 100.0 * vw)),
            LengthUnit::Vh => Some(LengthPercentageAuto::length(*n / 100.0 * vh)),
            LengthUnit::Percent => Some(LengthPercentageAuto::percent(*n / 100.0)),
            _ => Some(LengthPercentageAuto::length(*n)),
        },
        _ => None,
    }
}

fn to_dimension(val: &CssValue, vw: f32, vh: f32) -> Option<Dimension> {
    match val {
        CssValue::Auto => Some(Dimension::auto()),
        CssValue::Length(n, unit) => match unit {
            LengthUnit::Vw => Some(Dimension::length(*n / 100.0 * vw)),
            LengthUnit::Vh => Some(Dimension::length(*n / 100.0 * vh)),
            LengthUnit::Percent => Some(Dimension::percent(*n / 100.0)),
            _ => Some(Dimension::length(*n)),
        },
        CssValue::Keyword(k) if k == "auto" => Some(Dimension::auto()),
        _ => None,
    }
}

fn apply_rect_lp(target: &mut Rect<LengthPercentage>, value: &CssValue, vw: f32, vh: f32) {
    match value {
        CssValue::Shorthand(parts) => {
            let vals: Vec<LengthPercentage> = parts
                .iter()
                .filter_map(|p| to_length_percentage(p, vw, vh))
                .collect();
            match vals.len() {
                1 => {
                    *target = Rect {
                        top: vals[0],
                        right: vals[0],
                        bottom: vals[0],
                        left: vals[0],
                    }
                }
                2 => {
                    *target = Rect {
                        top: vals[0],
                        right: vals[1],
                        bottom: vals[0],
                        left: vals[1],
                    }
                }
                3 => {
                    *target = Rect {
                        top: vals[0],
                        right: vals[1],
                        bottom: vals[2],
                        left: vals[1],
                    }
                }
                4 => {
                    *target = Rect {
                        top: vals[0],
                        right: vals[1],
                        bottom: vals[2],
                        left: vals[3],
                    }
                }
                _ => {}
            }
        }
        _ => {
            if let Some(lp) = to_length_percentage(value, vw, vh) {
                *target = Rect {
                    top: lp,
                    right: lp,
                    bottom: lp,
                    left: lp,
                };
            }
        }
    }
}

fn apply_rect_lpa(target: &mut Rect<LengthPercentageAuto>, value: &CssValue, vw: f32, vh: f32) {
    match value {
        CssValue::Shorthand(parts) => {
            let vals: Vec<LengthPercentageAuto> = parts
                .iter()
                .filter_map(|p| to_length_percentage_auto(p, vw, vh))
                .collect();
            match vals.len() {
                1 => {
                    *target = Rect {
                        top: vals[0],
                        right: vals[0],
                        bottom: vals[0],
                        left: vals[0],
                    }
                }
                2 => {
                    *target = Rect {
                        top: vals[0],
                        right: vals[1],
                        bottom: vals[0],
                        left: vals[1],
                    }
                }
                3 => {
                    *target = Rect {
                        top: vals[0],
                        right: vals[1],
                        bottom: vals[2],
                        left: vals[1],
                    }
                }
                4 => {
                    *target = Rect {
                        top: vals[0],
                        right: vals[1],
                        bottom: vals[2],
                        left: vals[3],
                    }
                }
                _ => {}
            }
        }
        _ => {
            if let Some(v) = to_length_percentage_auto(value, vw, vh) {
                *target = Rect {
                    top: v,
                    right: v,
                    bottom: v,
                    left: v,
                };
            }
        }
    }
}

/// Generate Rust source code for Taffy styles
pub fn generate_taffy_styles(rules: &[CssRule]) -> Result<String> {
    let mut output = String::new();
    output.push_str("use taffy::prelude::*;\n\n");

    for rule in rules {
        let fn_name = selector_to_fn_name(&rule.selector);
        output.push_str(&format!("fn {}() -> Style {{\n", fn_name));
        output.push_str("    Style::default()\n");

        for prop in &rule.properties {
            if let Some(style_code) = generate_style_property(prop) {
                output.push_str(&format!("        .{}()\n", style_code));
            }
        }

        output.push_str("}\n\n");
    }

    Ok(output)
}

fn selector_to_fn_name(selector: &CssSelector) -> String {
    match selector {
        CssSelector::Class(name) => format!("style_{}", name.replace('-', "_")),
        CssSelector::Id(name) => format!("style_{}", name.replace('-', "_")),
        CssSelector::Tag(name) => format!("style_{}", name),
        CssSelector::Universal => "style_universal".to_string(),
        CssSelector::Descendant(_) => "style_descendant".to_string(),
        CssSelector::Child(_) => "style_child".to_string(),
        CssSelector::List(_) => "style_list".to_string(),
        CssSelector::PseudoClass(inner, pseudo) => {
            format!(
                "{}_{}",
                selector_to_fn_name(inner),
                pseudo.replace('-', "_")
            )
        }
        CssSelector::Attribute { selector, attr, .. } => format!(
            "{}_attr_{}",
            selector_to_fn_name(selector),
            attr.replace('-', "_")
        ),
    }
}

fn generate_style_property(prop: &CssProperty) -> Option<String> {
    match prop.name.as_str() {
        "display" => generate_display(&prop.value),
        "flex-direction" => generate_flex_direction(&prop.value),
        "flex-wrap" => generate_flex_wrap(&prop.value),
        "justify-content" => generate_justify_content(&prop.value),
        "align-items" => generate_align_items(&prop.value),
        "position" => generate_position(&prop.value),
        "overflow" => generate_overflow(&prop.value),
        "padding" => generate_length_prop("padding", &prop.value),
        "margin" => generate_length_prop("margin", &prop.value),
        "gap" => generate_length_prop("gap", &prop.value),
        "width" => generate_length_prop("width", &prop.value),
        "height" => generate_length_prop("height", &prop.value),
        "border-radius" => generate_length_prop("border_radius", &prop.value),
        "border-width" => generate_length_prop("border_width", &prop.value),
        _ => None,
    }
}

fn generate_display(val: &CssValue) -> Option<String> {
    match val {
        CssValue::Keyword(k) => match k.as_str() {
            "flex" => Some("display(Display::Flex)".to_string()),
            "grid" => Some("display(Display::Grid)".to_string()),
            "none" => Some("display(Display::None)".to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn generate_flex_direction(val: &CssValue) -> Option<String> {
    match val {
        CssValue::Keyword(k) => match k.as_str() {
            "row" => Some("flex_direction(FlexDirection::Row)".to_string()),
            "row-reverse" => Some("flex_direction(FlexDirection::RowReverse)".to_string()),
            "column" => Some("flex_direction(FlexDirection::Column)".to_string()),
            "column-reverse" => Some("flex_direction(FlexDirection::ColumnReverse)".to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn generate_flex_wrap(val: &CssValue) -> Option<String> {
    match val {
        CssValue::Keyword(k) => match k.as_str() {
            "nowrap" => Some("flex_wrap(FlexWrap::NoWrap)".to_string()),
            "wrap" => Some("flex_wrap(FlexWrap::Wrap)".to_string()),
            "wrap-reverse" => Some("flex_wrap(FlexWrap::WrapReverse)".to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn generate_justify_content(val: &CssValue) -> Option<String> {
    match val {
        CssValue::Keyword(k) => match k.as_str() {
            "flex-start" => Some("justify_content(JustifyContent::FLEX_START)".to_string()),
            "flex-end" => Some("justify_content(JustifyContent::FLEX_END)".to_string()),
            "center" => Some("justify_content(JustifyContent::CENTER)".to_string()),
            "space-between" => Some("justify_content(JustifyContent::SPACE_BETWEEN)".to_string()),
            "space-around" => Some("justify_content(JustifyContent::SPACE_AROUND)".to_string()),
            "space-evenly" => Some("justify_content(JustifyContent::SPACE_EVENLY)".to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn generate_align_items(val: &CssValue) -> Option<String> {
    match val {
        CssValue::Keyword(k) => match k.as_str() {
            "flex-start" => Some("align_items(AlignItems::FLEX_START)".to_string()),
            "flex-end" => Some("align_items(AlignItems::FLEX_END)".to_string()),
            "center" => Some("align_items(AlignItems::CENTER)".to_string()),
            "stretch" => Some("align_items(AlignItems::STRETCH)".to_string()),
            "baseline" => Some("align_items(AlignItems::BASELINE)".to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn generate_position(val: &CssValue) -> Option<String> {
    match val {
        CssValue::Keyword(k) => match k.as_str() {
            "relative" => Some("position(Position::Relative)".to_string()),
            "absolute" => Some("position(Position::Absolute)".to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn generate_overflow(val: &CssValue) -> Option<String> {
    match val {
        CssValue::Keyword(k) => match k.as_str() {
            "hidden" => {
                Some("overflow(Point { x: Overflow::Hidden, y: Overflow::Hidden })".to_string())
            }
            "scroll" => {
                Some("overflow(Point { x: Overflow::Scroll, y: Overflow::Scroll })".to_string())
            }
            "visible" => {
                Some("overflow(Point { x: Overflow::Visible, y: Overflow::Visible })".to_string())
            }
            _ => None,
        },
        _ => None,
    }
}

fn generate_length_prop(prop: &str, val: &CssValue) -> Option<String> {
    match val {
        CssValue::Length(n, _) => Some(format!("{}(LengthPercentage::length({:.1}))", prop, n)),
        CssValue::Auto => Some(format!("{}(LengthPercentageAuto::auto())", prop)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_css;

    #[test]
    fn test_generate_flex_style() {
        let css = ".container { display: flex; padding: 16px; }";
        let rules = parse_css(css).unwrap();
        let code = generate_taffy_styles(&rules).unwrap();
        assert!(code.contains("Display::Flex"));
        assert!(code.contains("padding"));
    }

    #[test]
    fn test_convert_to_taffy_styles() {
        let css = ".container { display: flex; padding: 16px; gap: 8px; }";
        let rules = parse_css(css).unwrap();
        let styles = convert_to_taffy_styles(&rules).unwrap();
        assert_eq!(styles.len(), 1);
        assert_eq!(styles[0].0, ".container");
        assert_eq!(styles[0].1.display, Display::Flex);
    }

    #[test]
    fn test_flex_direction_conversion() {
        let css = ".col { flex-direction: column; }";
        let rules = parse_css(css).unwrap();
        let styles = convert_to_taffy_styles(&rules).unwrap();
        assert_eq!(styles[0].1.flex_direction, FlexDirection::Column);
    }

    #[test]
    fn test_justify_content_conversion() {
        let css = ".center { justify-content: center; }";
        let rules = parse_css(css).unwrap();
        let styles = convert_to_taffy_styles(&rules).unwrap();
        assert_eq!(styles[0].1.justify_content, Some(JustifyContent::CENTER));
    }

    #[test]
    fn test_align_items_conversion() {
        let css = ".stretch { align-items: stretch; }";
        let rules = parse_css(css).unwrap();
        let styles = convert_to_taffy_styles(&rules).unwrap();
        assert_eq!(styles[0].1.align_items, Some(AlignItems::STRETCH));
    }

    #[test]
    fn test_gap_conversion() {
        let css = ".grid { gap: 16px; }";
        let rules = parse_css(css).unwrap();
        let styles = convert_to_taffy_styles(&rules).unwrap();
        let expected = LengthPercentage::length(16.0);
        assert_eq!(styles[0].1.gap.width, expected);
    }

    #[test]
    fn test_padding_shorthand() {
        let css = ".box { padding: 10px 20px; }";
        let rules = parse_css(css).unwrap();
        let styles = convert_to_taffy_styles(&rules).unwrap();
        assert_eq!(styles[0].1.padding.top, LengthPercentage::length(10.0));
        assert_eq!(styles[0].1.padding.right, LengthPercentage::length(20.0));
    }

    #[test]
    fn test_margin_auto() {
        let css = ".box { margin: auto; }";
        let rules = parse_css(css).unwrap();
        let styles = convert_to_taffy_styles(&rules).unwrap();
        assert!(styles[0].1.margin.top.is_auto());
    }

    #[test]
    fn test_position_conversion() {
        let css = ".abs { position: absolute; }";
        let rules = parse_css(css).unwrap();
        let styles = convert_to_taffy_styles(&rules).unwrap();
        assert_eq!(styles[0].1.position, Position::Absolute);
    }

    #[test]
    fn test_width_height_conversion() {
        let css = ".size { width: 100px; height: 50vh; }";
        let rules = parse_css(css).unwrap();
        let styles = convert_to_taffy_styles(&rules).unwrap();
        assert_eq!(styles[0].1.size.width, Dimension::length(100.0));
    }

    #[test]
    fn test_overflow_conversion() {
        let css = ".scroll { overflow: hidden; }";
        let rules = parse_css(css).unwrap();
        let styles = convert_to_taffy_styles(&rules).unwrap();
        assert_eq!(styles[0].1.overflow.x, Overflow::Hidden);
    }

    #[test]
    fn test_border_width_conversion() {
        let css = ".border { border-width: 2px; }";
        let rules = parse_css(css).unwrap();
        let styles = convert_to_taffy_styles(&rules).unwrap();
        assert_eq!(styles[0].1.border.top, LengthPercentage::length(2.0));
    }

    #[test]
    fn test_multiple_rules_conversion() {
        let css = ".a { display: flex; } .b { display: grid; }";
        let rules = parse_css(css).unwrap();
        let styles = convert_to_taffy_styles(&rules).unwrap();
        assert_eq!(styles.len(), 2);
        assert_eq!(styles[0].1.display, Display::Flex);
        assert_eq!(styles[1].1.display, Display::Grid);
    }

    // ── StyleMask ───────────────────────────────────────────────

    #[test]
    fn test_mask_tracks_only_specified_properties() {
        let css = ".only-width { width: 100px; }";
        let rules = parse_css(css).unwrap();
        let entries = convert_to_style_entries(&rules).unwrap();
        let mask = entries[0].mask;
        assert!(mask.width, "width should be marked as specified");
        assert!(!mask.display, "display was never specified");
        assert!(!mask.flex_direction);
        assert!(!mask.padding);
    }

    #[test]
    fn test_mask_multiple_properties() {
        let css = ".card { display: flex; flex-direction: column; padding: 8px; }";
        let rules = parse_css(css).unwrap();
        let entries = convert_to_style_entries(&rules).unwrap();
        let mask = entries[0].mask;
        assert!(mask.display);
        assert!(mask.flex_direction);
        assert!(mask.padding);
        assert!(!mask.width);
        assert!(!mask.margin);
    }

    #[test]
    fn test_mask_empty_for_paint_only_rule() {
        let css = ".text { color: red; }";
        let rules = parse_css(css).unwrap();
        let entries = convert_to_style_entries(&rules).unwrap();
        assert!(
            entries[0].mask.is_empty(),
            "color is paint-only, no layout field set"
        );
    }

    #[test]
    fn test_mask_or_assign() {
        let mut a = StyleMask {
            width: true,
            ..Default::default()
        };
        let b = StyleMask {
            display: true,
            ..Default::default()
        };
        a.or_assign(&b);
        assert!(a.width);
        assert!(a.display);
    }

    #[test]
    fn test_gap_shorthand_marks_both_axes() {
        let css = ".g { gap: 4px; }";
        let rules = parse_css(css).unwrap();
        let entries = convert_to_style_entries(&rules).unwrap();
        assert!(entries[0].mask.gap_width);
        assert!(entries[0].mask.gap_height);
    }

    // ── PaintProps ──────────────────────────────────────────────

    #[test]
    fn test_paint_background_color() {
        let css = ".app { background-color: #1a1a2e; }";
        let rules = parse_css(css).unwrap();
        let entries = convert_to_style_entries(&rules).unwrap();
        match entries[0].paint.background.clone().unwrap() {
            BackgroundValue::Solid(bg) => assert_eq!((bg.r, bg.g, bg.b), (0x1a, 0x1a, 0x2e)),
            other => panic!("expected solid background, got {other:?}"),
        }
    }

    #[test]
    fn test_paint_text_color() {
        let css = ".app { color: #e0e0e0; }";
        let rules = parse_css(css).unwrap();
        let entries = convert_to_style_entries(&rules).unwrap();
        let c = entries[0].paint.color.clone().unwrap();
        assert_eq!((c.r, c.g, c.b), (0xe0, 0xe0, 0xe0));
    }

    #[test]
    fn test_paint_font_size_px() {
        let css = "h1 { font-size: 32px; }";
        let rules = parse_css(css).unwrap();
        let entries = convert_to_style_entries(&rules).unwrap();
        assert_eq!(entries[0].paint.font_size, Some(32.0));
    }

    #[test]
    fn test_paint_font_size_rem_resolves_to_px() {
        let css = "h1 { font-size: 2rem; }";
        let rules = parse_css(css).unwrap();
        let entries = convert_to_style_entries(&rules).unwrap();
        assert_eq!(entries[0].paint.font_size, Some(32.0));
    }

    #[test]
    fn test_paint_font_family() {
        let css = ".app { font-family: monospace; }";
        let rules = parse_css(css).unwrap();
        let entries = convert_to_style_entries(&rules).unwrap();
        assert_eq!(entries[0].paint.font_family.as_deref(), Some("monospace"));
    }

    #[test]
    fn test_paint_empty_for_layout_only_rule() {
        let css = ".row { display: flex; }";
        let rules = parse_css(css).unwrap();
        let entries = convert_to_style_entries(&rules).unwrap();
        assert!(entries[0].paint.is_empty());
    }

    #[test]
    fn test_paint_merge_keeps_unspecified() {
        let mut base = PaintProps {
            background: Some(BackgroundValue::Solid(Color::rgb(1, 2, 3))),
            font_size: Some(16.0),
            ..Default::default()
        };
        let over = PaintProps {
            font_size: Some(24.0),
            ..Default::default()
        };
        base.merge(&over);
        assert_eq!(base.font_size, Some(24.0), "specified field overwritten");
        assert_eq!(
            base.background,
            Some(BackgroundValue::Solid(Color::rgb(1, 2, 3))),
            "unspecified field preserved"
        );
    }

    #[test]
    fn test_paint_border_and_opacity() {
        let css = ".b { border-width: 2px; border-radius: 6px; border-color: blue; opacity: 0.5; }";
        let rules = parse_css(css).unwrap();
        let entries = convert_to_style_entries(&rules).unwrap();
        let paint = &entries[0].paint;
        assert_eq!(paint.border_width, Some(2.0));
        assert_eq!(paint.border_radius, Some(6.0));
        assert_eq!(paint.opacity, Some(0.5));
        assert!(paint.border_color.is_some());
    }

    #[test]
    fn test_paint_text_overflow_ellipsis() {
        let css = ".t { text-overflow: ellipsis; }";
        let rules = parse_css(css).unwrap();
        let entries = convert_to_style_entries(&rules).unwrap();
        assert_eq!(entries[0].paint.text_overflow.as_deref(), Some("ellipsis"));
    }

    #[test]
    fn test_paint_text_overflow_clip() {
        let css = ".t { text-overflow: clip; }";
        let rules = parse_css(css).unwrap();
        let entries = convert_to_style_entries(&rules).unwrap();
        assert_eq!(entries[0].paint.text_overflow.as_deref(), Some("clip"));
    }

    #[test]
    fn test_convert_to_taffy_styles_still_compatible() {
        // The legacy (String, Style) API must keep working for existing callers.
        let css = ".container { display: flex; padding: 16px; }";
        let rules = parse_css(css).unwrap();
        let legacy = convert_to_taffy_styles(&rules).unwrap();
        let entries = convert_to_style_entries(&rules).unwrap();
        assert_eq!(legacy.len(), entries.len());
        assert_eq!(legacy[0].0, entries[0].selector);
        assert_eq!(legacy[0].1.display, entries[0].style.display);
    }

    // ── Viewport units (vw/vh) ──────────────────────────────────

    #[test]
    fn test_vw_resolves_to_pixels_against_viewport() {
        let css = ".w { width: 50vw; }";
        let rules = parse_css(css).unwrap();
        let entries = convert_to_style_entries_vp(&rules, 800.0, 600.0).unwrap();
        // 50vw of an 800px viewport = 400px, an absolute length not a percent.
        assert_eq!(entries[0].style.size.width, Dimension::length(400.0));
    }

    #[test]
    fn test_vh_resolves_to_pixels_against_viewport() {
        let css = ".h { height: 50vh; }";
        let rules = parse_css(css).unwrap();
        let entries = convert_to_style_entries_vp(&rules, 800.0, 600.0).unwrap();
        assert_eq!(entries[0].style.size.height, Dimension::length(300.0));
    }

    #[test]
    fn test_percent_stays_parent_relative() {
        let css = ".w { width: 50%; }";
        let rules = parse_css(css).unwrap();
        let entries = convert_to_style_entries_vp(&rules, 800.0, 600.0).unwrap();
        // `%` must remain a percentage, resolved against the parent by taffy.
        assert_eq!(entries[0].style.size.width, Dimension::percent(0.5));
    }

    #[test]
    fn test_vw_in_padding_resolves_to_length() {
        let css = ".p { padding: 10vw; }";
        let rules = parse_css(css).unwrap();
        let entries = convert_to_style_entries_vp(&rules, 1000.0, 500.0).unwrap();
        assert_eq!(
            entries[0].style.padding.top,
            LengthPercentage::length(100.0)
        );
    }
}
