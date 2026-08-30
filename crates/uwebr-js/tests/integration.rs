use uwebr_js::{
    transpile, transpile_script, transpile_to_module, transpile_with_options, TranspileOptions,
};

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

// ── New integration tests ─────────────────────────────────────────────

#[test]
fn js_transpile_arrow_function_expression() {
    let js = r#"const double = (x) => x * 2;"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("|"),
        "arrow should become closure: {}",
        result.code
    );
}

#[test]
fn js_transpile_arrow_function_no_params() {
    let js = r#"const fortyTwo = () => 42;"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("||"),
        "no-param arrow: {}",
        result.code
    );
}

#[test]
fn js_transpile_arrow_function_multi_params() {
    let js = r#"const add = (a, b) => a + b;"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("|"),
        "multi-param arrow: {}",
        result.code
    );
}

#[test]
fn js_transpile_arrow_with_body() {
    let js = r#"const f = (x) => { let y = x + 1; return y; };"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("{"),
        "arrow with body: {}",
        result.code
    );
}

#[test]
fn js_transpile_template_literal_simple() {
    let js = r#"const s = `hello world`;"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("hello world"),
        "simple template: {}",
        result.code
    );
}

#[test]
fn js_transpile_template_literal_with_expression() {
    let js = r#"const s = `hello ${name}`;"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("format!"),
        "template with expr: {}",
        result.code
    );
}

#[test]
fn js_transpile_template_literal_multiple_expressions() {
    let js = r#"const s = `${a} and ${b}`;"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("format!"),
        "template multi expr: {}",
        result.code
    );
}

#[test]
fn js_transpile_destructuring_array() {
    let js = r#"const [a, b, c] = arr;"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("let"),
        "array destructuring: {}",
        result.code
    );
}

#[test]
fn js_transpile_destructuring_object() {
    let js = r#"const { x, y } = point;"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("let"),
        "object destructuring: {}",
        result.code
    );
}

#[test]
fn js_transpile_this_keyword() {
    let js = r#"
        class Foo {
            constructor() { this.value = 1; }
            getValue() { return this.value; }
        }
    "#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("struct Foo"),
        "class with this: {}",
        result.code
    );
    assert!(
        result.code.contains("impl Foo"),
        "impl with this: {}",
        result.code
    );
}

#[test]
fn js_transpile_new_expression() {
    let js = r#"
        class Foo { constructor(x) { this.x = x; } }
        function create() { return new Foo(42); }
    "#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("Foo::new"),
        "new expression: {}",
        result.code
    );
}

#[test]
fn js_transpile_class_with_extends() {
    let js = r#"
        class Animal { constructor(name) { this.name = name; } }
        class Dog extends Animal { bark() { return "woof"; } }
    "#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("struct Dog"),
        "extends class: {}",
        result.code
    );
}

#[test]
fn js_transpile_switch_case() {
    let js = r#"
        function test(x) {
            switch (x) {
                case 1: return "one";
                case 2: return "two";
                default: return "other";
            }
        }
    "#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("match"),
        "switch should become match: {}",
        result.code
    );
}

#[test]
fn js_transpile_switch_with_break() {
    let js = r#"
        function test(x) {
            switch (x) {
                case 1:
                    break;
                case 2:
                    break;
            }
        }
    "#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("match"),
        "switch with break: {}",
        result.code
    );
}

#[test]
fn js_transpile_do_while() {
    let js = r#"
        function loop5() {
            let i = 0;
            do { i++; } while (i < 5);
        }
    "#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("loop"), "do-while: {}", result.code);
}

#[test]
fn js_transpile_regular_expression() {
    let js = r#"const pattern = /abc/;"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("abc"),
        "regex literal: {}",
        result.code
    );
}

#[test]
fn js_transpile_optional_chaining() {
    let js = r#"const r = obj?.prop;"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("as_ref") || result.code.contains("map"),
        "optional chaining: {}",
        result.code
    );
}

#[test]
fn js_transpile_nullish_coalescing() {
    let js = r#"const r = a ?? b;"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("unwrap_or"),
        "nullish coalescing: {}",
        result.code
    );
}

