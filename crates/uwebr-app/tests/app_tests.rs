use uwebr_app::{App, AppEvent, FnComponent};
use uwebr_app::component::Component;
use uwebr_core::component::{Element, NodeType, PropValue};
use winit::event::MouseButton;

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
