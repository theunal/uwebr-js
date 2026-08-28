use crate::ast::*;
use anyhow::Result;
use taffy::geometry::Point;
use taffy::prelude::*;
use taffy::style::Overflow;

/// Convert CssRule list to Vec<(String, Style)> for runtime use
pub fn convert_to_taffy_styles(rules: &[CssRule]) -> Result<Vec<(String, Style)>> {
    let mut styles = Vec::new();

    for rule in rules {
        let key = selector_key(&rule.selector);
        let mut style = Style::default();

        for prop in &rule.properties {
            apply_property(&mut style, &prop.name, &prop.value);
        }

        styles.push((key, style));
    }

    Ok(styles)
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
    }
}

fn apply_property(style: &mut Style, name: &str, value: &CssValue) {
    match name {
        "display" => {
            if let Some(v) = to_display(value) {
                style.display = v;
            }
        }
        "flex-direction" => {
            if let Some(v) = to_flex_direction(value) {
                style.flex_direction = v;
            }
        }
        "flex-wrap" => {
            if let Some(v) = to_flex_wrap(value) {
                style.flex_wrap = v;
            }
        }
        "justify-content" => {
            if let Some(v) = to_justify_content(value) {
                style.justify_content = Some(v);
            }
        }
        "align-items" => {
            if let Some(v) = to_align_items(value) {
                style.align_items = Some(v);
            }
        }
        "align-self" => {
            if let Some(v) = to_align_items(value) {
                style.align_self = Some(v);
            }
        }
        "flex-grow" => {
            if let CssValue::Length(n, _) = value {
                style.flex_grow = *n;
            }
        }
        "flex-shrink" => {
            if let CssValue::Length(n, _) = value {
                style.flex_shrink = *n;
            }
        }
        "gap" => {
            if let Some(lp) = to_length_percentage(value) {
                style.gap.width = lp;
                style.gap.height = lp;
            }
        }
        "row-gap" => {
            if let Some(lp) = to_length_percentage(value) {
                style.gap.height = lp;
            }
        }
        "column-gap" => {
            if let Some(lp) = to_length_percentage(value) {
                style.gap.width = lp;
            }
        }
        "padding" => apply_rect_lp(&mut style.padding, value),
        "padding-top" => {
            if let Some(lp) = to_length_percentage(value) {
                style.padding.top = lp;
            }
        }
        "padding-right" => {
            if let Some(lp) = to_length_percentage(value) {
                style.padding.right = lp;
            }
        }
        "padding-bottom" => {
            if let Some(lp) = to_length_percentage(value) {
                style.padding.bottom = lp;
            }
        }
        "padding-left" => {
            if let Some(lp) = to_length_percentage(value) {
                style.padding.left = lp;
            }
        }
        "margin" => apply_rect_lpa(&mut style.margin, value),
        "margin-top" => {
            if let Some(v) = to_length_percentage_auto(value) {
                style.margin.top = v;
            }
        }
        "margin-right" => {
            if let Some(v) = to_length_percentage_auto(value) {
                style.margin.right = v;
            }
        }
        "margin-bottom" => {
            if let Some(v) = to_length_percentage_auto(value) {
                style.margin.bottom = v;
            }
        }
        "margin-left" => {
            if let Some(v) = to_length_percentage_auto(value) {
                style.margin.left = v;
            }
        }
        "width" => {
            if let Some(d) = to_dimension(value) {
                style.size.width = d;
            }
        }
        "height" => {
            if let Some(d) = to_dimension(value) {
                style.size.height = d;
            }
        }
        "min-width" => {
            if let Some(v) = to_length_percentage_auto(value) {
                style.min_size.width = v;
            }
        }
        "min-height" => {
            if let Some(v) = to_length_percentage_auto(value) {
                style.min_size.height = v;
            }
        }
        "max-width" => {
            if let Some(v) = to_length_percentage_auto(value) {
                style.max_size.width = v;
            }
        }
        "max-height" => {
            if let Some(v) = to_length_percentage_auto(value) {
                style.max_size.height = v;
            }
        }
        "position" => {
            if let Some(v) = to_position(value) {
                style.position = v;
            }
        }
        "top" => {
            if let Some(v) = to_length_percentage_auto(value) {
                style.inset.top = v;
            }
        }
        "right" => {
            if let Some(v) = to_length_percentage_auto(value) {
                style.inset.right = v;
            }
        }
        "bottom" => {
            if let Some(v) = to_length_percentage_auto(value) {
                style.inset.bottom = v;
            }
        }
        "left" => {
            if let Some(v) = to_length_percentage_auto(value) {
                style.inset.left = v;
            }
        }
        "overflow" => {
            if let Some(v) = to_overflow(value) {
                style.overflow = Point { x: v, y: v };
            }
        }
        "border-radius" => {
            if let Some(lp) = to_length_percentage(value) {
                style.border.top = lp;
                style.border.right = lp;
                style.border.bottom = lp;
                style.border.left = lp;
            }
        }
        "border-width" => {
            if let Some(lp) = to_length_percentage(value) {
                style.border.top = lp;
                style.border.right = lp;
                style.border.bottom = lp;
                style.border.left = lp;
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

fn to_length_percentage(val: &CssValue) -> Option<LengthPercentage> {
    match val {
        CssValue::Length(n, unit) => match unit {
            LengthUnit::Percent => Some(LengthPercentage::percent(*n / 100.0)),
            _ => Some(LengthPercentage::length(*n)),
        },
        _ => None,
    }
}

fn to_length_percentage_auto(val: &CssValue) -> Option<LengthPercentageAuto> {
    match val {
        CssValue::Auto => Some(LengthPercentageAuto::auto()),
        CssValue::Length(n, unit) => match unit {
            LengthUnit::Percent => Some(LengthPercentageAuto::percent(*n / 100.0)),
            _ => Some(LengthPercentageAuto::length(*n)),
        },
        _ => None,
    }
}

fn to_dimension(val: &CssValue) -> Option<Dimension> {
    match val {
        CssValue::Auto => Some(Dimension::auto()),
        CssValue::Length(n, unit) => match unit {
            LengthUnit::Percent => Some(Dimension::percent(*n / 100.0)),
            _ => Some(Dimension::length(*n)),
        },
        CssValue::Keyword(k) if k == "auto" => Some(Dimension::auto()),
        _ => None,
    }
}

fn apply_rect_lp(target: &mut Rect<LengthPercentage>, value: &CssValue) {
    match value {
        CssValue::Shorthand(parts) => {
            let vals: Vec<LengthPercentage> = parts
                .iter()
                .filter_map(|v| to_length_percentage(v))
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
            if let Some(lp) = to_length_percentage(value) {
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

fn apply_rect_lpa(target: &mut Rect<LengthPercentageAuto>, value: &CssValue) {
    match value {
        CssValue::Shorthand(parts) => {
            let vals: Vec<LengthPercentageAuto> = parts
                .iter()
                .filter_map(|v| to_length_percentage_auto(v))
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
            if let Some(v) = to_length_percentage_auto(value) {
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
}