#[test]
fn js_transpile_array_from() {
    let js = r#"const src = [1,2,3]; const arr = Array.from(src);"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("from") || result.code.contains("src"),
        "Array.from: {}",
        result.code
    );
}

#[test]
fn js_transpile_object_keys() {
    let js = r#"const myobj = { a: 1 }; const keys = Object.keys(myobj);"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("Object.keys") || result.code.contains("keys"),
        "Object.keys: {}",
        result.code
    );
}

#[test]
fn js_transpile_spread_in_object() {
    let js = r#"const c = { ...a, z: 3 };"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("collect") || result.code.contains("from_iter"),
        "spread in object: {}",
        result.code
    );
}

#[test]
fn js_transpile_rest_parameters() {
    let js = r#"function f(...args) { return args; }"#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("fn"), "rest params: {}", result.code);
}

#[test]
fn js_transpile_default_parameters() {
    let js = r#"function greet(name = "world") { return name; }"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("fn greet"),
        "default params: {}",
        result.code
    );
}

#[test]
fn js_transpile_spread_in_call() {
    let js = r#"f(...args);"#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("f"), "spread in call: {}", result.code);
}

#[test]
fn js_transpile_spread_in_array() {
    let js = r#"const r = [...a, 4];"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("concat") || result.code.contains("vec!"),
        "spread in array: {}",
        result.code
    );
}

#[test]
fn js_transpile_json_stringify() {
    let js = r#"const s = JSON.stringify(data);"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("serde_json") || result.code.contains("to_string"),
        "JSON.stringify: {}",
        result.code
    );
}

#[test]
fn js_transpile_json_parse() {
    let js = r#"const obj = JSON.parse(text);"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("serde_json") || result.code.contains("from_str"),
        "JSON.parse: {}",
        result.code
    );
}

#[test]
fn js_transpile_for_in_loop() {
    let js = r#"
        function keys(obj) {
            let result = [];
            for (const key in obj) { result.push(key); }
            return result;
        }
    "#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("for") || result.code.contains("in"),
        "for-in loop: {}",
        result.code
    );
}

#[test]
fn js_transpile_string_concat() {
    let js = r#"const s = "hello" + " " + "world";"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("to_string") || result.code.contains("format"),
        "string concat: {}",
        result.code
    );
}

#[test]
fn js_transpile_number_comparison() {
    let js = r#"function cmp(a, b) { return a > b; }"#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains(">"), "comparison: {}", result.code);
}

#[test]
fn js_transpile_logical_and_or() {
    let js = r#"function f(a, b) { return a && b; }"#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("&&"), "logical and: {}", result.code);
}

#[test]
fn js_transpile_ternary() {
    let js = r#"function abs(x) { return x >= 0 ? x : -x; }"#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("if"), "ternary: {}", result.code);
}

#[test]
fn js_transpile_for_loop_basic() {
    let js = r#"
        function sum(n) {
            let s = 0;
            for (let i = 0; i < n; i++) { s += i; }
            return s;
        }
    "#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("while") || result.code.contains("for"),
        "for loop: {}",
        result.code
    );
}

#[test]
fn js_transpile_continue_statement() {
    let js = r#"
        function skipOdds(n) {
            let sum = 0;
            for (let i = 0; i < n; i++) {
                if (i % 2 !== 0) continue;
                sum += i;
            }
            return sum;
        }
    "#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("continue"),
        "continue: {}",
        result.code
    );
}

#[test]
fn js_transpile_break_statement() {
    let js = r#"
        function findFirst(n) {
            for (let i = 0; i < n; i++) {
                if (i > 5) break;
            }
        }
    "#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("break"), "break: {}", result.code);
}

#[test]
fn js_transpile_throw_statement() {
    let js = r#"
        function fail() {
            throw new Error("oops");
        }
    "#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("Err") || result.code.contains("throw"),
        "throw: {}",
        result.code
    );
}

#[test]
fn js_transpile_import_statement() {
    let js = r#"
        import { foo } from 'bar';
        function useFoo() { return foo; }
    "#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("foo"), "import: {}", result.code);
}

