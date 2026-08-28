/// Component trait for UI components
pub trait Component {
    /// Render this component
    fn render(&self) -> Element;
}

/// Element represents a renderable node
#[derive(Debug, Clone)]
pub struct Element {
    pub node_type: NodeType,
    pub props: Vec<(String, PropValue)>,
    pub children: Vec<Element>,
}

/// Node types
#[derive(Debug, Clone)]
pub enum NodeType {
    Text(String),
    Component(String),
    Raw(String),
}

/// Property values
#[derive(Debug, Clone)]
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

/// Component function type
pub type ComponentFn = fn() -> Element;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_element_text() {
        let el = Element::text("Hello");
        assert!(matches!(el.node_type, NodeType::Text(_)));
    }
}
