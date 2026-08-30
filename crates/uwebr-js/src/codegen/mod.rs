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
            RsStmt::ForLoop {
                init,
                test,
                update,
                body,
            } => {
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
                self.writeln("Ok(())");
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
                self.write("Err(Box::new(");
                self.generate_expr(expr);
                self.writeln(".to_string()))");
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TranspileOptions;

    fn codegen_module(module: &RustModule) -> String {
        let options = TranspileOptions::default();
        let mut codegen = CodeGen::new(&options);
        codegen.generate_module(module)
    }

    fn codegen_stmt(stmt: &RsStmt) -> String {
        let options = TranspileOptions::default();
        let mut codegen = CodeGen::new(&options);
        codegen.generate_stmt(stmt);
        codegen.output.clone()
    }

    #[test]
    fn js_codegen_let_statement() {
        let stmt = RsStmt::Let("x".into(), Type::I64, RsExpr::Lit(RsLit::I64(42)));
        let code = codegen_stmt(&stmt);
        assert!(code.contains("let x: i64 = 42;"), "got: {code}");
    }

    #[test]
    fn js_codegen_let_mut_statement() {
        let stmt = RsStmt::LetMut("x".into(), Type::I64, RsExpr::Lit(RsLit::I64(0)));
        let code = codegen_stmt(&stmt);
        assert!(code.contains("let mut x: i64 = 0;"), "got: {code}");
    }

    #[test]
    fn js_codegen_return_with_value() {
        let stmt = RsStmt::Return(Some(RsExpr::Lit(RsLit::I64(42))));
        let code = codegen_stmt(&stmt);
        assert!(code.contains("return 42;"), "got: {code}");
    }

    #[test]
    fn js_codegen_return_without_value() {
        let stmt = RsStmt::Return(None);
        let code = codegen_stmt(&stmt);
        assert!(code.contains("return;"), "got: {code}");
    }

    #[test]
    fn js_codegen_break() {
        let stmt = RsStmt::Break;
        let code = codegen_stmt(&stmt);
        assert!(code.contains("break;"), "got: {code}");
    }

    #[test]
    fn js_codegen_continue() {
        let stmt = RsStmt::Continue;
        let code = codegen_stmt(&stmt);
        assert!(code.contains("continue;"), "got: {code}");
    }

    #[test]
    fn js_codegen_if_no_else() {
        let stmt = RsStmt::If(RsExpr::Ident("cond".into()), vec![RsStmt::Break], None);
        let code = codegen_stmt(&stmt);
        assert!(code.contains("if cond"), "got: {code}");
        assert!(code.contains("break"), "got: {code}");
        assert!(!code.contains("else"), "should have no else: {code}");
    }

    #[test]
    fn js_codegen_if_else() {
        let stmt = RsStmt::If(
            RsExpr::Ident("cond".into()),
            vec![RsStmt::Break],
            Some(vec![RsStmt::Continue]),
        );
        let code = codegen_stmt(&stmt);
        assert!(code.contains("if cond"), "got: {code}");
        assert!(code.contains("else"), "got: {code}");
    }

    #[test]
    fn js_codegen_while_loop() {
        let stmt = RsStmt::While(RsExpr::Ident("running".into()), vec![RsStmt::Break]);
        let code = codegen_stmt(&stmt);
        assert!(code.contains("while running"), "got: {code}");
        assert!(code.contains("break"), "got: {code}");
    }

    #[test]
    fn js_codegen_for_loop() {
        let stmt = RsStmt::For("i".into(), RsExpr::Lit(RsLit::I64(10)), vec![RsStmt::Break]);
        let code = codegen_stmt(&stmt);
        assert!(code.contains("for i in 0..10"), "got: {code}");
    }

    #[test]
    fn js_codegen_for_in_loop() {
        let stmt = RsStmt::ForIn(
            "item".into(),
            RsExpr::Ident("items".into()),
            vec![RsStmt::Break],
        );
        let code = codegen_stmt(&stmt);
        assert!(code.contains("for item in items"), "got: {code}");
    }

    #[test]
    fn js_codegen_loop() {
        let stmt = RsStmt::Loop(vec![RsStmt::Break]);
        let code = codegen_stmt(&stmt);
        assert!(code.contains("loop"), "got: {code}");
        assert!(code.contains("break"), "got: {code}");
    }

    #[test]
    fn js_codegen_empty_statement() {
        let stmt = RsStmt::Empty;
        let code = codegen_stmt(&stmt);
        assert!(code.is_empty(), "empty should produce nothing: {code}");
    }

    #[test]
    fn js_codegen_use_statement() {
        let stmt = RsStmt::Use("std::collections::HashMap".into());
        let code = codegen_stmt(&stmt);
        assert!(
            code.contains("use std::collections::HashMap;"),
            "got: {code}"
        );
    }

    #[test]
    fn js_codegen_mod_statement() {
        let stmt = RsStmt::Mod("my_module".into());
        let code = codegen_stmt(&stmt);
        assert!(code.contains("mod my_module;"), "got: {code}");
    }

    #[test]
    fn js_codegen_async_block() {
        let stmt = RsStmt::Async(vec![RsStmt::Return(Some(RsExpr::Lit(RsLit::I64(42))))]);
        let code = codegen_stmt(&stmt);
        assert!(code.contains("async {"), "got: {code}");
        assert!(code.contains("return 42;"), "got: {code}");
    }

    #[test]
    fn js_codegen_throw_statement() {
        let stmt = RsStmt::Throw(RsExpr::Lit(RsLit::Str("error".into())));
        let code = codegen_stmt(&stmt);
        assert!(code.contains("Err"), "got: {code}");
    }

    #[test]
    fn js_codegen_try_catch() {
        let stmt = RsStmt::Try(vec![RsStmt::Break], "e".into(), vec![RsStmt::Continue]);
        let code = codegen_stmt(&stmt);
        assert!(code.contains("match"), "got: {code}");
        assert!(code.contains("Ok"), "got: {code}");
        assert!(code.contains("Err"), "got: {code}");
    }

    #[test]
    fn js_codegen_match_simple() {
        let stmt = RsStmt::Match(
            RsExpr::Ident("x".into()),
            vec![
                MatchArm {
                    pattern: Pattern::Lit(RsLit::I64(1)),
                    guard: None,
                    body: RsExpr::Lit(RsLit::Str("one".into())),
                },
                MatchArm {
                    pattern: Pattern::Wildcard,
                    guard: None,
                    body: RsExpr::Lit(RsLit::Str("other".into())),
                },
            ],
        );
        let code = codegen_stmt(&stmt);
        assert!(code.contains("match x"), "got: {code}");
        assert!(code.contains("1 =>"), "got: {code}");
        assert!(code.contains("_ =>"), "got: {code}");
    }

    #[test]
    fn js_codegen_pub_function() {
        let stmt = RsStmt::Pub(Box::new(RsStmt::Fn(FunctionDef {
            name: "hello".into(),
            params: vec![],
            return_type: Type::Void,
            body: vec![],
            is_async: false,
            generics: vec![],
        })));
        let code = codegen_stmt(&stmt);
        assert!(code.contains("pub fn hello()"), "got: {code}");
    }

    #[test]
    fn js_codegen_for_loop_with_init_and_update() {
        let stmt = RsStmt::ForLoop {
            init: Some(Box::new(RsStmt::LetMut(
                "i".into(),
                Type::I64,
                RsExpr::Lit(RsLit::I64(0)),
            ))),
            test: Some(RsExpr::Binary(
                BinOp::Lt,
                Box::new(RsExpr::Ident("i".into())),
                Box::new(RsExpr::Lit(RsLit::I64(10))),
            )),
            update: Some(RsExpr::Assign(
                AssignOp::AddAssign,
                Box::new(RsExpr::Ident("i".into())),
                Box::new(RsExpr::Lit(RsLit::I64(1))),
            )),
            body: vec![RsStmt::Break],
        };
        let code = codegen_stmt(&stmt);
        assert!(code.contains("let mut i"), "got: {code}");
        assert!(code.contains("while"), "got: {code}");
    }

    #[test]
    fn js_codegen_module_with_imports() {
        let module = RustModule {
            name: "test".into(),
            imports: vec![RsImport {
                path: "std::collections::HashMap".into(),
                items: vec!["HashMap".into()],
                is_glob: false,
            }],
            items: vec![RsStmt::Fn(FunctionDef {
                name: "f".into(),
                params: vec![],
                return_type: Type::Void,
                body: vec![],
                is_async: false,
                generics: vec![],
            })],
        };
        let code = codegen_module(&module);
        assert!(code.contains("fn f"), "got: {code}");
    }

    #[test]
    fn js_codegen_module_with_module_name() {
        let module = RustModule {
            name: "test".into(),
            imports: vec![],
            items: vec![RsStmt::Fn(FunctionDef {
                name: "f".into(),
                params: vec![],
                return_type: Type::Void,
                body: vec![],
                is_async: false,
                generics: vec![],
            })],
        };
        let options = TranspileOptions {
            module_name: Some("my_mod".into()),
            ..Default::default()
        };
        let result = generate(&module, &options).unwrap();
        assert!(result.code.contains("mod my_mod"), "got: {}", result.code);
    }

    #[test]
    fn js_codegen_pattern_or() {
        let stmt = RsStmt::Match(
            RsExpr::Ident("x".into()),
            vec![MatchArm {
                pattern: Pattern::Or(vec![
                    Pattern::Lit(RsLit::I64(1)),
                    Pattern::Lit(RsLit::I64(2)),
                ]),
                guard: None,
                body: RsExpr::Lit(RsLit::Str("one or two".into())),
            }],
        );
        let code = codegen_stmt(&stmt);
        assert!(code.contains("1 | 2 =>"), "got: {code}");
    }

    #[test]
    fn js_codegen_pattern_tuple() {
        let stmt = RsStmt::Match(
            RsExpr::Ident("x".into()),
            vec![MatchArm {
                pattern: Pattern::Tuple(vec![
                    Pattern::Lit(RsLit::I64(1)),
                    Pattern::Lit(RsLit::I64(2)),
                ]),
                guard: None,
                body: RsExpr::Lit(RsLit::Str("pair".into())),
            }],
        );
        let code = codegen_stmt(&stmt);
        assert!(code.contains("(1, 2) =>"), "got: {code}");
    }

    #[test]
    fn js_codegen_pattern_range() {
        let stmt = RsStmt::Match(
            RsExpr::Ident("x".into()),
            vec![MatchArm {
                pattern: Pattern::Range(
                    Box::new(RsExpr::Lit(RsLit::I64(1))),
                    Box::new(RsExpr::Lit(RsLit::I64(10))),
                ),
                guard: None,
                body: RsExpr::Lit(RsLit::Str("range".into())),
            }],
        );
        let code = codegen_stmt(&stmt);
        assert!(code.contains("1..=10 =>"), "got: {code}");
    }

    #[test]
    fn js_codegen_pattern_with_guard() {
        let stmt = RsStmt::Match(
            RsExpr::Ident("x".into()),
            vec![MatchArm {
                pattern: Pattern::Ident("n".into()),
                guard: Some(RsExpr::Binary(
                    BinOp::Gt,
                    Box::new(RsExpr::Ident("n".into())),
                    Box::new(RsExpr::Lit(RsLit::I64(0))),
                )),
                body: RsExpr::Lit(RsLit::Str("positive".into())),
            }],
        );
        let code = codegen_stmt(&stmt);
        assert!(code.contains("if"), "got: {code}");
        assert!(code.contains("n if"), "got: {code}");
    }

    #[test]
    fn js_codegen_expression_statement() {
        let stmt = RsStmt::Expr(RsExpr::Call(
            Box::new(RsExpr::Ident("do_something".into())),
            vec![],
        ));
        let code = codegen_stmt(&stmt);
        assert!(code.contains("doSomething();"), "got: {code}");
    }

    #[test]
    fn js_codegen_await_stmt() {
        let stmt = RsStmt::AwaitStmt(RsExpr::Ident("promise".into()));
        let code = codegen_stmt(&stmt);
        assert!(code.contains("promise.await;"), "got: {code}");
    }
}