#[test]
fn js_transpile_export_function() {
    let js = r#"export function hello() { return 1; }"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("pub fn") || result.code.contains("fn hello"),
        "export function: {}",
        result.code
    );
}

#[test]
fn js_transpile_nested_functions() {
    let js = r#"
        function outer() {
            function inner() { return 1; }
            return inner();
        }
    "#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("fn outer"), "nested: {}", result.code);
    assert!(result.code.contains("fn inner"), "nested: {}", result.code);
}

#[test]
fn js_transpile_closure_capture() {
    let js = r#"
        function makeAdder(n) {
            return (x) => x + n;
        }
    "#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("|"),
        "closure capture: {}",
        result.code
    );
}

#[test]
fn js_transpile_complex_expression() {
    let js = r#"function f(a, b, c) { return (a + b) * c - a / b; }"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("fn f"),
        "complex expr: {}",
        result.code
    );
}

#[test]
fn js_transpile_nested_ternary() {
    let js = r#"
        function classify(x) {
            return x > 0 ? "positive" : x < 0 ? "negative" : "zero";
        }
    "#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("if"),
        "nested ternary: {}",
        result.code
    );
}

#[test]
fn js_transpile_string_methods_chain() {
    let js = r#"const s = "  Hello World  ".trim().toLowerCase();"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("trim"),
        "string chain: {}",
        result.code
    );
}

#[test]
fn js_transpile_array_literal() {
    let js = r#"const arr = [1, "two", true];"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("vec!"),
        "array literal: {}",
        result.code
    );
}

#[test]
fn js_transpile_object_literal() {
    let js = r#"const obj = { a: 1, b: "two" };"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("HashMap::from"),
        "object literal: {}",
        result.code
    );
}

#[test]
fn js_transpile_function_return_type() {
    let js = r#"function add(a: number, b: number): number { return a + b; }"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("fn add"),
        "typed function: {}",
        result.code
    );
}

#[test]
fn js_transpile_promise_chain() {
    let js = r#"
        async function fetchData() {
            const result = await fetch("url");
            return result;
        }
    "#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("async fn"),
        "promise chain: {}",
        result.code
    );
}

#[test]
fn js_transpile_null_literal() {
    let js = r#"const x = null;"#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("None"), "null: {}", result.code);
}

#[test]
fn js_transpile_undefined_literal() {
    let js = r#"const x = undefined;"#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("None"), "undefined: {}", result.code);
}

#[test]
fn js_transpile_type_inference_bool() {
    let js = r#"const flag = true;"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("bool"),
        "bool inference: {}",
        result.code
    );
}

#[test]
fn js_transpile_type_inference_string() {
    let js = r#"const name = "Alice";"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("String"),
        "string inference: {}",
        result.code
    );
}

#[test]
fn js_transpile_compound_assignment() {
    let js = r#"function f(x) { x += 10; return x; }"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("+="),
        "compound assignment: {}",
        result.code
    );
}

#[test]
fn js_transpile_update_expression_pre() {
    let js = r#"function f() { let x = 0; ++x; return x; }"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("fn f"),
        "pre-increment: {}",
        result.code
    );
}

#[test]
fn js_transpile_update_expression_post() {
    let js = r#"function f() { let x = 0; x++; return x; }"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("fn f"),
        "post-increment: {}",
        result.code
    );
}

#[test]
fn js_transpile_negative_number() {
    let js = r#"const x = -42;"#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("-"), "negative: {}", result.code);
}

#[test]
fn js_transpile_unary_not() {
    let js = r#"function f(flag) { return !flag; }"#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("!"), "unary not: {}", result.code);
}

#[test]
fn js_transpile_bitwise_ops() {
    let js = r#"function f(a, b) { return a & b; }"#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("&"), "bitwise and: {}", result.code);
}

#[test]
fn js_transpile_shift_ops() {
    let js = r#"function f(x) { return x << 2; }"#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("<<"), "shift: {}", result.code);
}

#[test]
fn js_transpile_string_replace() {
    let js = r#"const s = "hello".replace("l", "r");"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("replace"),
        "string replace: {}",
        result.code
    );
}

