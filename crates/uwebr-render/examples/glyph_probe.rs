//! Diagnostic: verify that a text render node actually produces glyphs.
//!
//! Run with `cargo run -p uwebr-render --example glyph_probe`.

fn main() {
    use uwebr_render::scene::{LayoutInfo, RenderNode, RenderScene};
    use uwebr_render::scene_builder::SceneBuilder;

    let mut scene = RenderScene::new();
    scene.add_node(RenderNode::text(
        1,
        LayoutInfo::new(10.0, 10.0, 400.0, 30.0),
        "Hello from uwebr!",
        24.0,
        vello::peniko::color::palette::css::WHITE,
    ));

    let vs = SceneBuilder::build_scene(&scene, 800, 600);
    let enc = vs.encoding();
    println!(
        "glyphs={} glyph_runs={} paths={} clips={}",
        enc.resources.glyphs.len(),
        enc.resources.glyph_runs.len(),
        enc.n_paths,
        enc.n_clips
    );

    let mut engine = uwebr_render::layout::LayoutEngine::new();
    let (w, h) = engine.measure_text("Hello from uwebr!", 24.0);
    println!("measured text: {w} x {h}");
}
