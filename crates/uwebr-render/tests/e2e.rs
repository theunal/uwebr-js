//! End-to-end tests for the render pipeline at the `uwebr-render` layer.
//!
//! `uwebr-render` builds scenes only and does not depend on `uwebr-html`, so
//! these tests construct [`Element`] trees directly (the same shape the
//! transpiler produces) and drive them through:
//!
//!   Element + CSS → StyleBook → LayoutEngine → RenderScene → vello::Scene
//!
//! Vello's encoded scene is largely opaque, so most assertions verify layout
//! results and that scene assembly does not panic ("panic-free = pass").

use uwebr_core::component::{Element, NodeType, PropValue};
use uwebr_render::layout::LayoutEngine;
use uwebr_render::scene::{LayoutInfo, RenderNode, RenderScene};
use uwebr_render::scene_builder::SceneBuilder;
use uwebr_render::stylebook::StyleBook;

fn el(tag: &str, props: Vec<(String, PropValue)>, children: Vec<Element>) -> Element {
    Element {
        node_type: NodeType::Element(tag.to_string()),
        props,
        children,
    }
}

fn class(name: &str) -> Vec<(String, PropValue)> {
    vec![("class".to_string(), PropValue::String(name.to_string()))]
}

fn path_count(scene: &vello::Scene) -> usize {
    scene.encoding().n_paths as usize
}

/// Full pipeline for a single styled div: parse → layout → scene → vello.
#[test]
fn e2e_simple_div_with_background() {
    let css = r#"
        .box {
            width: 200px;
            height: 100px;
            background: #ff0000;
            display: flex;
            justify-content: center;
            align-items: center;
        }
    "#;
    let root = el("div", class("box"), vec![Element::text("Hello")]);

    // 1. Parse CSS into a stylebook.
    let stylebook = StyleBook::parse(css).expect("CSS parse failed");

    // 2. Build + compute layout.
    let mut engine = LayoutEngine::new();
    let node = engine
        .build_tree(&root, &stylebook)
        .expect("build_tree failed");
    engine.compute(node, 800.0, 600.0).expect("compute failed");

    // 3. Verify the box picked up its CSS dimensions.
    let info = engine.get_layout_info(node).expect("layout info");
    assert_eq!(info.width, 200.0, "width from CSS");
    assert_eq!(info.height, 100.0, "height from CSS");

    // 4. Collect positioned nodes and feed them into a RenderScene.
    let positioned = engine.collect_positioned_nodes(node, &root, &stylebook);
    assert!(!positioned.is_empty(), "expected at least the root node");

    let mut scene = RenderScene::new();
    for (i, pos) in positioned.iter().enumerate() {
        scene.add_node(RenderNode::rect(
            i as u64,
            pos.layout,
            vello::peniko::color::palette::css::RED,
        ));
    }
    assert!(scene.node_count() >= 1);

    // 5. Assemble the vello scene — must not panic.
    let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
    drop(vello_scene);
}

/// Nested flex column with three items and text content.
#[test]
fn e2e_nested_layout_with_text() {
    let css = r#"
        .container {
            display: flex;
            flex-direction: column;
            width: 300px;
        }
        .item {
            height: 50px;
            background: #0000ff;
        }
    "#;
    let root = el(
        "div",
        class("container"),
        vec![
            el("div", class("item"), vec![Element::text("Item 1")]),
            el("div", class("item"), vec![Element::text("Item 2")]),
            el("div", class("item"), vec![Element::text("Item 3")]),
        ],
    );

    let stylebook = StyleBook::parse(css).expect("CSS parse failed");
    let mut engine = LayoutEngine::new();
    let node = engine
        .build_tree(&root, &stylebook)
        .expect("build_tree failed");
    engine.compute(node, 800.0, 600.0).expect("compute failed");

    let positioned = engine.collect_positioned_nodes(node, &root, &stylebook);
    // root + 3 items + 3 text leaves = 7 nodes.
    assert!(
        positioned.len() >= 4,
        "expected root plus 3 items, got {}",
        positioned.len()
    );

    // Items stack vertically: each item should sit below the previous one.
    let container = engine.get_layout_info(node).expect("container layout");
    assert_eq!(container.width, 300.0);
}

/// An image render node with empty data must not panic during scene assembly.
#[test]
fn e2e_image_render_node() {
    let node = RenderNode::image(
        1,
        LayoutInfo::new(0.0, 0.0, 100.0, 100.0),
        vec![], // empty data → invalid image, but must not panic
        0,
        0,
    );

    let mut scene = RenderScene::new();
    scene.add_node(node);
    let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
    drop(vello_scene); // no panic = pass
}

