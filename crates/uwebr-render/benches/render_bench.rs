use criterion::{black_box, criterion_group, criterion_main, Criterion};

use uwebr_core::component::{Element, NodeType, PropValue};
use uwebr_render::layout::LayoutEngine;
use uwebr_render::scene::RenderScene;
use uwebr_render::scene_builder::SceneBuilder;
use uwebr_render::stylebook::StyleBook;
use uwebr_render::text::TextRenderer;

/// Build a flat tree of `count` `<div class="box">` children under one root.
fn flat_tree(count: usize) -> Element {
    let children = (0..count)
        .map(|i| Element {
            node_type: NodeType::Element("div".to_string()),
            props: vec![("class".to_string(), PropValue::String("box".to_string()))],
            children: vec![Element::text(&format!("Node {i}"))],
        })
        .collect();

    Element {
        node_type: NodeType::Element("div".to_string()),
        props: vec![],
        children,
    }
}

fn bench_css_parse(c: &mut Criterion) {
    let css = ".a { width: 100px; height: 200px; background: red; }";
    c.bench_function("css_parse_simple", |b| {
        b.iter(|| StyleBook::parse(black_box(css)).unwrap());
    });
}

fn bench_layout_100_nodes(c: &mut Criterion) {
    let css = ".box { width: 50px; height: 50px; }";
    let root = flat_tree(100);
    let stylebook = StyleBook::parse(css).unwrap();

    c.bench_function("layout_100_nodes", |b| {
        b.iter(|| {
            let mut engine = LayoutEngine::new();
            let node = engine.build_tree(black_box(&root), &stylebook).unwrap();
            engine.compute(node, 800.0, 600.0).unwrap();
        });
    });
}

fn bench_scene_build(c: &mut Criterion) {
    let scene = RenderScene::new();
    c.bench_function("scene_build_empty", |b| {
        b.iter(|| {
            let _ = SceneBuilder::build_scene(black_box(&scene), 800, 600);
        });
    });
}

fn bench_text_measure(c: &mut Criterion) {
    let mut renderer = TextRenderer::new();
    c.bench_function("text_measure_short", |b| {
        b.iter(|| {
            renderer.measure(black_box("Hello World"), 16.0, None, None);
        });
    });
}

criterion_group!(
    benches,
    bench_css_parse,
    bench_layout_100_nodes,
    bench_scene_build,
    bench_text_measure,
);
criterion_main!(benches);
