use uwebr_macro::{component, Props};

#[derive(Props)]
struct ButtonProps {
    label: String,
    disabled: bool,
}

#[test]
fn test_props_builder() {
    let props = ButtonProps::builder()
        .label("Click me".to_string())
        .disabled(false)
        .build()
        .unwrap();

    assert_eq!(props.label, "Click me");
    assert!(!props.disabled);
}

#[test]
fn test_props_missing_required() {
    let result = ButtonProps::builder().build();
    assert!(result.is_err());
}

#[test]
fn test_props_partial_build() {
    let result = ButtonProps::builder()
        .label("Only label".to_string())
        .build();
    match result {
        Err(e) => assert!(e.contains("disabled")),
        Ok(_) => panic!("Expected error for missing prop"),
    }
}

#[component]
fn my_component(name: String) -> String {
    format!("Hello, {}!", name)
}

#[test]
fn test_component_macro() {
    let result = my_component("World".to_string());
    assert_eq!(result, "Hello, World!");
}

#[component]
fn multi_param_component(a: String, b: i32) -> String {
    format!("{}-{}", a, b)
}

#[test]
fn test_component_macro_multiple_params() {
    let result = multi_param_component("test".to_string(), 42);
    assert_eq!(result, "test-42");
}
