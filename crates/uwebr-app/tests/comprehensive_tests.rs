//! Comprehensive tests for pipeline composition, multi-component, event
//! handling, and state integration.

use uwebr_app::component::Component;
use uwebr_app::{App, AppEvent, FnComponent, RenderPipeline};
use uwebr_core::component::{Element, NodeType, PropValue};
use uwebr_core::events::{clear_actions, dispatch_action, register_action};
use uwebr_core::signal::{is_render_dirty, take_render_dirty};
use uwebr_core::state;
use uwebr_render::scene::RenderNodeKind;
use winit::event::MouseButton;

// ── Helpers ────────────────────────────────────────────────────────

fn reset() {
    state::clear();
    clear_actions();
    take_render_dirty();
}

fn text(content: &str) -> Element {
    Element::text(content)
}

fn div(children: Vec<Element>) -> Element {
    Element {
        node_type: NodeType::Element("div".into()),
        props: vec![],
        children,
    }
}

fn div_with(props: Vec<(String, PropValue)>, children: Vec<Element>) -> Element {
    Element {
        node_type: NodeType::Element("div".into()),
        props,
        children,
    }
}

fn el(tag: &str, props: Vec<(String, PropValue)>, children: Vec<Element>) -> Element {
    Element {
        node_type: NodeType::Element(tag.into()),
        props,
        children,
    }
}

fn rendered_text(pipeline: &RenderPipeline) -> Vec<String> {
    pipeline
        .render_scene()
        .nodes()
        .iter()
        .filter_map(|n| match &n.kind {
            RenderNodeKind::Text { content, .. } => Some(content.clone()),
            _ => None,
        })
        .collect()
}

fn node_count(pipeline: &RenderPipeline) -> usize {
    pipeline.render_scene().nodes().len()
}

fn clickable_button(action: &str, width: f64, height: f64) -> Element {
    el(
        "button",
        vec![
            ("on:click".into(), PropValue::Closure(action.into())),
            ("width".into(), PropValue::Number(width)),
            ("height".into(), PropValue::Number(height)),
        ],
        vec![text("Click")],
    )
}