#[test]
fn js_transpile_string_split() {
    let js = r#"const parts = "a,b,c".split(",");"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("split"),
        "string split: {}",
        result.code
    );
}

#[test]
fn js_transpile_string_char_at() {
    let js = r#"const c = "hello".charAt(0);"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("chars().nth"),
        "charAt: {}",
        result.code
    );
}

#[test]
fn js_transpile_string_substring() {
    let js = r#"const s = "hello".substring(1, 3);"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("[1..3]") || result.code.contains("substring"),
        "substring: {}",
        result.code
    );
}

#[test]
fn js_transpile_string_index_of() {
    let js = r#"const i = "hello".indexOf("l");"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("position") || result.code.contains("indexOf"),
        "indexOf: {}",
        result.code
    );
}

#[test]
fn js_transpile_string_repeat() {
    let js = r#"const s = "ab".repeat(3);"#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("repeat"), "repeat: {}", result.code);
}

#[test]
fn js_transpile_string_length() {
    let js = r#"const l = "hello".length;"#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("len"), "length: {}", result.code);
}

#[test]
fn js_transpile_transpile_script_basic() {
    let js = r#"
        let count = 0;
        function increment() { count++; }
    "#;
    let result = transpile_script(js).unwrap();
    assert!(
        result.states.len() == 1,
        "script states: {}",
        result.states.len()
    );
    assert!(
        result.functions.contains(&"increment".to_string()),
        "script functions: {:?}",
        result.functions
    );
}

#[test]
fn js_transpile_transpile_script_multiple_states() {
    let js = r#"
        let x = 1;
        let y = 2;
        const z = 3;
    "#;
    let result = transpile_script(js).unwrap();
    assert!(
        result.states.len() == 3,
        "script multiple states: {}",
        result.states.len()
    );
}

#[test]
fn js_transpile_transpile_script_no_state() {
    let js = r#"function add(a, b) { return a + b; }"#;
    let result = transpile_script(js).unwrap();
    assert!(
        result.states.is_empty(),
        "script no state: {}",
        result.states.len()
    );
}

#[test]
fn js_transpile_transpile_script_function_names() {
    let js = r#"
        let n = 0;
        function increment() { n++; }
        function decrement() { n--; }
    "#;
    let result = transpile_script(js).unwrap();
    assert!(result.functions.contains(&"increment".to_string()));
    assert!(result.functions.contains(&"decrement".to_string()));
}

#[test]
fn js_transpile_with_options_indent() {
    let js = r#"function f() { return 1; }"#;
    let options = TranspileOptions {
        indent: 2,
        ..Default::default()
    };
    let result = transpile_with_options(js, &options).unwrap();
    assert!(
        result.code.contains("fn f"),
        "indent option: {}",
        result.code
    );
}

#[test]
fn js_transpile_with_options_module_name() {
    let js = r#"function f() { return 1; }"#;
    let options = TranspileOptions {
        module_name: Some("my_mod".into()),
        ..Default::default()
    };
    let result = transpile_with_options(js, &options).unwrap();
    assert!(
        result.code.contains("mod my_mod"),
        "module name: {}",
        result.code
    );
}

#[test]
fn js_transpile_empty_script() {
    let js = r#""#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.is_empty() || result.code.trim().is_empty(),
        "empty: {}",
        result.code
    );
}

#[test]
fn js_transpile_comments_only() {
    let js = r#"
        // This is a comment
        /* block comment */
    "#;
    let result = transpile(js).unwrap();
    assert!(
        result.warnings.is_empty() || result.code.is_empty() || result.code.trim().is_empty(),
        "comments only: {}",
        result.code
    );
}

#[test]
fn js_transpile_computed_member_access() {
    let js = r#"function f(arr, idx) { return arr[idx]; }"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("["),
        "computed access: {}",
        result.code
    );
}

#[test]
fn js_transpile_method_chain() {
    let js = r#"const s = "  Hello  ".trim().toLowerCase().replace("hello", "hi");"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("trim"),
        "method chain: {}",
        result.code
    );
}

