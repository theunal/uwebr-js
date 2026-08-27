pub mod expressions;
pub mod functions;
pub mod rs_types;

use crate::types::*;
use crate::TranspileOptions;

pub struct CodeGen {
    output: String,
    indent_level: usize,
    indent_str: String,
    options: TranspileOptions,
}

impl CodeGen {
    pub fn new(options: &TranspileOptions) -> Self {
        Self {
            output: String::new(),
            indent_level: 0,
            indent_str: " ".repeat(options.indent),
            options: options.clone(),
        }
    }
    pub fn indent(&mut self) {
        self.indent_level += 1;
    }
    pub fn dedent(&mut self) {
        if self.indent_level > 0 {
            self.indent_level -= 1;
        }
    }
    pub fn write_indent(&mut self) {
        for _ in 0..self.indent_level {
            self.output.push_str(&self.indent_str);
        }
    }
    pub fn writeln(&mut self, s: &str) {
        self.write_indent();
        self.output.push_str(s);
        self.output.push('\n');
    }
    pub fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }

    pub fn generate_module(&mut self, module: &RustModule) -> String {
        if let Some(ref module_name) = self.options.module_name {
            self.writeln(&format!("mod {} {{", module_name));
            self.indent();
        }
        for item in &module.items {
            self.generate_stmt(item);
            self.output.push('\n');
        }
        if self.options.module_name.is_some() {
            self.dedent();
            self.writeln("}");
        }
        self.output.clone()
    }

    pub fn generate_stmt(&mut self, stmt: &RsStmt) {
        match stmt {
            RsStmt::Fn(func) => self.generate_function(func),
            RsStmt::Struct(def) => self.generate_struct(def),
            RsStmt::Enum(def) => self.generate_enum(def),
            RsStmt::Impl(impl_def) => self.generate_impl(impl_def),
            RsStmt::Trait(trait_def) => self.generate_trait(trait_def),
            RsStmt::Let(name, ty, init) => {
                self.write_indent();
                self.write(&format!("let {}: {} = ", name, ty.to_rust_string()));
                self.generate_expr(init);
                self.write(";\n");
            }
            RsStmt::LetMut(name, ty, init) => {
                self.write_indent();
                self.write(&format!("let mut {}: {} = ", name, ty.to_rust_string()));
                self.generate_expr(init);
                self.write(";\n");
            }
            RsStmt::Expr(expr) => {
                self.write_indent();
                self.generate_expr(expr);
                self.write(";\n");
            }
            RsStmt::Return(Some(expr)) => {
                self.write_indent();
                self.write("return ");
                self.generate_expr(expr);
                self.write(";\n");
            }
            RsStmt::Return(None) => {
                self.writeln("return;");
            }
            RsStmt::If(test, cons, alt) => {
                self.write_indent();
                self.write("if ");
                self.generate_expr(test);
                self.writeln(" {");
                self.indent();
                for s in cons {
                    self.generate_stmt(s);
                }
                self.dedent();
                if let Some(alt_stmts) = alt {
                    self.writeln("} else {");
                    self.indent();
                    for s in alt_stmts {
                        self.generate_stmt(s);
                    }
                    self.dedent();
                    self.writeln("}");
                } else {
                    self.writeln("}");
                }
            }
            RsStmt::While(test, body) => {
                self.write_indent();
                self.write("while ");
                self.generate_expr(test);
                self.write(" {\n");
                self.indent();
                for s in body {
                    self.generate_stmt(s);
                }
                self.dedent();
                self.write_indent();
                self.writeln("}");
            }
            RsStmt::For(name, test, body) => {
                self.write_indent();
                self.write(&format!("for {} in 0..", name));
                self.generate_expr(test);
                self.writeln(" {");
                self.indent();
                for s in body {
                    self.generate_stmt(s);
                }
                self.dedent();
                self.writeln("}");
            }
            RsStmt::ForLoop { init, test, update, body } => {
                self.write_indent();
                self.writeln("{");
                self.indent();
                if let Some(init_stmt) = init {
                    self.generate_stmt(init_stmt);
                }
                if let Some(test_expr) = test {
                    self.write_indent();
                    self.write("while ");
                    self.generate_expr(test_expr);
                    self.writeln(" {");
                } else {
                    self.write_indent();
                    self.writeln("loop {");
                }
                self.indent();
                for s in body {
                    self.generate_stmt(s);
                }
                if let Some(update_expr) = update {
                    self.write_indent();
                    self.generate_expr(update_expr);
                    self.writeln(";");
                }
                self.dedent();
                self.writeln("}");
                self.dedent();
                self.writeln("}");
            }
            RsStmt::ForIn(name, iter, body) => {
                self.write_indent();
                self.write(&format!("for {} in ", name));
                self.generate_expr(iter);
                self.writeln(" {");
                self.indent();
                for s in body {
                    self.generate_stmt(s);
                }
                self.dedent();
                self.writeln("}");
            }
            RsStmt::Loop(body) => {
                self.writeln("loop {");
                self.indent();
                for s in body {
                    self.generate_stmt(s);
                }
                self.dedent();
                self.writeln("}");
            }
            RsStmt::Try(try_body, catch_name, catch_body) => {
                self.writeln("match (|| -> Result<(), Box<dyn std::error::Error>> {");
                self.indent();
                self.write_indent();
                self.writeln("Ok({");
                self.indent();
                for stmt in try_body {
                    self.generate_stmt(stmt);
                }
                self.dedent();
                self.writeln("})");
                self.dedent();
                self.writeln("})() {");
                self.indent();
                self.writeln("Ok(val) => val,");
                self.write_indent();
                self.writeln(&format!("Err({}) => {{", catch_name));
                self.indent();
                for stmt in catch_body {
                    self.generate_stmt(stmt);
                }
                self.dedent();
                self.writeln("}");
                self.dedent();
                self.writeln("}");
            }
            RsStmt::Throw(expr) => {
                self.write_indent();
                self.write("return Err(Box::new(");
                self.generate_expr(expr);
                self.writeln("));");
            }
            RsStmt::Break => self.writeln("break;"),
            RsStmt::Continue => self.writeln("continue;"),
            RsStmt::Use(path) => self.writeln(&format!("use {};", path)),
            RsStmt::Mod(name) => self.writeln(&format!("mod {};", name)),
            RsStmt::Pub(inner) => {
                self.write_indent();
                self.write("pub ");
                self.generate_stmt(inner);
            }
            RsStmt::Async(body) => {
                self.writeln("async {");
                self.indent();
                for s in body {
                    self.generate_stmt(s);
                }
                self.dedent();
                self.writeln("}");
            }
            RsStmt::AwaitStmt(expr) => {
                self.write_indent();
                self.generate_expr(expr);
                self.write(".await;\n");
            }
            RsStmt::Empty => {}
            RsStmt::Match(expr, arms) => {
                self.write_indent();
                self.write("match ");
                self.generate_expr(expr);
                self.writeln(" {");
                self.indent();
                for arm in arms {
                    self.write_indent();
                    self.generate_pattern(&arm.pattern);
                    if let Some(guard) = &arm.guard {
                        self.write(" if ");
                        self.generate_expr(guard);
                    }
                    self.write(" => ");
                    self.generate_expr(&arm.body);
                    self.write(",\n");
                }
                self.dedent();
                self.write_indent();
                self.writeln("}");
            }
        }
    }

    pub fn generate_expr(&mut self, expr: &RsExpr) {
        self::expressions::generate_expression(self, expr);
    }

    pub fn generate_function(&mut self, func: &FunctionDef) {
        self::functions::generate_function_def(self, func);
    }

    pub fn generate_struct(&mut self, def: &StructDef) {
        self::rs_types::generate_struct(self, def);
    }

    pub fn generate_enum(&mut self, def: &EnumDef) {
        self::rs_types::generate_enum(self, def);
    }

    pub fn generate_impl(&mut self, impl_def: &ImplDef) {
        self::rs_types::generate_impl(self, impl_def);
    }

    pub fn generate_trait(&mut self, trait_def: &TraitDef) {
        self::rs_types::generate_trait(self, trait_def);
    }

    pub fn generate_pattern(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Lit(lit) => match lit {
                RsLit::Bool(b) => self.write(if *b { "true" } else { "false" }),
                RsLit::I64(n) => self.write(&n.to_string()),
                RsLit::F64(n) => self.write(&n.to_string()),
                RsLit::Str(s) => self.write(&format!("\"{}\"", s)),
                RsLit::Null => self.write("None"),
                _ => self.write("_"),
            },
            Pattern::Ident(name) => self.write(name),
            Pattern::Wildcard => self.write("_"),
            Pattern::Tuple(patterns) => {
                self.write("(");
                for (i, p) in patterns.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.generate_pattern(p);
                }
                self.write(")");
            }
            Pattern::Or(patterns) => {
                for (i, p) in patterns.iter().enumerate() {
                    if i > 0 {
                        self.write(" | ");
                    }
                    self.generate_pattern(p);
                }
            }
            Pattern::Range(start, end) => {
                self.generate_expr(start);
                self.write("..=");
                self.generate_expr(end);
            }
            _ => self.write("_"),
        }
    }
}

pub fn generate(
    module: &RustModule,
    options: &TranspileOptions,
) -> Result<crate::TranspileResult, anyhow::Error> {
    let mut codegen = CodeGen::new(options);
    let code = codegen.generate_module(module);
    Ok(crate::TranspileResult {
        code,
        imports: Vec::new(),
        warnings: Vec::new(),
    })
}