const CSS: &str = r#"
.app { display: flex; flex-direction: column; background-color: #1a1a2e; color: #e0e0e0; }
button { width: 120px; height: 40px; }
.card { width: 200px; height: 100px; padding: 10px; }
.header { width: 100%; height: 60px; }
.content { flex: 1; padding: 16px; }
.footer { height: 40px; }
"#;

// ══════════════════════════════════════════════════════════════════
//  Pipeline Composition (~20 tests)
// ══════════════════════════════════════════════════════════════════

#[test]
fn app_pipeline_multiple_components_sequential() {
    let mut pipeline = RenderPipeline::new();
    let comp_a = div(vec![text("A")]);
    let comp_b = div(vec![text("B")]);
    let comp_c = div(vec![text("C")]);

    pipeline.build_render_scene(&comp_a, 800, 600);
    let texts_a = rendered_text(&pipeline);
    assert!(texts_a.contains(&"A".to_string()));

    pipeline.build_render_scene(&comp_b, 800, 600);
    let texts_b = rendered_text(&pipeline);
    assert!(texts_b.contains(&"B".to_string()));
    assert!(!texts_b.contains(&"A".to_string()));

    pipeline.build_render_scene(&comp_c, 800, 600);
    let texts_c = rendered_text(&pipeline);
    assert!(texts_c.contains(&"C".to_string()));
}

#[test]
fn app_pipeline_component_nesting_parent_child() {
    let mut pipeline = RenderPipeline::new();
    let child = div(vec![text("Child")]);
    let parent = div(vec![child]);
    pipeline.build_render_scene(&parent, 800, 600);

    let texts = rendered_text(&pipeline);
    assert_eq!(texts.len(), 1);
    assert_eq!(texts[0], "Child");
}

#[test]
fn app_pipeline_deeply_nested_components() {
    let mut pipeline = RenderPipeline::new();
    let level3 = div(vec![text("Deep")]);
    let level2 = div(vec![level3]);
    let level1 = div(vec![level2]);
    let root = div(vec![level1]);
    pipeline.build_render_scene(&root, 800, 600);

    let texts = rendered_text(&pipeline);
    assert_eq!(texts.len(), 1);
    assert_eq!(texts[0], "Deep");
}

#[test]
fn app_pipeline_conditional_rendering_true_branch() {
    let mut pipeline = RenderPipeline::new();
    let show = true;
    let children = if show { vec![text("Visible")] } else { vec![] };
    let el = div(children);
    pipeline.build_render_scene(&el, 800, 600);

    let texts = rendered_text(&pipeline);
    assert_eq!(texts.len(), 1);
    assert_eq!(texts[0], "Visible");
}

#[test]
fn app_pipeline_conditional_rendering_false_branch() {
    let mut pipeline = RenderPipeline::new();
    let show = false;
    let children = if show { vec![text("Visible")] } else { vec![] };
    let el = div(children);
    pipeline.build_render_scene(&el, 800, 600);

    let texts = rendered_text(&pipeline);
    assert!(texts.is_empty());
}

#[test]
fn app_pipeline_multiple_css_rules() {
    let css = ".a { width: 100px; height: 50px; } .b { width: 200px; height: 80px; } .c { width: 300px; height: 120px; }";
    let mut pipeline = RenderPipeline::new().with_css(css);
    let el = div(vec![
        div_with(
            vec![("class".into(), PropValue::String("a".into()))],
            vec![],
        ),
        div_with(
            vec![("class".into(), PropValue::String("b".into()))],
            vec![],
        ),
        div_with(
            vec![("class".into(), PropValue::String("c".into()))],
            vec![],
        ),
    ]);
    pipeline.build_render_scene(&el, 800, 600);
    assert!(node_count(&pipeline) >= 4);
}

#[test]
fn app_pipeline_complex_element_tree() {
    let mut pipeline = RenderPipeline::new().with_css(CSS);
    let tree = div_with(
        vec![("class".into(), PropValue::String("app".into()))],
        vec![
            el(
                "header",
                vec![("class".into(), PropValue::String("header".into()))],
                vec![text("Header")],
            ),
            el(
                "main",
                vec![],
                vec![
                    el("h1", vec![], vec![text("Title")]),
                    el("p", vec![], vec![text("Paragraph 1")]),
                    el("p", vec![], vec![text("Paragraph 2")]),
                ],
            ),
            el(
                "footer",
                vec![("class".into(), PropValue::String("footer".into()))],
                vec![text("Footer")],
            ),
        ],
    );
    pipeline.build_render_scene(&tree, 800, 600);

    let texts = rendered_text(&pipeline);
    assert!(texts.contains(&"Header".to_string()));
    assert!(texts.contains(&"Title".to_string()));
    assert!(texts.contains(&"Paragraph 1".to_string()));
    assert!(texts.contains(&"Paragraph 2".to_string()));
    assert!(texts.contains(&"Footer".to_string()));
}

#[test]
fn app_pipeline_rerender_after_state_change() {
    reset();
    let mut pipeline = RenderPipeline::new().with_css(CSS);

    fn component() -> Element {
        let count: i64 = state::get("counter", 0);
        register_action("inc", || {
            let c: i64 = state::get("counter", 0);
            state::set("counter", c + 1);
        });
        div_with(
            vec![("class".into(), PropValue::String("app".into()))],
            vec![
                text(&count.to_string()),
                clickable_button("inc", 120.0, 40.0),
            ],
        )
    }

    pipeline.build_render_scene(&component(), 800, 600);
    assert!(rendered_text(&pipeline).contains(&"0".to_string()));

    let action = pipeline.hit_test(10.0, 50.0).unwrap().to_string();
    dispatch_action(&action);

    pipeline.build_render_scene(&component(), 800, 600);
    assert!(rendered_text(&pipeline).contains(&"1".to_string()));
}

#[test]
fn app_pipeline_text_and_element_mix() {
    let mut pipeline = RenderPipeline::new();
    let el = div(vec![
        text("Before"),
        el("span", vec![], vec![text("Middle")]),
        text("After"),
    ]);
    pipeline.build_render_scene(&el, 800, 600);

    let texts = rendered_text(&pipeline);
    assert_eq!(texts.len(), 3);
    assert!(texts.contains(&"Before".to_string()));
    assert!(texts.contains(&"Middle".to_string()));
    assert!(texts.contains(&"After".to_string()));
}

#[test]
fn app_pipeline_event_handler_on_element() {
    let mut pipeline = RenderPipeline::new();
    register_action("clicked", || {});
    let el = el(
        "button",
        vec![
            ("on:click".into(), PropValue::Closure("clicked".into())),
            ("width".into(), PropValue::Number(100.0)),
            ("height".into(), PropValue::Number(40.0)),
        ],
        vec![text("Click Me")],
    );
    pipeline.build_render_scene(&el, 800, 600);

    assert_eq!(pipeline.hit_targets().len(), 1);
    assert_eq!(pipeline.hit_targets()[0].action, "clicked");
    clear_actions();
}

#[test]
fn app_pipeline_image_node() {
    let mut pipeline = RenderPipeline::new();
    let el = el(
        "img",
        vec![
            ("src".into(), PropValue::String("test.png".into())),
            ("width".into(), PropValue::Number(200.0)),
            ("height".into(), PropValue::Number(150.0)),
        ],
        vec![],
    );
    pipeline.build_render_scene(&el, 800, 600);

    let nodes = pipeline.render_scene().nodes();
    assert_eq!(nodes.len(), 1);
    match &nodes[0].kind {
        RenderNodeKind::Image {
            data,
            width,
            height,
        } => {
            assert_eq!(data, b"test.png");
            assert_eq!(*width, 200);
            assert_eq!(*height, 150);
        }
        other => panic!("expected image node, got {other:?}"),
    }
}

#[test]
fn app_pipeline_hit_test_nested_elements() {
    let mut pipeline = RenderPipeline::new();
    let inner = el(
        "button",
        vec![
            ("on:click".into(), PropValue::Closure("inner".into())),
            ("width".into(), PropValue::Number(60.0)),
            ("height".into(), PropValue::Number(30.0)),
        ],
        vec![],
    );
    let outer = el(
        "div",
        vec![
            ("on:click".into(), PropValue::Closure("outer".into())),
            ("width".into(), PropValue::Number(200.0)),
            ("height".into(), PropValue::Number(100.0)),
        ],
        vec![inner],
    );

    pipeline.build_render_scene(&outer, 800, 600);

    // Inside inner button → innermost wins
    assert_eq!(pipeline.hit_test(10.0, 10.0), Some("inner"));
    // Below inner, still inside outer
    assert_eq!(pipeline.hit_test(10.0, 50.0), Some("outer"));
    // Outside both
    assert_eq!(pipeline.hit_test(300.0, 200.0), None);
}

#[test]
fn app_pipeline_element_with_multiple_children() {
    let mut pipeline = RenderPipeline::new();
    let el = div(vec![
        text("One"),
        text("Two"),
        text("Three"),
        text("Four"),
        text("Five"),
    ]);
    pipeline.build_render_scene(&el, 800, 600);

    let texts = rendered_text(&pipeline);
    assert_eq!(texts.len(), 5);
}

#[test]
fn app_pipeline_div_with_css_flex_row() {
    let mut pipeline =
        RenderPipeline::new().with_css(".row { display: flex; flex-direction: row; }");
    let el = div_with(
        vec![("class".into(), PropValue::String("row".into()))],
        vec![
            div_with(
                vec![("width".into(), PropValue::Number(100.0))],
                vec![text("A")],
            ),
            div_with(
                vec![("width".into(), PropValue::Number(100.0))],
                vec![text("B")],
            ),
        ],
    );
    pipeline.build_render_scene(&el, 800, 600);

    let texts = rendered_text(&pipeline);
    assert!(texts.contains(&"A".to_string()));
    assert!(texts.contains(&"B".to_string()));
}

#[test]
fn app_pipeline_empty_div_produces_container_node() {
    let mut pipeline = RenderPipeline::new();
    let el = div(vec![]);
    pipeline.build_render_scene(&el, 800, 600);

    let nodes = pipeline.render_scene().nodes();
    assert_eq!(nodes.len(), 1);
    assert!(matches!(nodes[0].kind, RenderNodeKind::Container));
}

#[test]
fn app_pipeline_whitespace_only_text_dropped() {
    let mut pipeline = RenderPipeline::new();
    let el = div(vec![text("   \n  ")]);
    pipeline.build_render_scene(&el, 800, 600);

    let texts = rendered_text(&pipeline);
    assert!(texts.is_empty());
}

#[test]
fn app_pipeline_nonzero_text_preserved() {
    let mut pipeline = RenderPipeline::new();
    let el = div(vec![text("X")]);
    pipeline.build_render_scene(&el, 800, 600);

    let texts = rendered_text(&pipeline);
    assert_eq!(texts.len(), 1);
    assert_eq!(texts[0], "X");
}

#[test]
fn app_pipeline_render_is_idempotent() {
    let mut pipeline = RenderPipeline::new().with_css(CSS);
    let el = div_with(
        vec![("class".into(), PropValue::String("app".into()))],
        vec![text("Test")],
    );
    let scene1 = pipeline.render(&el, 800, 600);
    let scene2 = pipeline.render(&el, 800, 600);

    let enc1 = scene1.encoding();
    let enc2 = scene2.encoding();
    assert_eq!(enc1.resources.glyphs.len(), enc2.resources.glyphs.len());
    assert_eq!(enc1.n_paths, enc2.n_paths);
}

#[test]
fn app_pipeline_viewport_resize_re_resolves_vw() {
    let mut pipeline = RenderPipeline::new().with_css(".full { width: 100vw; height: 50vh; }");
    let el = div_with(
        vec![("class".into(), PropValue::String("full".into()))],
        vec![],
    );

    pipeline.build_render_scene(&el, 800, 600);
    assert_eq!(pipeline.render_scene().nodes()[0].layout.width, 800.0);

    pipeline.build_render_scene(&el, 1200, 900);
    assert_eq!(pipeline.render_scene().nodes()[0].layout.width, 1200.0);
}

// ══════════════════════════════════════════════════════════════════
//  Multi-Component (~10 tests)
// ══════════════════════════════════════════════════════════════════

#[test]
fn app_two_independent_components_same_scene() {
    let mut pipeline = RenderPipeline::new();
    let comp_a = div(vec![text("Alpha")]);
    let comp_b = div(vec![text("Beta")]);
    let combined = div(vec![comp_a, comp_b]);
    pipeline.build_render_scene(&combined, 800, 600);

    let texts = rendered_text(&pipeline);
    assert!(texts.contains(&"Alpha".to_string()));
    assert!(texts.contains(&"Beta".to_string()));
}

#[test]
fn app_component_communication_shared_state() {
    reset();
    let mut pipeline = RenderPipeline::new().with_css(CSS);

    register_action("set_a", || {
        state::set("shared", "from_a".to_string());
    });

    fn comp_a() -> Element {
        div(vec![
            clickable_button("set_a", 120.0, 40.0),
            text(&state::get("shared", "none".to_string())),
        ])
    }

    fn comp_b() -> Element {
        let val: String = state::get("shared", "default".to_string());
        div(vec![text(&val)])
    }

    // Render both components
    let scene_a = div(vec![comp_a()]);
    pipeline.build_render_scene(&scene_a, 800, 600);
    assert!(
        rendered_text(&pipeline).contains(&"none".to_string()),
        "initial shared state should be 'none', got {:?}",
        rendered_text(&pipeline)
    );

    // Simulate click on comp_a's button
    let action = pipeline.hit_test(10.0, 10.0).unwrap().to_string();
    dispatch_action(&action);

    // Re-render comp_b — should see the updated shared state
    let scene_b = div(vec![comp_b()]);
    pipeline.build_render_scene(&scene_b, 800, 600);
    assert!(rendered_text(&pipeline).contains(&"from_a".to_string()));
    clear_actions();
}

#[test]
fn app_component_list_rendering() {
    let mut pipeline = RenderPipeline::new();
    let items = vec!["Apple", "Banana", "Cherry", "Date"];
    let list = div(items
        .iter()
        .map(|item| el("li", vec![], vec![text(item)]))
        .collect());
    pipeline.build_render_scene(&list, 800, 600);

    let texts = rendered_text(&pipeline);
    assert_eq!(texts.len(), 4);
    assert!(texts.contains(&"Apple".to_string()));
    assert!(texts.contains(&"Banana".to_string()));
    assert!(texts.contains(&"Cherry".to_string()));
    assert!(texts.contains(&"Date".to_string()));
}

#[test]
fn app_component_with_slots_children() {
    let mut pipeline = RenderPipeline::new();

    // A "card" component that wraps children
    fn card(children: Vec<Element>) -> Element {
        div_with(
            vec![
                ("class".into(), PropValue::String("card".into())),
                ("width".into(), PropValue::Number(200.0)),
                ("height".into(), PropValue::Number(100.0)),
            ],
            children,
        )
    }

    let page = div(vec![
        card(vec![text("Card 1 Content")]),
        card(vec![text("Card 2 Content")]),
    ]);
    pipeline.build_render_scene(&page, 800, 600);

    let texts = rendered_text(&pipeline);
    assert!(texts.contains(&"Card 1 Content".to_string()));
    assert!(texts.contains(&"Card 2 Content".to_string()));
}

#[test]
fn app_dynamic_component_switching() {
    let mut pipeline = RenderPipeline::new();
    let view_a = div(vec![text("View A")]);
    let view_b = div(vec![text("View B")]);

    let show_a = true;
    let root = div(vec![if show_a { view_a } else { view_b }]);
    pipeline.build_render_scene(&root, 800, 600);

    let texts = rendered_text(&pipeline);
    assert!(texts.contains(&"View A".to_string()));
    assert!(!texts.contains(&"View B".to_string()));
}

#[test]
fn app_dynamic_component_switching_toggle() {
    let mut pipeline = RenderPipeline::new();
    reset();

    fn comp(show_second: bool) -> Element {
        if show_second {
            div(vec![text("Second")])
        } else {
            div(vec![text("First")])
        }
    }

    pipeline.build_render_scene(&comp(false), 800, 600);
    assert!(rendered_text(&pipeline).contains(&"First".to_string()));

    pipeline.build_render_scene(&comp(true), 800, 600);
    assert!(rendered_text(&pipeline).contains(&"Second".to_string()));
    assert!(!rendered_text(&pipeline).contains(&"First".to_string()));
}

#[test]
fn app_nested_component_tree_preserves_structure() {
    let mut pipeline = RenderPipeline::new();
    let tree = div(vec![
        el(
            "nav",
            vec![],
            vec![
                el("a", vec![], vec![text("Home")]),
                el("a", vec![], vec![text("About")]),
                el("a", vec![], vec![text("Contact")]),
            ],
        ),
        el(
            "main",
            vec![],
            vec![
                el("h1", vec![], vec![text("Welcome")]),
                el("p", vec![], vec![text("Content here")]),
            ],
        ),
    ]);
    pipeline.build_render_scene(&tree, 800, 600);

    let texts = rendered_text(&pipeline);
    assert_eq!(texts.len(), 5);
}

#[test]
fn app_component_with_mixed_node_types() {
    let mut pipeline = RenderPipeline::new();
    let root = div(vec![
        text("Text node"),
        el("div", vec![], vec![text("Nested element")]),
        Element {
            node_type: NodeType::Raw("<span>Raw HTML</span>".to_string()),
            props: vec![],
            children: vec![],
        },
    ]);
    pipeline.build_render_scene(&root, 800, 600);

    let texts = rendered_text(&pipeline);
    assert!(texts.contains(&"Text node".to_string()));
    assert!(texts.contains(&"Nested element".to_string()));
    assert!(texts.contains(&"Raw HTML".to_string()));
}

#[test]
fn app_component_with_disabled_prop() {
    let mut pipeline = RenderPipeline::new();
    let el = el(
        "button",
        vec![
            ("disabled".into(), PropValue::Bool(true)),
            ("width".into(), PropValue::Number(100.0)),
            ("height".into(), PropValue::Number(40.0)),
        ],
        vec![text("Disabled")],
    );
    pipeline.build_render_scene(&el, 800, 600);

    let texts = rendered_text(&pipeline);
    assert!(texts.contains(&"Disabled".to_string()));
}

#[test]
fn app_fn_component_render_multiple_times() {
    let comp = FnComponent::new(|| div(vec![text("Static")]));
    let mut pipeline = RenderPipeline::new();

    for _ in 0..5 {
        let el = comp.render();
        pipeline.build_render_scene(&el, 800, 600);
        let texts = rendered_text(&pipeline);
        assert_eq!(texts.len(), 1);
        assert_eq!(texts[0], "Static");
    }
}

// ══════════════════════════════════════════════════════════════════
//  Event Handling (~10 tests)
// ══════════════════════════════════════════════════════════════════

#[test]
fn app_click_event_propagation_innermost_wins() {
    reset();
    let mut pipeline = RenderPipeline::new().with_css(CSS);

    register_action("inner_action", || {
        state::set("clicked".to_string(), "inner".to_string());
    });
    register_action("outer_action", || {
        state::set("clicked".to_string(), "outer".to_string());
    });

    let inner = el(
        "button",
        vec![
            ("on:click".into(), PropValue::Closure("inner_action".into())),
            ("width".into(), PropValue::Number(60.0)),
            ("height".into(), PropValue::Number(30.0)),
        ],
        vec![],
    );
    let outer = el(
        "div",
        vec![
            ("on:click".into(), PropValue::Closure("outer_action".into())),
            ("width".into(), PropValue::Number(200.0)),
            ("height".into(), PropValue::Number(100.0)),
        ],
        vec![inner],
    );

    pipeline.build_render_scene(&outer, 800, 600);

    // Click inside inner → inner wins
    let action = pipeline.hit_test(10.0, 10.0).unwrap().to_string();
    dispatch_action(&action);
    assert_eq!(
        state::get::<String>("clicked".to_string(), String::new()),
        "inner"
    );

    // Click outside inner but inside outer → outer wins
    let action = pipeline.hit_test(10.0, 50.0).unwrap().to_string();
    dispatch_action(&action);
    assert_eq!(
        state::get::<String>("clicked".to_string(), String::new()),
        "outer"
    );
    clear_actions();
}

#[test]
fn app_mouse_move_tracking() {
    let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::<(f32, f32)>::new()));
    let events_clone = events.clone();

    let _app = App::new("Test").on_event(move |event| {
        if let AppEvent::MouseMove(x, y) = event {
            events_clone.lock().unwrap().push((*x, *y));
        }
    });

    // AppEvent::MouseMove should carry coordinates
    let event = AppEvent::MouseMove(123.0, 456.0);
    assert_eq!(event.name(), "mousemove");
}

