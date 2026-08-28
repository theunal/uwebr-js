//! Diagnostic: print the Rust produced for the `uwebr init` scaffold template.
//!
//! Run with `cargo run -p uwebr-cli --example scaffold_output`.

fn main() {
    let input = r#"<div class="app">
  <h1>Hello from uwebr!</h1>
</div>

<script>
  let count = 0;

  function increment() {
    count++;
  }
</script>

<style>
  .app {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100vh;
    background-color: #1a1a2e;
    color: #e0e0e0;
  }

  h1 {
    font-size: 2rem;
  }
</style>
"#;

    match uwebr_cli::transpiler::transpile(input, "App") {
        Ok(code) => println!("{code}"),
        Err(e) => eprintln!("transpile failed: {e}"),
    }
}
