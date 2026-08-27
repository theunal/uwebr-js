use uwebr_js::{transpile, transpile_to_module};

fn main() {
    let js_code = r#"
        function fibonacci(n) {
            if (n <= 1) return n;
            return fibonacci(n - 1) + fibonacci(n - 2);
        }

        function add(a, b) {
            return a + b;
        }

        class Calculator {
            constructor(value) {
                this.value = value;
            }

            add(x) {
                return this.value + x;
            }

            subtract(x) {
                return this.value - x;
            }
        }

        async function fetchData(url) {
            try {
                const response = await fetch(url);
                const data = await response.json();
                return data;
            } catch (e) {
                console.error(e);
                return null;
            }
        }

        const numbers = [1, 2, 3, 4, 5];
        const doubled = numbers.map(n => n * 2);
        const evens = numbers.filter(n => n % 2 === 0);
        const sum = numbers.reduce((acc, n) => acc + n, 0);
    "#;

    println!("=== Basic Transpilation ===");
    match transpile(js_code) {
        Ok(result) => {
            println!("{}", result.code);
            println!("\nWarnings:");
            for w in &result.warnings {
                println!("  - {}", w);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }

    println!("\n=== Module Transpilation ===");
    match transpile_to_module(js_code, "math_utils") {
        Ok(result) => {
            println!("{}", result.code);
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