#[test]
fn app_keyboard_event_name() {
    let event = AppEvent::KeyPress("KeyA".into());
    assert_eq!(event.name(), "keypress");

    let event = AppEvent::KeyRelease("KeyA".into());
    assert_eq!(event.name(), "keyrelease");
}

#[test]
fn app_focus_blur_event_names() {
    // Focus and blur are handled via state, not AppEvent, but we test the
    // state API integration
    reset();
    state::clear_element_state();

    assert!(!state::any_focused());
    state::set_focused(Some(42));
    assert!(state::is_focused(42));
    assert!(state::any_focused());
    assert!(!state::is_focused(99));

    state::set_focused(None);
    assert!(!state::any_focused());
}

#[test]
fn app_scroll_event_name() {
    let event = AppEvent::MouseScroll(0.0, 100.0);
    assert_eq!(event.name(), "mousescroll");
}

#[test]
fn app_event_target_identification() {
    reset();
    let mut pipeline = RenderPipeline::new().with_css(CSS);

    register_action("btn_action", || {});
    register_action("other_action", || {});

    let el = div(vec![
        clickable_button("btn_action", 120.0, 40.0),
        clickable_button("other_action", 120.0, 40.0),
    ]);
    pipeline.build_render_scene(&el, 800, 600);

    let targets = pipeline.hit_targets();
    assert_eq!(targets.len(), 2);
    // Both buttons have distinct actions
    assert_ne!(targets[0].action, targets[1].action);
    clear_actions();
}

