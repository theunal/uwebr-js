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

fn div_with(props: Vec<(String, PropValue)>, children: Vec<Element>) -> Element {
    Element {
        node_type: NodeType::Element("div".into()),
        props,
        children,
    }
}

fn clickable_button(action: &str, width: f64, height: f64) -> Element {
    Element {
        node_type: NodeType::Element("button".into()),
        props: vec![
            ("on:click".into(), PropValue::Closure(action.into())),
            ("width".into(), PropValue::Number(width)),
            ("height".into(), PropValue::Number(height)),
        ],
        children: vec![text("Click")],
    }
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

// ── Additional interaction tests ─────────────────────────────────

#[test]
fn app_click_dispatches_and_renders_new_state() {
    reset();
    let mut pipeline = RenderPipeline::new().with_css(CSS);

    register_action("double", || {
        let v: i64 = state::get("val", 1);
        state::set("val", v * 2);
    });

    fn comp() -> Element {
        let v: i64 = state::get("val", 1);
        register_action("double", || {
            let val: i64 = state::get("val", 1);
            state::set("val", val * 2);
        });
        div_with(
            vec![("class".into(), PropValue::String("app".into()))],
            vec![
                text(&v.to_string()),
                clickable_button("double", 120.0, 40.0),
            ],
        )
    }

    pipeline.build_render_scene(&comp(), 800, 600);
    assert!(rendered_text(&pipeline).contains(&"1".to_string()));

    let action = pipeline.hit_test(10.0, 50.0).unwrap().to_string();
    dispatch_action(&action);

    pipeline.build_render_scene(&comp(), 800, 600);
    assert!(rendered_text(&pipeline).contains(&"2".to_string()));
}

#[test]
fn app_multiple_clicks_chain_state() {
    reset();
    let mut pipeline = RenderPipeline::new().with_css(CSS);

    fn comp() -> Element {
        let v: i64 = state::get("val", 0);
        register_action("add_5", || {
            let v: i64 = state::get("val", 0);
            state::set("val", v + 5);
        });
        div_with(
            vec![("class".into(), PropValue::String("app".into()))],
            vec![text(&v.to_string()), clickable_button("add_5", 120.0, 40.0)],
        )
    }

    for expected in [5, 10, 15, 20, 25] {
        pipeline.build_render_scene(&comp(), 800, 600);
        let action = pipeline.hit_test(10.0, 50.0).unwrap().to_string();
        dispatch_action(&action);
        pipeline.build_render_scene(&comp(), 800, 600);
        assert!(
            rendered_text(&pipeline).contains(&expected.to_string()),
            "expected {expected} after click, got {:?}",
            rendered_text(&pipeline)
        );
    }
}

#[test]
fn app_hover_state_applies_css_rule() {
    reset();
    let mut pipeline = RenderPipeline::new()
        .with_css(".box { width: 100px; height: 40px; } .box:hover { background-color: green; }");
    let el = div_with(
        vec![("class".into(), PropValue::String("box".into()))],
        vec![],
    );

    pipeline.build_render_scene(&el, 800, 600);
    assert!(
        pipeline.render_scene().nodes()[0]
            .style
            .background
            .is_none(),
        "no hover yet"
    );

    uwebr_core::state::set_hovered(0, true);
    pipeline.build_render_scene(&el, 800, 600);
    assert!(
        pipeline.render_scene().nodes()[0]
            .style
            .background
            .is_some(),
        ":hover background should appear"
    );
    uwebr_core::state::clear_element_state();
}

#[test]
fn app_focus_state_tracks_node() {
    reset();
    state::clear_element_state();
    assert!(!state::any_focused());

    state::set_focused(Some(7));
    assert!(state::is_focused(7));
    assert!(!state::is_focused(3));
    assert!(state::any_focused());

    state::set_focused(None);
    assert!(!state::is_focused(7));
    assert!(!state::any_focused());
}

#[test]
fn app_hover_and_focus_independent() {
    reset();
    state::clear_element_state();

    state::set_hovered(1, true);
    state::set_focused(Some(2));

    assert!(state::is_hovered(1));
    assert!(!state::is_hovered(2));
    assert!(state::is_focused(2));
    assert!(!state::is_focused(1));
    assert!(state::any_focused());

    // Clearing hover doesn't affect focus
    state::clear_hover();
    assert!(!state::is_hovered(1));
    assert!(state::is_focused(2));
}

#[test]
fn app_clear_element_state_resets_all() {
    reset();
    state::set_hovered(5, true);
    state::set_focused(Some(10));
    assert!(state::any_focused());

    state::clear_element_state();
    assert!(!state::is_hovered(5));
    assert!(!state::is_focused(10));
    assert!(!state::any_focused());
}

#[test]
fn app_click_on_nested_hit_target() {
    reset();
    let mut pipeline = RenderPipeline::new().with_css(CSS);

    register_action("btn1", || {
        state::set("last".to_string(), "btn1".to_string());
    });
    register_action("btn2", || {
        state::set("last".to_string(), "btn2".to_string());
    });

    let el = div_with(
        vec![("class".into(), PropValue::String("app".into()))],
        vec![
            clickable_button("btn1", 120.0, 40.0),
            clickable_button("btn2", 120.0, 40.0),
        ],
    );

    pipeline.build_render_scene(&el, 800, 600);

    let targets = pipeline.hit_targets();
    assert_eq!(targets.len(), 2);

    // Both should be hit-testable at different positions
    let action1 = pipeline.hit_test(10.0, 10.0).unwrap().to_string();
    dispatch_action(&action1);
    assert_eq!(
        state::get::<String>("last".to_string(), String::new()),
        "btn1"
    );

    let action2 = pipeline.hit_test(10.0, 50.0).unwrap().to_string();
    dispatch_action(&action2);
    assert_eq!(
        state::get::<String>("last".to_string(), String::new()),
        "btn2"
    );
    clear_actions();
}

#[test]
fn app_state_write_then_render_consistent() {
    reset();
    let mut pipeline = RenderPipeline::new().with_css(CSS);

    state::set("name".to_string(), "Alice".to_string());

    fn comp() -> Element {
        let name: String = state::get("name".to_string(), "Unknown".to_string());
        register_action("noop", || {});
        div_with(
            vec![("class".into(), PropValue::String("app".into()))],
            vec![text(&name)],
        )
    }

    pipeline.build_render_scene(&comp(), 800, 600);
    assert!(rendered_text(&pipeline).contains(&"Alice".to_string()));

    state::set("name".to_string(), "Bob".to_string());
    pipeline.build_render_scene(&comp(), 800, 600);
    assert!(rendered_text(&pipeline).contains(&"Bob".to_string()));
    assert!(!rendered_text(&pipeline).contains(&"Alice".to_string()));
}
