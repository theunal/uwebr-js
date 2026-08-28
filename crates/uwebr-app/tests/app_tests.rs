use uwebr_app::{App, AppEvent, FnComponent, RenderPipeline};
use uwebr_app::component::Component;
use uwebr_core::component::{Element, NodeType, PropValue};
use uwebr_core::timer::{TimerRegistry, set_timeout, set_interval, cancel_timer, request_animation_frame};
use winit::event::MouseButton;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[test]
fn test_app_creation() {
    let _app = App::new("Test");
}

#[test]
fn test_app_with_size() {
    let _app = App::new("Test").with_size(1024, 768);
}

#[test]
fn test_app_default() {
    let _app = App::default();
}

#[test]
fn test_fn_component() {
    let comp = FnComponent::new(|| Element {
        node_type: NodeType::Element("div".to_string()),
        props: vec![],
        children: vec![],
    });
    let el = comp.render();
    assert!(matches!(el.node_type, NodeType::Element(ref tag) if tag == "div"));
}

#[test]
fn test_fn_component_with_props() {
    let comp = FnComponent::new(|| Element {
        node_type: NodeType::Element("button".to_string()),
        props: vec![
            ("width".to_string(), PropValue::Number(100.0)),
            ("height".to_string(), PropValue::Number(40.0)),
        ],
        children: vec![Element {
            node_type: NodeType::Text("Click me".to_string()),
            props: vec![],
            children: vec![],
        }],
    });
    let el = comp.render();
    assert_eq!(el.props.len(), 2);
    assert_eq!(el.children.len(), 1);
}

#[test]
fn test_app_event_names() {
    assert_eq!(AppEvent::Resize(800, 600).name(), "resize");
    assert_eq!(AppEvent::Close.name(), "close");
    assert_eq!(AppEvent::KeyPress("a".into()).name(), "keypress");
    assert_eq!(AppEvent::MouseClick(MouseButton::Left).name(), "mouseclick");
}

#[test]
fn test_app_with_event_handler() {
    let _app = App::new("Test").on_event(|_event| {});
}

#[test]
fn test_app_with_component() {
    let comp = FnComponent::new(|| Element {
        node_type: NodeType::Element("div".to_string()),
        props: vec![],
        children: vec![],
    });
    let _app = App::new("Test").with_component(comp);
}

#[test]
fn test_pipeline_integration() {
    let mut pipeline = RenderPipeline::new();
    let el = Element {
        node_type: NodeType::Element("div".into()),
        props: vec![
            ("width".into(), PropValue::Number(400.0)),
            ("height".into(), PropValue::Number(300.0)),
            ("bg".into(), PropValue::String("#ff0000".into())),
        ],
        children: vec![Element {
            node_type: NodeType::Text("Hello World".into()),
            props: vec![],
            children: vec![],
        }],
    };
    let _scene = pipeline.render(&el, 800, 600);
}

#[test]
fn test_pipeline_nested_components() {
    let mut pipeline = RenderPipeline::new();
    let el = Element {
        node_type: NodeType::Element("div".into()),
        props: vec![("bg".into(), PropValue::String("black".into()))],
        children: vec![
            Element {
                node_type: NodeType::Element("div".into()),
                props: vec![("bg".into(), PropValue::String("red".into()))],
                children: vec![Element {
                    node_type: NodeType::Text("Nested".into()),
                    props: vec![],
                    children: vec![],
                }],
            },
        ],
    };
    let _scene = pipeline.render(&el, 800, 600);
}

// ── Multi-window tests ───────────────────────────────────────

#[test]
fn test_app_multi_window_fields() {
    let app = App::new("Test");
    assert_eq!(app.window_count(), 0);
}

#[test]
fn test_app_open_window_pending() {
    let app = App::new("Main")
        .open_window("Child", 400, 300, FnComponent::new(|| Element {
            node_type: NodeType::Element("div".into()),
            props: vec![],
            children: vec![],
        }));
    assert_eq!(app.pending_window_count(), 1);
}

#[test]
fn test_app_multiple_pending_windows() {
    let app = App::new("Main")
        .open_window("Win1", 400, 300, FnComponent::new(|| Element {
            node_type: NodeType::Element("div".into()),
            props: vec![],
            children: vec![],
        }))
        .open_window("Win2", 600, 400, FnComponent::new(|| Element {
            node_type: NodeType::Element("span".into()),
            props: vec![],
            children: vec![],
        }));
    assert_eq!(app.pending_window_count(), 2);
}

#[test]
fn test_app_primary_window_before_run() {
    let app = App::new("Test");
    assert!(app.primary_window().is_none());
}

#[test]
fn test_app_window_count_before_run() {
    let app = App::new("Test");
    assert_eq!(app.window_count(), 0);
}

// ── Timer tests ──────────────────────────────────────────────

#[test]
fn test_timer_registry_create() {
    let r = TimerRegistry::new();
    assert_eq!(r.pending_count(), 0);
}

#[test]
fn test_set_timeout_schedules() {
    let r = TimerRegistry::new();
    let _h = r.set_timeout(|| {}, Duration::from_secs(1));
    assert_eq!(r.pending_count(), 1);
}

#[test]
fn test_set_interval_schedules() {
    let r = TimerRegistry::new();
    let _h = r.set_interval(|| {}, Duration::from_millis(100));
    assert_eq!(r.pending_count(), 1);
}

#[test]
fn test_cancel_removes() {
    let r = TimerRegistry::new();
    let h = r.set_timeout(|| {}, Duration::from_secs(10));
    r.cancel(h);
    assert_eq!(r.pending_count(), 0);
}

#[test]
fn test_tick_fires_expired() {
    let r = TimerRegistry::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let c = counter.clone();

    let _h = r.set_timeout(move || {
        c.fetch_add(1, Ordering::SeqCst);
    }, Duration::from_millis(0));

    r.tick();
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[test]
fn test_interval_reschedules() {
    let r = TimerRegistry::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let c = counter.clone();

    let _h = r.set_interval(move || {
        c.fetch_add(1, Ordering::SeqCst);
    }, Duration::from_millis(0));

    r.tick();
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    assert_eq!(r.pending_count(), 1);
}

#[test]
fn test_animation_frame_fires() {
    let r = TimerRegistry::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let c = counter.clone();

    let _h = r.request_animation_frame(move || {
        c.fetch_add(1, Ordering::SeqCst);
    });

    r.fire_animation_frames();
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[test]
fn test_global_set_timeout() {
    let h = set_timeout(|| {}, Duration::from_secs(1));
    cancel_timer(h);
}

#[test]
fn test_global_set_interval() {
    let h = set_interval(|| {}, Duration::from_millis(50));
    cancel_timer(h);
}

#[test]
fn test_global_request_animation_frame() {
    let _h = request_animation_frame(|| {});
}

#[test]
fn test_handle_equality() {
    let r = TimerRegistry::new();
    let h1 = r.set_timeout(|| {}, Duration::from_secs(1));
    let h2 = r.set_timeout(|| {}, Duration::from_secs(1));
    assert_ne!(h1, h2);
    assert_eq!(h1, h1);
}
