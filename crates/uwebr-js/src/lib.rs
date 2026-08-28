pub mod analyzer;
pub mod codegen;
pub mod context;
pub mod parser;
pub mod transformer;
pub mod types;
pub mod utils;

use anyhow::Result;

pub use context::Context;
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

pub fn transpile(js_code: &str) -> Result<TranspileResult> {
    transpile_with_options(js_code, &TranspileOptions::default())
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