#[test]
fn app_mouse_click_event_variant() {
    let event = AppEvent::MouseClick(MouseButton::Right);
    assert_eq!(event.name(), "mouseclick");

    let event = AppEvent::MouseRelease(MouseButton::Left);
    assert_eq!(event.name(), "mouserelease");
}

#[test]
fn app_resize_event_carries_dimensions() {
    let event = AppEvent::Resize(1920, 1080);
    assert_eq!(event.name(), "resize");
    if let AppEvent::Resize(w, h) = event {
        assert_eq!(w, 1920);
        assert_eq!(h, 1080);
    }
}

#[test]
fn app_close_event() {
    let event = AppEvent::Close;
    assert_eq!(event.name(), "close");
}

#[test]
fn app_click_outside_all_buttons_does_nothing() {
    reset();
    let mut pipeline = RenderPipeline::new().with_css(CSS);
    let el = clickable_button("some_action", 120.0, 40.0);
    pipeline.build_render_scene(&el, 800, 600);

    take_render_dirty();
    assert!(pipeline.hit_test(700.0, 550.0).is_none());
    assert!(!is_render_dirty());
}

// ══════════════════════════════════════════════════════════════════
//  State Integration (~10 tests)
// ══════════════════════════════════════════════════════════════════

#[test]
fn app_state_machine_abc() {
    reset();
    state::set("phase".to_string(), "A".to_string());
    assert_eq!(
        state::get::<String>("phase".to_string(), String::new()),
        "A"
    );

    state::set("phase".to_string(), "B".to_string());
    assert_eq!(
        state::get::<String>("phase".to_string(), String::new()),
        "B"
    );

    state::set("phase".to_string(), "C".to_string());
    assert_eq!(
        state::get::<String>("phase".to_string(), String::new()),
        "C"
    );
}

