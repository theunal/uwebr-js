use crate::ast::*;
use anyhow::Result;

/// Generate Taffy Style Rust code from CssRule list
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
        CssSelector::Class(name) => {
            format!("style_{}", name.replace('-', "_"))
        }
        CssSelector::Id(name) => {
            format!("style_{}", name.replace('-', "_"))
        }
        CssSelector::Tag(name) => {
            format!("style_{}", name)
        }
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
        "align-self" => generate_align_self(&prop.value),
        "gap" => Some(format!("gap(LengthPercentage::Length({}))", generate_length(&prop.value)?)),
        "padding" => Some(format!("padding({})", generate_val(&prop.value)?)),
        "margin" => Some(format!("margin({})", generate_val(&prop.value)?)),
        "width" => Some(format!("width({})", generate_val(&prop.value)?)),
        "height" => Some(format!("height({})", generate_val(&prop.value)?)),
        "min-width" => Some(format!("min_width({})", generate_val(&prop.value)?)),
        "min-height" => Some(format!("min_height({})", generate_val(&prop.value)?)),
        "max-width" => Some(format!("max_width({})", generate_val(&prop.value)?)),
        "max-height" => Some(format!("max_height({})", generate_val(&prop.value)?)),
        "position" => generate_position(&prop.value),
        "top" => Some(format!("top({})", generate_val(&prop.value)?)),
        "right" => Some(format!("right({})", generate_val(&prop.value)?)),
        "bottom" => Some(format!("bottom({})", generate_val(&prop.value)?)),
        "left" => Some(format!("left({})", generate_val(&prop.value)?)),
        "overflow" => generate_overflow(&prop.value),
        "border-radius" => Some(format!("border_radius({})", generate_val(&prop.value)?)),
        "border-width" => Some(format!("border_width({})", generate_val(&prop.value)?)),
        _ => None,
    }
}

fn generate_display(val: &CssValue) -> Option<String> {
    match val {
        CssValue::Keyword(k) => match k.as_str() {
            "flex" => Some("display(Display::Flex)".to_string()),
            "block" => Some("display(Display::Block)".to_string()),
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
            "column-reverse" => {
                Some("flex_direction(FlexDirection::ColumnReverse)".to_string())
            }
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
            "flex-start" => {
                Some("justify_content(JustifyContent::FlexStart)".to_string())
            }
            "flex-end" => Some("justify_content(JustifyContent::FlexEnd)".to_string()),
            "center" => Some("justify_content(JustifyContent::Center)".to_string()),
            "space-between" => {
                Some("justify_content(JustifyContent::SpaceBetween)".to_string())
            }
            "space-around" => {
                Some("justify_content(JustifyContent::SpaceAround)".to_string())
            }
            "space-evenly" => {
                Some("justify_content(JustifyContent::SpaceEvenly)".to_string())
            }
            _ => None,
        },
        _ => None,
    }
}

fn generate_align_items(val: &CssValue) -> Option<String> {
    match val {
        CssValue::Keyword(k) => match k.as_str() {
            "flex-start" => Some("align_items(AlignItems::FlexStart)".to_string()),
            "flex-end" => Some("align_items(AlignItems::FlexEnd)".to_string()),
            "center" => Some("align_items(AlignItems::Center)".to_string()),
            "stretch" => Some("align_items(AlignItems::Stretch)".to_string()),
            "baseline" => Some("align_items(AlignItems::Baseline)".to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn generate_align_self(val: &CssValue) -> Option<String> {
    match val {
        CssValue::Keyword(k) => match k.as_str() {
            "flex-start" => Some("align_self(AlignSelf::FlexStart)".to_string()),
            "flex-end" => Some("align_self(AlignSelf::FlexEnd)".to_string()),
            "center" => Some("align_self(AlignSelf::Center)".to_string()),
            "stretch" => Some("align_self(AlignSelf::Stretch)".to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn generate_position(val: &CssValue) -> Option<String> {
    match val {
        CssValue::Keyword(k) => match k.as_str() {
            "relative" => Some("position(PositionType::Relative)".to_string()),
            "absolute" => Some("position(PositionType::Absolute)".to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn generate_overflow(val: &CssValue) -> Option<String> {
    match val {
        CssValue::Keyword(k) => match k.as_str() {
            "hidden" => Some("overflow(Overflow::Hidden)".to_string()),
            "scroll" => Some("overflow(Overflow::Scroll)".to_string()),
            "visible" => Some("overflow(Overflow::Visible)".to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn generate_length(val: &CssValue) -> Option<String> {
    match val {
        CssValue::Length(n, unit) => match unit {
            LengthUnit::Px => Some(format!("Val::Px({:.1})", n)),
            LengthUnit::Em => Some(format!("Val::Em({:.1})", n)),
            LengthUnit::Rem => Some(format!("Val::Rem({:.1})", n)),
            LengthUnit::Percent => Some(format!("Val::Percent({:.1})", n)),
            LengthUnit::Auto => Some("Val::Auto".to_string()),
            _ => Some(format!("Val::Px({:.1})", n)),
        },
        CssValue::Auto => Some("Val::Auto".to_string()),
        _ => Some("Val::Px(0.0)".to_string()),
    }
}

fn generate_val(val: &CssValue) -> Option<String> {
    match val {
        CssValue::Length(_, _) => Some(format!(
            "Length::Length({})",
            generate_length(val)?
        )),
        CssValue::Auto => Some("Length::Auto".to_string()),
        _ => Some("Length::Length(Val::Px(0.0))".to_string()),
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
}
