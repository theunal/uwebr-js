pub mod ast;
pub mod parser;
pub mod codegen;
pub mod directives;

pub use ast::{HtmlNode, HtmlElement, HtmlAttribute, HtmlAttributeValue, HtmlComponent, HtmlEach, HtmlIf};
pub use parser::{parse_html, parse_fragment};
pub use codegen::generate_rsx;
pub use directives::expand_directives;

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