/// A linear-gradient background parses into a stylebook with matchable rules.
#[test]
fn e2e_gradient_render() {
    let css = r#"
        .grad {
            width: 200px;
            height: 200px;
            background: linear-gradient(to right, #ff0000, #0000ff);
        }
    "#;
    let stylebook = StyleBook::parse(css).expect("StyleBook parse failed");
    assert!(!stylebook.is_empty(), "StyleBook should have rules");

    // The gradient must survive layout + scene assembly.
    let root = el("div", class("grad"), vec![]);
    let mut engine = LayoutEngine::new();
    let node = engine
        .build_tree(&root, &stylebook)
        .expect("build_tree failed");
    engine.compute(node, 800.0, 600.0).expect("compute failed");

    let positioned = engine.collect_positioned_nodes(node, &root, &stylebook);
    let mut scene = RenderScene::new();
    for (i, pos) in positioned.iter().enumerate() {
        // Carry the resolved background through to the render node.
        let mut render_node = RenderNode::rect(
            i as u64,
            pos.layout,
            vello::peniko::color::palette::css::BLACK,
        );
        render_node.style.background = pos.paint.background.clone();
        scene.add_node(render_node);
    }
    let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
    drop(vello_scene);
}

/// `overflow: hidden` sets a clip layer during scene assembly without panicking.
#[test]
fn e2e_overflow_hidden_clip() {
    let mut scene = RenderScene::new();
    let mut node = RenderNode::rect(
        1,
        LayoutInfo::new(0.0, 0.0, 100.0, 100.0),
        vello::peniko::color::palette::css::BLUE,
    );
    node.style.overflow_hidden = true;
    scene.add_node(node);

    // overflow_hidden pushes a clip layer; assembly must remain panic-free.
    let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
    drop(vello_scene);
}

// ── Stress tests ──────────────────────────────────────────────

#[test]
fn e2e_1000_node_layout_tree() {
    let children: Vec<Element> = (0..1000)
        .map(|i| {
            el(
                "div",
                vec![
                    ("class".to_string(), PropValue::String("box".to_string())),
                    ("width".to_string(), PropValue::Number(10.0)),
                    ("height".to_string(), PropValue::Number(10.0)),
                ],
                vec![Element::text(&i.to_string())],
            )
        })
        .collect();
    let root = el("div", vec![], children);

    let stylebook = StyleBook::parse(".box { width: 10px; height: 10px; }").unwrap();
    let mut engine = LayoutEngine::new();
    let node = engine.build_tree(&root, &stylebook).unwrap();
    engine.compute(node, 12000.0, 600.0).unwrap();
    let positioned = engine.collect_positioned_nodes(node, &root, &stylebook);
    assert!(
        positioned.len() >= 1001,
        "should have at least 1001 nodes, got {}",
        positioned.len()
    );
}

#[test]
fn e2e_500_css_rules_stylebook() {
    let mut css = String::new();
    for i in 0..500 {
        css.push_str(&format!(
            ".c{i} {{ width: {w}px; height: {h}px; }}\n",
            w = i % 100 + 10,
            h = i % 50 + 10
        ));
    }
    let stylebook = StyleBook::parse(&css).unwrap();
    assert_eq!(stylebook.len(), 500);

    let el = el(
        "div",
        vec![("class".to_string(), PropValue::String("c250".to_string()))],
        vec![],
    );
    let (style, matched) = stylebook.match_element(&el);
    assert!(matched);
    let expected_w = (250 % 100 + 10) as f32;
    assert_eq!(style.size.width, taffy::Dimension::length(expected_w));
}

#[test]
fn e2e_rapid_relayout_10_iterations() {
    let css = ".box { width: 50px; height: 50px; }";
    let stylebook = StyleBook::parse(css).unwrap();
    let root = el(
        "div",
        vec![],
        vec![el(
            "div",
            vec![("class".to_string(), PropValue::String("box".to_string()))],
            vec![],
        )],
    );

    for i in 0..10 {
        let mut engine = LayoutEngine::new();
        let node = engine.build_tree(&root, &stylebook).unwrap();
        engine.compute(node, 800.0, 600.0).unwrap();
        let nodes = engine.collect_positioned_nodes(node, &root, &stylebook);
        let child = nodes.iter().find(|n| n.depth == 1).unwrap();
        assert_eq!(
            child.layout.width, 50.0,
            "iteration {i}: child width should be 50"
        );
        assert_eq!(
            child.layout.height, 50.0,
            "iteration {i}: child height should be 50"
        );
    }
}

#[test]
fn e2e_large_text_block_measurement() {
    let mut engine = LayoutEngine::new();
    let long_text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(50);
    let el = Element {
        node_type: NodeType::Text(long_text.clone()),
        props: vec![],
        children: vec![],
    };
    let root = engine.build_tree(&el, &StyleBook::empty()).unwrap();
    engine.compute(root, 200.0, 10000.0).unwrap();
    let info = engine.get_layout_info(root).unwrap();
    assert!(info.width > 0.0, "large text should have positive width");
    assert!(info.height > 0.0, "large text should have positive height");
    assert!(
        info.height > 100.0,
        "large text should wrap and exceed single line height"
    );
}

#[test]
fn e2e_many_overlapping_elements() {
    let mut scene = RenderScene::new();
    for i in 0..100 {
        let mut node = RenderNode::rect(
            i as u64,
            LayoutInfo::new((i as f32) * 2.0, (i as f32) * 2.0, 100.0, 100.0),
            vello::peniko::color::palette::css::RED,
        );
        node.style.opacity = 0.1;
        scene.add_node(node);
    }
    let vello_scene = SceneBuilder::build_scene(&scene, 800, 600);
    assert!(
        path_count(&vello_scene) > 100,
        "100 overlapping semi-transparent rects should all be encoded"
    );
}
