use uwebr_core::component::{prop_bool, prop_string, Element, NodeType, PropValue};

/// A component that reads props the way transpiler-generated code does.
fn card_component(props: &[(String, PropValue)]) -> Element {
    let title = prop_string(props, "title");
    let disabled = prop_bool(props, "disabled");
    Element {
        node_type: NodeType::Element("div".into()),
        props: vec![("disabled".into(), PropValue::Bool(disabled))],
        children: vec![Element::text(&title)],
    }
}

#[test]
fn test_props_flow_to_component() {
    let props = vec![
        ("title".into(), PropValue::String("Test".into())),
        ("disabled".into(), PropValue::Bool(true)),
    ];
    let el = card_component(&props);

    assert_eq!(el.children[0].node_type, NodeType::Text("Test".into()));
    assert_eq!(el.props[0], ("disabled".into(), PropValue::Bool(true)));
}

#[test]
fn test_component_with_empty_props() {
    let el = card_component(&[]);
    // Missing props fall back to defaults.
    assert_eq!(el.children[0].node_type, NodeType::Text("".into()));
    assert_eq!(el.props[0], ("disabled".into(), PropValue::Bool(false)));
}