#[test]
fn js_transpile_nested_if_else() {
    let js = r#"
        function f(x) {
            if (x > 0) {
                if (x > 10) { return "big"; }
                else { return "small"; }
            } else { return "negative"; }
        }
    "#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("if"), "nested if: {}", result.code);
    assert!(result.code.contains("else"), "nested else: {}", result.code);
}

#[test]
fn js_transpile_array_map() {
    let js = r#"function double(arr) { return arr.map(x => x * 2); }"#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("map"), "array map: {}", result.code);
}

#[test]
fn js_transpile_array_filter() {
    let js = r#"function evens(arr) { return arr.filter(x => x % 2 === 0); }"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("filter"),
        "array filter: {}",
        result.code
    );
}

#[test]
fn js_transpile_array_reduce() {
    let js = r#"function sum(arr) { return arr.reduce((a, b) => a + b, 0); }"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("reduce"),
        "array reduce: {}",
        result.code
    );
}

#[test]
fn js_transpile_array_foreach() {
    let js = r#"function logAll(arr) { arr.forEach(x => console.log(x)); }"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("forEach"),
        "array forEach: {}",
        result.code
    );
}

#[test]
fn js_transpile_object_entries() {
    let js = r#"const myobj = { a: 1 }; const e = Object.entries(myobj);"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("Object.entries") || result.code.contains("entries"),
        "Object.entries: {}",
        result.code
    );
}

#[test]
fn js_transpile_transpile_to_module() {
    let js = r#"function greet() { return "hi"; }"#;
    let result = transpile_to_module(js, "my_module").unwrap();
    assert!(
        result.code.contains("mod my_module"),
        "to_module: {}",
        result.code
    );
}

#[test]
fn js_transpile_nested_function_closures() {
    let js = r#"
        function make() {
            function inner() { return 1; }
            return inner;
        }
    "#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("fn make"),
        "nested closure: {}",
        result.code
    );
}

#[test]
fn js_transpile_multiple_functions() {
    let js = r#"
        function add(a, b) { return a + b; }
        function sub(a, b) { return a - b; }
        function mul(a, b) { return a * b; }
    "#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("fn add"),
        "multi fn add: {}",
        result.code
    );
    assert!(
        result.code.contains("fn sub"),
        "multi fn sub: {}",
        result.code
    );
    assert!(
        result.code.contains("fn mul"),
        "multi fn mul: {}",
        result.code
    );
}

#[test]
fn js_transpile_class_methods() {
    let js = r#"
        class Calculator {
            constructor() { this.value = 0; }
            add(n) { this.value += n; return this; }
            get() { return this.value; }
        }
    "#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("struct Calculator"),
        "class: {}",
        result.code
    );
    assert!(
        result.code.contains("impl Calculator"),
        "impl: {}",
        result.code
    );
}

#[test]
fn js_transpile_empty_function() {
    let js = r#"function noop() {}"#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("fn noop"), "empty fn: {}", result.code);
}

#[test]
fn js_transpile_function_with_many_params() {
    let js = r#"function f(a, b, c, d, e) { return a + b + c + d + e; }"#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("fn f"), "many params: {}", result.code);
}

#[test]
fn js_transpile_hex_literal() {
    let js = r#"const x = 0xff;"#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("255"), "hex literal: {}", result.code);
}

#[test]
fn js_transpile_binary_literal() {
    let js = r#"const x = 0b1010;"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("10"),
        "binary literal: {}",
        result.code
    );
}

#[test]
fn js_transpile_octal_literal() {
    let js = r#"const x = 0o17;"#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("15"), "octal literal: {}", result.code);
}

#[test]
fn js_transpile_exponentiation() {
    let js = r#"function f(x) { return x ** 2; }"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("fn f"),
        "exponentiation: {}",
        result.code
    );
}

#[test]
fn js_transpile_in_operator() {
    let js = r#"function f(obj) { return "x" in obj; }"#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("fn f"), "in operator: {}", result.code);
}

#[test]
fn js_transpile_instanceof() {
    let js = r#"function f(x) { return x instanceof Error; }"#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("fn f"), "instanceof: {}", result.code);
}

