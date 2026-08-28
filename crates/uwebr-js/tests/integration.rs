use uwebr_js::{transpile, transpile_to_module};

#[test]
fn test_basic_function() {
    let js = r#"
        function add(a, b) {
            return a + b;
        }
    "#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("fn add"));
    assert!(result.code.contains("a + b") || result.code.contains("a+b"));
}

#[test]
fn test_class_transpilation() {
    let js = r#"
        class Dog {
            constructor(name) {
                this.name = name;
            }
            bark() {
                return "Woof!";
            }
        }
    "#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("struct Dog"));
    assert!(result.code.contains("impl Dog"));
}

#[test]
fn test_async_function() {
    let js = r#"
        async function fetchData(url) {
            return await fetch(url);
        }
    "#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("async fn"));
}

#[test]
fn test_array_operations() {
    let js = r#"
        const arr = [1, 2, 3];
        const doubled = arr.map(x => x * 2);
    "#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("vec!["));
}

#[test]
fn test_module_mode() {
    let js = r#"
        function greet(name) {
            return "Hello, " + name;
        }
    "#;
    let result = transpile_to_module(js, "greetings").unwrap();
    assert!(result.code.contains("mod greetings"));
}

#[test]
fn test_if_else() {
    let js = r#"
        function isEven(n) {
            if (n % 2 === 0) {
                return true;
            } else {
                return false;
            }
        }
    "#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("if"));
    assert!(result.code.contains("else"));
}

#[test]
fn test_while_loop() {
    let js = r#"
        function countdown(n) {
            while (n > 0) {
                console.log(n);
                n = n - 1;
            }
        }
    "#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("while"));
}

#[test]
fn test_try_catch() {
    let js = r#"
        function riskyOperation() {
            try {
                return JSON.parse("invalid");
            } catch (e) {
                console.error(e);
                return null;
            }
        }
    "#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("Result"));
}

#[test]
fn test_for_loop() {
    let js = r#"
        function sum(n) {
            let total = 0;
            for (let i = 0; i < n; i++) {
                total = total + i;
            }
            return total;
        }
    "#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("while") || result.code.contains("for"));
}

#[test]
fn test_string_methods() {
    let js = r#"
        const s = "hello";
        const upper = s.toUpperCase();
        const lower = s.toLowerCase();
        const trimmed = s.trim();
        const has = s.includes("ell");
    "#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("to_uppercase"));
    assert!(result.code.contains("to_lowercase"));
    assert!(result.code.contains("trim()"));
    assert!(result.code.contains("contains("));
}

#[test]
fn test_object_spread() {
    let js = r#"
        const a = { x: 1 };
        const b = { y: 2 };
        const c = { ...a, ...b, z: 3 };
    "#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("HashMap::from_iter"));
}

#[test]
fn test_function_expression_to_closure() {
    let js = r#"
        const add = function(a, b) { return a + b; };
    "#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("|"));
    assert!(!result.code.contains("fn(a:"));
}

#[test]
fn test_for_of_loop() {
    let js = r#"
        function sum(arr) {
            let total = 0;
            for (const x of arr) {
                total = total + x;
            }
            return total;
        }
    "#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("for") || result.code.contains("iter"));
}
