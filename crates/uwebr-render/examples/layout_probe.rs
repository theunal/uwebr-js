//! Diagnostic: show how font-size flows into the computed text box.
//!
//! Text leaves are measured through parley via Taffy's measure function, so a
//! larger `font-size` must yield a taller box. Before FAZ 8 they measured 0x0.
//!
//! Run with `cargo run -p uwebr-render --example layout_probe`.

use uwebr_core::component::{Element, NodeType};
use uwebr_render::layout::LayoutEngine;
use uwebr_render::stylebook::StyleBook;

fn text(c: &str) -> Element {
    Element {
        node_type: NodeType::Text(c.into()),
        props: vec![],
        children: vec![],
    }
}

fn main() {
    let sb = StyleBook::parse("h1 { font-size: 48px; } h2 { font-size: 12px; }").unwrap();
    for tag in ["h1", "h2"] {
        let el = Element {
            node_type: NodeType::Element(tag.into()),
            props: vec![],
            children: vec![text("Hello")],
        };
        let mut e = LayoutEngine::new();
        let root = e.build_tree(&el, &sb).unwrap();
        e.compute(root, 800.0, 600.0).unwrap();
        for n in e.collect_positioned_nodes(root, &el, &sb) {
            println!(
                "{tag} {:?} {:?} fs={}",
                n.element.node_type, n.layout, n.paint.font_size
            );
        }
    }
}