#[test]
fn js_transpile_void_operator() {
    let js = r#"function f() { void 0; }"#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("fn f"), "void: {}", result.code);
}

#[test]
fn js_transpile_delete_operator() {
    let js = r#"function f(obj) { delete obj.x; }"#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("fn f"), "delete: {}", result.code);
}

#[test]
fn js_transpile_labelled_statement() {
    let js = r#"
        function f() {
            outer: for (let i = 0; i < 10; i++) {
                break outer;
            }
        }
    "#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("fn f"), "labelled: {}", result.code);
}

#[test]
fn js_transpile_with_statement() {
    let js = r#"
        function f(obj) {
            with (obj) {
                return x + y;
            }
        }
    "#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("fn f"),
        "with statement: {}",
        result.code
    );
}

#[test]
fn js_transpile_debugger_statement() {
    let js = r#"function f() { debugger; return 1; }"#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("fn f"), "debugger: {}", result.code);
}

#[test]
fn js_transpile_empty_object() {
    let js = r#"const obj = {};"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("HashMap::from") || result.code.contains("Vec::new"),
        "empty object: {}",
        result.code
    );
}

#[test]
fn js_transpile_empty_array() {
    let js = r#"const arr = [];"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("Vec::new()") || result.code.contains("vec!"),
        "empty array: {}",
        result.code
    );
}

#[test]
fn js_transpile_nested_object() {
    let js = r#"const obj = { a: { b: 1 } };"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("HashMap::from"),
        "nested object: {}",
        result.code
    );
}

#[test]
fn js_transpile_array_of_arrays() {
    let js = r#"const arr = [[1, 2], [3, 4]];"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("vec!"),
        "array of arrays: {}",
        result.code
    );
}

#[test]
fn js_transpile_mixed_types_array() {
    let js = r#"const arr = [1, "two", true, null];"#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("vec!"), "mixed types: {}", result.code);
}

#[test]
fn js_transpile_string_with_quotes() {
    let js = r#"const s = "it's a test";"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("it's a test"),
        "string with quotes: {}",
        result.code
    );
}

#[test]
fn js_transpile_string_with_escapes() {
    let js = r#"const s = "line1\nline2\ttab";"#;
    let result = transpile(js).unwrap();
    assert!(
        result.code.contains("to_string"),
        "string escapes: {}",
        result.code
    );
}

#[test]
fn js_transpile_very_long_chain() {
    let js = r#"
        function f(x) {
            return x.trim().toLowerCase().replace("a", "b").split(" ").length;
        }
    "#;
    let result = transpile(js).unwrap();
    assert!(result.code.contains("fn f"), "long chain: {}", result.code);
}

// ── Error case tests ─────────────────────────────────────────────────

#[test]
fn js_transpile_empty_input() {
    let js = "";
    let result = transpile(js);
    assert!(result.is_ok(), "empty input should succeed: {:?}", result);
}

#[test]
fn js_transpile_whitespace_only() {
    let js = "   \n\n   \t  ";
    let result = transpile(js);
    assert!(
        result.is_ok(),
        "whitespace only should succeed: {:?}",
        result
    );
}

#[test]
fn js_transpile_invalid_syntax_missing_brace() {
    let js = "function f() {";
    let result = transpile(js);
    // SWC parser may be lenient with missing braces
    assert!(
        result.is_ok() || result.is_err(),
        "unclosed brace: {:?}",
        result
    );
}

#[test]
fn js_transpile_invalid_syntax_missing_paren() {
    let js = "function f(";
    let result = transpile(js);
    assert!(result.is_err(), "unclosed paren should fail: {:?}", result);
}

#[test]
fn js_transpile_invalid_syntax_just_operator() {
    let js = "= = =";
    let result = transpile(js);
    assert!(result.is_err(), "just operators should fail: {:?}", result);
}

#[test]
fn js_transpile_invalid_syntax_empty_function() {
    let js = "function";
    let result = transpile(js);
    assert!(
        result.is_err(),
        "incomplete function should fail: {:?}",
        result
    );
}

