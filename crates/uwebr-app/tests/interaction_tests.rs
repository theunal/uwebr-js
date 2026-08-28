//! End-to-end tests for the interaction loop: click → action → state → repaint.
//!
//! These exercise the seam between `uwebr-app`'s hit-testing and `uwebr-core`'s
//! action registry and dirty flag. Before FAZ 8 none of these links existed:
//! `on:click` was dropped as a plain string and only timers requested redraws.

use uwebr_app::RenderPipeline;
use uwebr_core::component::{Element, NodeType, PropValue};
use uwebr_core::events::{clear_actions, dispatch_action, register_action};
use uwebr_core::signal::{is_render_dirty, take_render_dirty};
use uwebr_core::state;

const CSS: &str = r#"
.app { display: flex; flex-direction: column; background-color: #1a1a2e; color: #e0e0e0; }
button { width: 120px; height: 40px; }
"#;

fn text(content: &str) -> Element {
    Element {
        node_type: NodeType::Text(content.to_string()),
        props: vec![],
        children: vec![],
    }
}

/// The component a transpiled counter `.uwebr` file produces.
fn counter_component() -> Element {
    register_action("increment", || {
        let current: i64 = state::get("count", 0);
        state::set("count", current + 1);
    });

    let count: i64 = state::get("count", 0);

    Element {
        node_type: NodeType::Element("div".into()),
        props: vec![("class".into(), PropValue::String("app".into()))],
        children: vec![
            Element {
                node_type: NodeType::Element("p".into()),
                props: vec![],
                children: vec![text(&count.to_string())],
            },
            Element {
                node_type: NodeType::Element("button".into()),
                props: vec![("on:click".into(), PropValue::Closure("increment".into()))],
                children: vec![text("Increment")],
            },
        ],
    }
}

/// Reset the thread-local state these tests share.
fn reset() {
    state::clear();
    clear_actions();
    take_render_dirty();
}

fn rendered_text(pipeline: &RenderPipeline) -> Vec<String> {
    pipeline
        .render_scene()
        .nodes()
        .iter()
        .filter_map(|n| match &n.kind {
            uwebr_render::scene::RenderNodeKind::Text { content, .. } => Some(content.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn click_runs_handler_and_updates_rendered_text() {
    reset();
    let mut pipeline = RenderPipeline::new().with_css(CSS);

    pipeline.build_render_scene(&counter_component(), 800, 600);
    assert!(
        rendered_text(&pipeline).contains(&"0".to_string()),
        "initial count should render as 0, got {:?}",
        rendered_text(&pipeline)
    );

    // Click the button by coordinate, exactly as the winit handler does.
    let action = pipeline
        .hit_test(10.0, 50.0)
        .expect("button should be hit-testable")
        .to_string();
    assert_eq!(action, "increment");
    assert!(dispatch_action(&action));

    pipeline.build_render_scene(&counter_component(), 800, 600);
    assert!(
        rendered_text(&pipeline).contains(&"1".to_string()),
        "count should render as 1 after the click, got {:?}",
        rendered_text(&pipeline)
    );
}

#[test]
fn click_marks_the_ui_dirty() {
    reset();
    let mut pipeline = RenderPipeline::new().with_css(CSS);
    pipeline.build_render_scene(&counter_component(), 800, 600);

    take_render_dirty();
    assert!(!is_render_dirty());

    let action = pipeline.hit_test(10.0, 50.0).unwrap().to_string();
    dispatch_action(&action);

    assert!(
        is_render_dirty(),
        "state write must schedule a repaint, otherwise the screen never updates"
    );
}

#[test]
fn repeated_clicks_accumulate() {
    reset();
    let mut pipeline = RenderPipeline::new().with_css(CSS);

    for _ in 0..3 {
        pipeline.build_render_scene(&counter_component(), 800, 600);
        let action = pipeline.hit_test(10.0, 50.0).unwrap().to_string();
        dispatch_action(&action);
    }

    pipeline.build_render_scene(&counter_component(), 800, 600);
    assert!(rendered_text(&pipeline).contains(&"3".to_string()));
}

#[test]
fn click_outside_the_button_does_nothing() {
    reset();
    let mut pipeline = RenderPipeline::new().with_css(CSS);
    pipeline.build_render_scene(&counter_component(), 800, 600);

    take_render_dirty();
    assert!(pipeline.hit_test(700.0, 550.0).is_none());
    assert!(!is_render_dirty());
}

#[test]
fn rerender_does_not_reset_state() {
    // The component re-registers its handler and re-reads `count` on every
    // render; passing the literal initial value again must not clobber it.
    reset();
    let mut pipeline = RenderPipeline::new().with_css(CSS);

    pipeline.build_render_scene(&counter_component(), 800, 600);
    dispatch_action("increment");

    for _ in 0..5 {
        pipeline.build_render_scene(&counter_component(), 800, 600);
    }

    assert_eq!(state::get::<i64>("count", 0), 1);
}

#[test]
fn scene_contains_background_and_glyphs() {
    reset();
    let mut pipeline = RenderPipeline::new().with_css(CSS);
    let scene = pipeline.render(&counter_component(), 800, 600);
    let enc = scene.encoding();

    assert!(
        enc.n_paths >= 2,
        "surface background + .app background expected, got {}",
        enc.n_paths
    );
    assert!(
        !enc.resources.glyphs.is_empty(),
        "expected glyphs for the button label and the count"
    );
}

#[test]
fn inherited_text_colour_reaches_the_scene() {
    reset();
    let mut pipeline = RenderPipeline::new().with_css(CSS);
    pipeline.build_render_scene(&counter_component(), 800, 600);

    let colours: Vec<_> = pipeline
        .render_scene()
        .nodes()
        .iter()
        .filter_map(|n| match &n.kind {
            uwebr_render::scene::RenderNodeKind::Text { color, .. } => Some(*color),
            _ => None,
        })
        .collect();

    assert!(!colours.is_empty());
    for c in colours {
        assert_eq!(
            c,
            vello::peniko::Color::from_rgba8(0xe0, 0xe0, 0xe0, 255),
            ".app color must be inherited by descendant text"
        );
    }
}

#[test]
fn hit_target_bounds_match_the_css_size() {
    reset();
    let mut pipeline = RenderPipeline::new().with_css(CSS);
    pipeline.build_render_scene(&counter_component(), 800, 600);

    let target = &pipeline.hit_targets()[0];
    assert_eq!(target.action, "increment");
    assert_eq!(target.bounds.width, 120.0);
    assert_eq!(target.bounds.height, 40.0);
}
