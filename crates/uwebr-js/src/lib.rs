pub mod analyzer;
pub mod codegen;
pub mod context;
pub mod parser;
pub mod script;
pub mod transformer;
pub mod types;
pub mod utils;

use anyhow::Result;

pub use context::Context;
pub use script::ScriptState;
pub use types::*;

#[derive(Debug, Clone)]
pub struct TranspileOptions {
    pub module_name: Option<String>,
    pub use_serde: bool,
    pub use_tokio: bool,
    pub indent: usize,
}

impl Default for TranspileOptions {
    fn default() -> Self {
        Self {
            module_name: None,
            use_serde: true,
            use_tokio: true,
            indent: 4,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TranspileResult {
    pub code: String,
    pub imports: Vec<String>,
    pub warnings: Vec<String>,
}

/// Transpiled `<script>` block: Rust code plus the reactive state it declares.
#[derive(Debug, Clone)]
pub struct ScriptResult {
    pub code: String,
    pub warnings: Vec<String>,
    /// Top-level bindings lowered into `uwebr_core::state` accessors.
    pub states: Vec<ScriptState>,
    /// Names of top-level functions, used to bind `on:click={handler}`.
    pub functions: Vec<String>,
}

pub fn transpile(js_code: &str) -> Result<TranspileResult> {
    transpile_with_options(js_code, &TranspileOptions::default())
}

/// Transpile a `.uwebr` `<script>` block.
///
/// Unlike [`transpile`], top-level `let`/`const` bindings are lowered into
/// reactive accessor functions rather than emitted as module-scope `let`
/// statements, which Rust rejects.
pub fn transpile_script(js_code: &str) -> Result<ScriptResult> {
    let options = TranspileOptions::default();
    let module = parser::parse_js(js_code)?;
    let (ctx, _analysis_stmts) = analyzer::analyze(&module)?;
    let mut transformer = transformer::Transformer::with_context(ctx);
    let rust_ast = transformer.transform_module(&module)?;

    let (lowered, states) = script::lower_script_state(&rust_ast);

    let functions = lowered
        .items
        .iter()
        .filter_map(|item| match item {
            RsStmt::Fn(f) => Some(f.name.clone()),
            RsStmt::Pub(inner) => match &**inner {
                RsStmt::Fn(f) => Some(f.name.clone()),
                _ => None,
            },
            _ => None,
        })
        // Generated accessors are an implementation detail, not event handlers.
        .filter(|name| !name.starts_with("__state_") && !name.starts_with("__set_state_"))
        .collect();

    let generated = codegen::generate(&lowered, &options)?;

    Ok(ScriptResult {
        code: generated.code,
        warnings: generated.warnings,
        states,
        functions,
    })
}

pub fn transpile_with_options(
    js_code: &str,
    options: &TranspileOptions,
) -> Result<TranspileResult> {
    let module = parser::parse_js(js_code)?;
    let (ctx, _analysis_stmts) = analyzer::analyze(&module)?;
    let mut transformer = transformer::Transformer::with_context(ctx);
    let rust_ast = transformer.transform_module(&module)?;
    let code = codegen::generate(&rust_ast, options)?;
    Ok(code)
}

pub fn transpile_file(js_path: &str, rs_path: &str) -> Result<TranspileResult> {
    let js_code = std::fs::read_to_string(js_path)?;
    let result = transpile(&js_code)?;
    std::fs::write(rs_path, &result.code)?;
    Ok(result)
}

pub fn transpile_to_module(js_code: &str, module_name: &str) -> Result<TranspileResult> {
    let options = TranspileOptions {
        module_name: Some(module_name.to_string()),
        ..Default::default()
    };
    transpile_with_options(js_code, &options)
}
