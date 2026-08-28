use serde::{Deserialize, Serialize};

/// Root HTML node
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HtmlNode {
    Element(HtmlElement),
    Text(String),
    Expression(String),
    RawHtml(String),
    Component(HtmlComponent),
    EachLoop(HtmlEach),
    IfBlock(HtmlIf),
    Fragment(Vec<HtmlNode>),
    Comment(String),
}

/// HTML element: <tag attr="value">children</tag>
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HtmlElement {
    pub tag: String,
    pub attributes: Vec<HtmlAttribute>,
    pub children: Vec<HtmlNode>,
    pub self_closing: bool,
}

/// HTML attribute: name="value" or name={expr}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HtmlAttribute {
    pub name: String,
    pub value: HtmlAttributeValue,
}

/// Attribute value types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HtmlAttributeValue {
    /// Literal string: "hello"
    Literal(String),
    /// Expression: {variable}
    Expression(String),
    /// Boolean attribute: disabled
    Boolean(bool),
    /// Shorthand: {name} → name={name}
    Shorthand(String),
    /// Class list: class={active ? "active" : ""}
    Conditional(String, String, String),
}

/// Component: <Component props />
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HtmlComponent {
    pub name: String,
    pub attributes: Vec<HtmlAttribute>,
    pub children: Vec<HtmlNode>,
}

/// Each loop: {#each items as item}...{/each}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HtmlEach {
    pub iterable: String,
    pub item_name: String,
    pub index_name: Option<String>,
    pub body: Vec<HtmlNode>,
}

/// If block: {#if condition}...{:else}...{/if}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HtmlIf {
    pub condition: String,
    pub then_body: Vec<HtmlNode>,
    pub else_body: Option<Vec<HtmlNode>>,
}

impl HtmlNode {
    pub fn element(tag: &str) -> Self {
        HtmlNode::Element(HtmlElement {
            tag: tag.to_string(),
            attributes: vec![],
            children: vec![],
            self_closing: false,
        })
    }

    pub fn text(content: &str) -> Self {
        HtmlNode::Text(content.to_string())
    }

    pub fn expression(expr: &str) -> Self {
        HtmlNode::Expression(expr.to_string())
    }

    pub fn component(name: &str) -> Self {
        HtmlNode::Component(HtmlComponent {
            name: name.to_string(),
            attributes: vec![],
            children: vec![],
        })
    }
}