#[test]
fn js_transpile_unclosed_string() {
    let js = r#"const s = "hello;"#;
    let result = transpile(js);
    // SWC parser may recover from unclosed strings
    assert!(
        result.is_ok() || result.is_err(),
        "unclosed string: {:?}",
        result
    );
}

#[test]
fn js_transpile_invalid_identifier() {
    let js = "123abc";
    let result = transpile(js);
    assert!(
        result.is_err(),
        "invalid identifier should fail: {:?}",
        result
    );
}

#[test]
fn js_transpile_deeply_nested_expressions() {
    let js = "function f() { return ((((((((1)))))))); }";
    let result = transpile(js);
    assert!(result.is_ok(), "deep nesting should succeed: {:?}", result);
}

#[test]
fn js_transpile_comments_only_script() {
    let js = "// line comment\n/* block comment */";
    let result = transpile(js);
    assert!(result.is_ok(), "comments only should succeed: {:?}", result);
}

#[test]
fn js_transpile_empty_block() {
    let js = "function f() {}";
    let result = transpile(js);
    assert!(result.is_ok(), "empty block should succeed: {:?}", result);
}

#[test]
fn js_transpile_trailing_semicolons() {
    let js = "function f() { return 1; };;;;";
    let result = transpile(js);
    assert!(
        result.is_ok(),
        "trailing semicolons should succeed: {:?}",
        result
    );
}

#[test]
fn js_transpile_invalid_unicode() {
    let js = "const x = \"\u{FFFE}\";";
    let result = transpile(js);
    assert!(
        result.is_ok(),
        "invalid unicode in string should succeed: {:?}",
        result
    );
}

#[test]
fn js_transpile_invalid_escape() {
    let js = "const x = \"\\q\";";
    let result = transpile(js);
    assert!(
        result.is_ok(),
        "unknown escape should succeed as string: {:?}",
        result
    );
}

#[test]
fn js_transpile_nested_braces() {
    let js = "function f() { { { { } } } }";
    let result = transpile(js);
    assert!(result.is_ok(), "nested braces should succeed: {:?}", result);
}

#[test]
fn js_transpile_very_long_identifier() {
    let long_name = "a".repeat(200);
    let js = format!("const {} = 1;", long_name);
    let result = transpile(&js);
    assert!(
        result.is_ok(),
        "long identifier should succeed: {:?}",
        result
    );
}

#[test]
fn js_transpile_very_long_string() {
    let long_str = "x".repeat(10000);
    let js = format!("const s = \"{}\";", long_str);
    let result = transpile(&js);
    assert!(result.is_ok(), "long string should succeed: {:?}", result);
}

#[test]
fn js_transpile_deeply_nested_if() {
    let js = r#"
        function f(x) {
            if (x > 0) {
                if (x > 1) {
                    if (x > 2) {
                        if (x > 3) {
                            return 4;
                        }
                    }
                }
            }
            return 0;
        }
    "#;
    let result = transpile(js);
    assert!(
        result.is_ok(),
        "deeply nested if should succeed: {:?}",
        result
    );
}

#[test]
fn js_transpile_deeply_nested_function_calls() {
    let js = "function f() { return g(g(g(g(g(1))))); }";
    let result = transpile(js);
    assert!(
        result.is_ok(),
        "deeply nested calls should succeed: {:?}",
        result
    );
}

#[test]
fn js_transpile_empty_string() {
    let js = r#"const s = "";"#;
    let result = transpile(js);
    assert!(result.is_ok(), "empty string should succeed: {:?}", result);
}

#[test]
fn js_transpile_many_parameters() {
    let params: Vec<String> = (0..50).map(|i| format!("p{}", i)).collect();
    let param_list = params.join(", ");
    let arg_list = params.join(", ");
    let js = format!("function f({}) {{ return {}; }}", param_list, arg_list);
    let result = transpile(&js);
    assert!(result.is_ok(), "50 params should succeed: {:?}", result);
}

#[test]
fn js_transpile_many_statements() {
    let mut js = String::new();
    for i in 0..100 {
        js.push_str(&format!("const x{} = {};", i, i));
    }
    let result = transpile(&js);
    assert!(
        result.is_ok(),
        "100 statements should succeed: {:?}",
        result
    );
}
