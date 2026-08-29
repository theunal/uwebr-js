/// Component trait for UI components
pub trait Component {
    /// Render this component
    fn render(&self) -> Element;
}

/// Element represents a renderable node
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    pub node_type: NodeType,
    pub props: Vec<(String, PropValue)>,
    pub children: Vec<Element>,
}

/// Node types
#[derive(Debug, Clone, PartialEq)]
pub enum NodeType {
    Element(String),
    Text(String),
    Component(String),
    Raw(String),
}

/// Property values
#[derive(Debug, Clone, PartialEq)]
pub enum PropValue {
    String(String),
    Bool(bool),
    Number(f64),
    Closure(String),
}

impl Element {
    pub fn text(content: &str) -> Self {
        Self {
            node_type: NodeType::Text(content.to_string()),
            props: vec![],
            children: vec![],
        }
    }

    pub fn component(name: &str) -> Self {
        Self {
            node_type: NodeType::Component(name.to_string()),
            props: vec![],
            children: vec![],
        }
    }
}

/// Component function type (no props)
pub type ComponentFn = fn() -> Element;

/// Component function type that receives props
pub type PropsComponentFn = fn(&[(String, PropValue)]) -> Element;

/// Read a `String` prop by key. Returns an empty string if absent or not a string.
pub fn prop_string(props: &[(String, PropValue)], key: &str) -> String {
    props
        .iter()
        .find_map(|(k, v)| {
            if k == key {
                match v {
                    PropValue::String(s) => Some(s.clone()),
                    _ => None,
                }
            } else {
                None
            }
        })
        .unwrap_or_default()
}

/// Read a `bool` prop by key. Returns `false` if absent or not a bool.
pub fn prop_bool(props: &[(String, PropValue)], key: &str) -> bool {
    props
        .iter()
        .find_map(|(k, v)| {
            if k == key {
                match v {
                    PropValue::Bool(b) => Some(*b),
                    _ => None,
                }
            } else {
                None
            }
        })
        .unwrap_or(false)
}

/// Read a numeric prop by key. Falls back to parsing a `String` prop, else `0.0`.
pub fn prop_number(props: &[(String, PropValue)], key: &str) -> f64 {
    props
        .iter()
        .find_map(|(k, v)| {
            if k == key {
                match v {
                    PropValue::Number(n) => Some(*n),
                    PropValue::String(s) => s.parse().ok(),
                    _ => None,
                }
            } else {
                None
            }
        })
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_element_text() {
        let el = Element::text("Hello");
        assert!(matches!(el.node_type, NodeType::Text(_)));
    }

    #[test]
    fn test_prop_string() {
        let props = vec![
            ("title".into(), PropValue::String("Hello".into())),
            ("disabled".into(), PropValue::Bool(true)),
        ];
        assert_eq!(prop_string(&props, "title"), "Hello");
        assert_eq!(prop_string(&props, "missing"), "");
        // Wrong type is not coerced.
        assert_eq!(prop_string(&props, "disabled"), "");
    }

    #[test]
    fn test_prop_bool() {
        let props = vec![("disabled".into(), PropValue::Bool(true))];
        assert!(prop_bool(&props, "disabled"));
        assert!(!prop_bool(&props, "missing"));
    }

    #[test]
    fn test_prop_number() {
        let props = vec![
            ("count".into(), PropValue::Number(42.0)),
            ("size".into(), PropValue::String("7".into())),
        ];
        assert_eq!(prop_number(&props, "count"), 42.0);
        // String props are parsed as a fallback.
        assert_eq!(prop_number(&props, "size"), 7.0);
        assert_eq!(prop_number(&props, "missing"), 0.0);
    }
}
