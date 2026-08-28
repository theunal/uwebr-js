pub mod ast;
pub mod codegen;
pub mod directives;
pub mod parser;

pub use ast::{
    HtmlAttribute, HtmlAttributeValue, HtmlComponent, HtmlEach, HtmlElement, HtmlIf, HtmlNode,
};
pub use codegen::generate_rsx;
pub use directives::expand_directives;
pub use parser::{parse_fragment, parse_html};

/// Parse HTML and expand template directives
pub fn parse_html_with_directives(html: &str) -> Result<HtmlNode, anyhow::Error> {
    let mut node = parse_html(html)?;
    expand_directives(&mut node);
    Ok(node)
}

/// Parse HTML fragment and expand template directives
pub fn parse_fragment_with_directives(html: &str) -> Result<Vec<HtmlNode>, anyhow::Error> {
    let mut nodes = parse_fragment(html)?;
    for node in &mut nodes {
        expand_directives(node);
    }
    Ok(nodes)
}