#[test]
fn app_derived_state_from_multiple_signals() {
    reset();
    state::set("x", 10i64);
    state::set("y", 20i64);

    let sum = state::get::<i64>("x", 0) + state::get::<i64>("y", 0);
    assert_eq!(sum, 30);

    state::set("x", 15i64);
    let sum = state::get::<i64>("x", 0) + state::get::<i64>("y", 0);
    assert_eq!(sum, 35);
}

#[test]
fn app_batch_state_updates() {
    reset();
    take_render_dirty();
    state::set("a", 0i64);
    state::set("b", 0i64);

    // Multiple rapid state changes
    for _ in 0..10 {
        state::set("a", state::get::<i64>("a", 0) + 1);
        state::set("b", state::get::<i64>("b", 0) + 2);
    }

    assert_eq!(state::get::<i64>("a", 0), 10);
    assert_eq!(state::get::<i64>("b", 0), 20);
    assert!(is_render_dirty());
}

#[test]
fn app_state_reset_and_restore() {
    reset();
    state::set("value".to_string(), "original".to_string());
    assert_eq!(
        state::get::<String>("value".to_string(), String::new()),
        "original"
    );

    // Modify
    state::set("value".to_string(), "modified".to_string());
    assert_eq!(
        state::get::<String>("value".to_string(), String::new()),
        "modified"
    );

    // Reset
    state::clear();
    assert_eq!(
        state::get::<String>("value".to_string(), "restored".to_string()),
        "restored"
    );
}

#[test]
fn app_component_state_isolation() {
    reset();
    // Two distinct state keys act as separate component states
    state::set("comp_a_count", 0i64);
    state::set("comp_b_count", 0i64);

    state::set("comp_a_count", 5i64);
    assert_eq!(state::get::<i64>("comp_a_count", 0), 5);
    assert_eq!(state::get::<i64>("comp_b_count", 0), 0);
}

#[test]
fn app_state_persists_across_renders() {
    reset();
    state::set("persist", 42i64);

    let mut pipeline = RenderPipeline::new().with_css(CSS);

    fn comp() -> Element {
        let v: i64 = state::get("persist", 0);
        register_action("noop", || {});
        div(vec![
            text(&v.to_string()),
            clickable_button("noop", 100.0, 40.0),
        ])
    }

    pipeline.build_render_scene(&comp(), 800, 600);
    assert!(rendered_text(&pipeline).contains(&"42".to_string()));

    // Re-render multiple times — state must persist
    for _ in 0..5 {
        pipeline.build_render_scene(&comp(), 800, 600);
    }
    assert!(rendered_text(&pipeline).contains(&"42".to_string()));
    assert_eq!(state::get::<i64>("persist", 0), 42);
    clear_actions();
}

#[test]
fn app_state_render_dirty_flag_toggling() {
    reset();
    take_render_dirty();
    assert!(!is_render_dirty());

    state::set("flag_test", 0i64);
    assert!(is_render_dirty());

    take_render_dirty();
    assert!(!is_render_dirty());

    state::set("flag_test", 1i64);
    assert!(is_render_dirty());
}

#[test]
fn app_state_handler_modifies_state_and_marks_dirty() {
    reset();
    take_render_dirty();

    register_action("inc", || {
        let c: i64 = state::get("counter", 0);
        state::set("counter", c + 1);
    });

    dispatch_action("inc");
    assert!(is_render_dirty());
    assert_eq!(state::get::<i64>("counter", 0), 1);

    dispatch_action("inc");
    assert_eq!(state::get::<i64>("counter", 0), 2);
    clear_actions();
}

#[test]
fn app_state_string_manipulation() {
    reset();
    state::set("message".to_string(), "Hello".to_string());
    let current = state::get::<String>("message".to_string(), String::new());
    state::set("message".to_string(), format!("{current} World"));
    assert_eq!(
        state::get::<String>("message".to_string(), String::new()),
        "Hello World"
    );
}

#[test]
fn app_state_boolean_toggle() {
    reset();
    state::set("visible", true);
    assert!(state::get::<bool>("visible", false));

    state::set("visible", false);
    assert!(!state::get::<bool>("visible", true));

    state::set("visible", true);
    assert!(state::get::<bool>("visible", false));
}

#[test]
fn app_app_builder_chain_all() {
    let comp = FnComponent::new(|| div(vec![text("Built")]));
    let _app = App::new("Chain Test")
        .with_size(1280, 720)
        .with_component(comp)
        .with_css("div { width: 100%; }")
        .on_event(|_| {})
        .open_window(
            "Second",
            640,
            480,
            FnComponent::new(|| div(vec![text("Second Win")])),
        );
    // Builder chain should complete without panicking
}

#[test]
fn app_event_handler_multiple_listeners() {
    let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let c1 = counter.clone();
    let c2 = counter.clone();
    let c3 = counter.clone();

    let _app = App::new("Test")
        .on_event(move |_| {
            c1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        })
        .on_event(move |_| {
            c2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        })
        .on_event(move |_| {
            c3.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
    // All three handlers registered — no panic
}
