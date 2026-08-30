use anyhow::Result;
use swc_common::{BytePos, DUMMY_SP};
use swc_ecma_ast::*;
use swc_ecma_parser::{lexer::Lexer, Parser, StringInput, Syntax, TsSyntax};

#[derive(Debug, Clone)]
pub struct ParsedModule {
    pub body: Vec<ModuleItem>,
    pub source: String,
}

pub fn parse_js(code: &str) -> Result<ParsedModule> {
    let lexer = Lexer::new(
        Syntax::Typescript(TsSyntax {
            tsx: true,
            decorators: true,
            ..Default::default()
        }),
        EsVersion::latest(),
        StringInput::new(code, BytePos(0), BytePos(code.len() as u32)),
        None,
    );

    let mut parser = Parser::new_from(lexer);
    let module = parser
        .parse_module()
        .map_err(|e| anyhow::anyhow!("Failed to parse JavaScript/TypeScript code: {:?}", e))?;

    Ok(ParsedModule {
        body: module.body,
        source: code.to_string(),
    })
}

pub fn stmt_to_expr(stmt: &Stmt) -> Option<&Expr> {
    match stmt {
        Stmt::Expr(expr_stmt) => Some(&expr_stmt.expr),
        _ => None,
    }
}

pub fn is_literal_expr(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Lit(Lit::Num(..))
            | Expr::Lit(Lit::Str(..))
            | Expr::Lit(Lit::Bool(..))
            | Expr::Lit(Lit::Null(..))
            | Expr::Lit(Lit::Regex(..))
            | Expr::Lit(Lit::BigInt(..))
    )
}

pub fn extract_string_lit(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Lit(Lit::Str(s)) => Some(s.value.to_string_lossy().to_string()),
        _ => None,
    }
}

pub fn extract_ident(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Ident(id) => Some(id.sym.as_str().to_string()),
        _ => None,
    }
}

pub fn is_async_function(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Decl(Decl::Fn(f)) => f.function.is_async,
        _ => false,
    }
}

pub fn is_await_expr(expr: &Expr) -> bool {
    matches!(expr, Expr::Await(..))
}

pub fn block_to_stmts(block: &BlockStmt) -> Vec<Stmt> {
    block.stmts.clone()
}

pub fn fn_body_to_stmts(body: &BlockStmt) -> Vec<Stmt> {
    body.stmts.clone()
}

pub fn arrow_body_to_stmts(body: &ArrowFunctionBody) -> Vec<Stmt> {
    match body {
        ArrowFunctionBody::FunctionBody(body) => body.stmts.clone(),
        ArrowFunctionBody::Expr(expr) => vec![Stmt::Expr(ExprStmt {
            span: DUMMY_SP,
            expr: expr.clone(),
        })],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(code: &str) -> ParsedModule {
        parse_js(code).unwrap()
    }

    #[test]
    fn js_parse_identifier() {
        let m = parse("let x = 1;");
        assert_eq!(m.body.len(), 1);
    }

    #[test]
    fn js_parse_number_literal() {
        let m = parse("let n = 42;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Lit(Lit::Num(n)) => assert_eq!(n.value, 42.0),
                    _ => panic!("expected number literal"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_float_literal() {
        let m = parse("let pi = 3.14;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Lit(Lit::Num(n)) => assert!((n.value - 3.14).abs() < 1e-10),
                    _ => panic!("expected number literal"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_string_literal_double_quotes() {
        let m = parse(r#"let s = "hello";"#);
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Lit(Lit::Str(s)) => {
                        let val = s.value.to_atom_lossy().into_owned();
                        assert_eq!(val.as_str(), "hello");
                    }
                    _ => panic!("expected string literal"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_string_literal_single_quotes() {
        let m = parse("let s = 'world';");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Lit(Lit::Str(s)) => {
                        let val = s.value.to_atom_lossy().into_owned();
                        assert_eq!(val.as_str(), "world");
                    }
                    _ => panic!("expected string literal"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_escape_sequences() {
        let m = parse(r#"let s = "line1\nline2";"#);
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Lit(Lit::Str(s)) => {
                        let val = s.value.to_atom_lossy().into_owned();
                        assert!(!val.is_empty(), "string should not be empty");
                    }
                    _ => panic!("expected string literal"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_boolean_true() {
        let m = parse("let b = true;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Lit(Lit::Bool(b)) => assert!(b.value),
                    _ => panic!("expected bool literal"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_boolean_false() {
        let m = parse("let b = false;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Lit(Lit::Bool(b)) => assert!(!b.value),
                    _ => panic!("expected bool literal"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_binary_ops() {
        let m = parse("let r = a + b;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Bin(bin) => assert!(matches!(bin.op, BinaryOp::Add)),
                    _ => panic!("expected binary expr"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_subtraction() {
        let m = parse("let r = a - b;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Bin(bin) => assert!(matches!(bin.op, BinaryOp::Sub)),
                    _ => panic!("expected binary expr"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_multiplication() {
        let m = parse("let r = a * b;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Bin(bin) => assert!(matches!(bin.op, BinaryOp::Mul)),
                    _ => panic!("expected binary expr"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_division() {
        let m = parse("let r = a / b;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Bin(bin) => assert!(matches!(bin.op, BinaryOp::Div)),
                    _ => panic!("expected binary expr"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_modulo() {
        let m = parse("let r = a % b;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Bin(bin) => assert!(matches!(bin.op, BinaryOp::Mod)),
                    _ => panic!("expected binary expr"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_equality_ops() {
        let m = parse("let r = a == b;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Bin(bin) => assert!(matches!(bin.op, BinaryOp::EqEq)),
                    _ => panic!("expected binary expr"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_strict_equality() {
        let m = parse("let r = a === b;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Bin(bin) => assert!(matches!(bin.op, BinaryOp::EqEqEq)),
                    _ => panic!("expected binary expr"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_not_equal() {
        let m = parse("let r = a != b;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Bin(bin) => assert!(matches!(bin.op, BinaryOp::NotEq)),
                    _ => panic!("expected binary expr"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_less_than() {
        let m = parse("let r = a < b;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Bin(bin) => assert!(matches!(bin.op, BinaryOp::Lt)),
                    _ => panic!("expected binary expr"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_greater_than() {
        let m = parse("let r = a > b;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Bin(bin) => assert!(matches!(bin.op, BinaryOp::Gt)),
                    _ => panic!("expected binary expr"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_logical_and() {
        let m = parse("let r = a && b;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Bin(bin) => assert!(matches!(bin.op, BinaryOp::LogicalAnd)),
                    _ => panic!("expected binary expr"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_logical_or() {
        let m = parse("let r = a || b;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Bin(bin) => assert!(matches!(bin.op, BinaryOp::LogicalOr)),
                    _ => panic!("expected binary expr"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_negative_number() {
        let m = parse("let x = -5;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Unary(unary) => {
                        assert!(matches!(unary.op, UnaryOp::Minus));
                    }
                    _ => panic!("expected unary expr"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_arrow_function_no_params() {
        let m = parse("const f = () => 1;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Arrow(arrow) => assert!(arrow.params.is_empty()),
                    _ => panic!("expected arrow function"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_arrow_function_single_param() {
        let m = parse("const f = (x) => x * 2;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Arrow(arrow) => assert_eq!(arrow.params.len(), 1),
                    _ => panic!("expected arrow function"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_arrow_function_multi_params() {
        let m = parse("const f = (a, b, c) => a + b + c;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Arrow(arrow) => assert_eq!(arrow.params.len(), 3),
                    _ => panic!("expected arrow function"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_arrow_function_with_body() {
        let m = parse("const f = (x) => { return x + 1; };");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Arrow(arrow) => match &*arrow.body {
                        ArrowFunctionBody::FunctionBody(body) => {
                            assert!(!body.stmts.is_empty())
                        }
                        _ => panic!("expected function body"),
                    },
                    _ => panic!("expected arrow function"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_array_destructuring() {
        let m = parse("let [a, b, c] = arr;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => match &vd.decls[0].name {
                Pat::Array(arr) => assert_eq!(arr.elems.len(), 3),
                _ => panic!("expected array pattern"),
            },
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_object_destructuring() {
        let m = parse("let {x, y} = point;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => match &vd.decls[0].name {
                Pat::Object(obj) => assert_eq!(obj.props.len(), 2),
                _ => panic!("expected object pattern"),
            },
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_spread_in_array() {
        let m = parse("let r = [...a, ...b];");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Array(arr) => {
                        for elem in arr.elems.iter().flatten() {
                            assert!(elem.spread.is_some());
                        }
                    }
                    _ => panic!("expected array expr"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_spread_in_function_call() {
        let m = parse("f(...args);");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Expr(es)) => match &*es.expr {
                Expr::Call(call) => {
                    for arg in &call.args {
                        assert!(arg.spread.is_some());
                    }
                }
                _ => panic!("expected call expr"),
            },
            _ => panic!("expected expr stmt"),
        }
    }

    #[test]
    fn js_parse_null_literal() {
        let m = parse("let x = null;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Lit(Lit::Null(_)) => {}
                    _ => panic!("expected null literal"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_function_declaration() {
        let m = parse("function add(a, b) { return a + b; }");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Fn(fd))) => {
                assert_eq!(fd.ident.sym.as_str(), "add");
                assert_eq!(fd.function.params.len(), 2);
            }
            _ => panic!("expected fn decl"),
        }
    }

    #[test]
    fn js_parse_class_declaration() {
        let m = parse("class Foo { constructor() {} }");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Class(cd))) => {
                assert_eq!(cd.ident.sym.as_str(), "Foo");
            }
            _ => panic!("expected class decl"),
        }
    }

    #[test]
    fn js_parse_ternary_expression() {
        let m = parse("let r = a > 0 ? a : -a;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Cond(cond) => {
                        assert!(!matches!(*cond.alt, Expr::Lit(Lit::Null(_))));
                    }
                    _ => panic!("expected conditional expr"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_operator_precedence_mul_before_add() {
        let m = parse("let r = 1 + 2 * 3;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Bin(outer) => {
                        assert!(matches!(outer.op, BinaryOp::Add));
                        match &*outer.right {
                            Expr::Bin(inner) => assert!(matches!(inner.op, BinaryOp::Mul)),
                            _ => panic!("expected inner binary"),
                        }
                    }
                    _ => panic!("expected binary expr"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_member_access() {
        let m = parse("let r = obj.prop;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Member(member) => {
                        if let MemberProp::Ident(id) = &member.prop {
                            assert_eq!(id.sym.as_str(), "prop");
                        } else {
                            panic!("expected ident prop");
                        }
                    }
                    _ => panic!("expected member expr"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_computed_member_access() {
        let m = parse("let r = arr[0];");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Member(member) => {
                        assert!(matches!(&member.prop, MemberProp::Computed(_)));
                    }
                    _ => panic!("expected member expr"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_update_expression() {
        let m = parse("x++;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Expr(es)) => match &*es.expr {
                Expr::Update(upd) => assert!(matches!(upd.op, UpdateOp::PlusPlus)),
                _ => panic!("expected update expr"),
            },
            _ => panic!("expected expr stmt"),
        }
    }

    #[test]
    fn js_parse_for_statement() {
        let m = parse("for (let i = 0; i < 10; i++) {}");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::For(_)) => {}
            _ => panic!("expected for statement"),
        }
    }

    #[test]
    fn js_parse_while_statement() {
        let m = parse("while (true) {}");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::While(_)) => {}
            _ => panic!("expected while statement"),
        }
    }

    #[test]
    fn js_parse_if_else_statement() {
        let m = parse("if (true) {} else {}");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::If(ifst)) => {
                assert!(ifst.alt.is_some());
            }
            _ => panic!("expected if statement"),
        }
    }

    #[test]
    fn js_parse_try_catch() {
        let m = parse("try { x(); } catch (e) { }");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Try(ts)) => {
                assert!(ts.handler.is_some());
            }
            _ => panic!("expected try statement"),
        }
    }

    #[test]
    fn js_parse_switch_case() {
        let m = parse("switch (x) { case 1: break; default: break; }");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Switch(sw)) => {
                assert_eq!(sw.cases.len(), 2);
            }
            _ => panic!("expected switch statement"),
        }
    }

    #[test]
    fn js_parse_object_literal() {
        let m = parse("let obj = { a: 1, b: 2 };");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Object(obj) => assert_eq!(obj.props.len(), 2),
                    _ => panic!("expected object literal"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_array_literal() {
        let m = parse("let arr = [1, 2, 3];");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Array(arr) => assert_eq!(arr.elems.len(), 3),
                    _ => panic!("expected array literal"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_template_literal_no_expressions() {
        let m = parse("let s = `hello world`;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Tpl(tpl) => assert!(tpl.exprs.is_empty()),
                    _ => panic!("expected template literal"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_template_literal_with_expressions() {
        let m = parse("let s = `hello ${name}`;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Tpl(tpl) => assert_eq!(tpl.exprs.len(), 1),
                    _ => panic!("expected template literal"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_optional_chaining() {
        let m = parse("let r = obj?.prop;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::OptChain(_) => {}
                    _ => panic!("expected optional chain"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_new_expression() {
        let m = parse("new Foo(1, 2);");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Expr(es)) => match &*es.expr {
                Expr::New(new) => match &*new.callee {
                    Expr::Ident(id) => assert_eq!(id.sym.as_str(), "Foo"),
                    _ => panic!("expected ident callee"),
                },
                _ => panic!("expected new expression"),
            },
            _ => panic!("expected expr stmt"),
        }
    }

    #[test]
    fn js_parse_utility_is_literal() {
        let num_expr = Expr::Lit(Lit::Num(1.0.into()));
        assert!(is_literal_expr(&num_expr));
        let ident = Expr::Ident(Ident::new("x".into(), DUMMY_SP, Default::default()));
        assert!(!is_literal_expr(&ident));
    }

    #[test]
    fn js_parse_utility_extract_string() {
        let str_lit = Lit::Str(Str {
            span: DUMMY_SP,
            value: "test".into(),
            raw: None,
        });
        let expr = Expr::Lit(str_lit);
        assert_eq!(extract_string_lit(&expr).unwrap(), "test");
    }

    #[test]
    fn js_parse_utility_extract_ident() {
        let ident = Expr::Ident(Ident::new("myVar".into(), DUMMY_SP, Default::default()));
        assert_eq!(extract_ident(&ident).unwrap(), "myVar");
    }

    #[test]
    fn js_parse_utility_extract_ident_non_ident() {
        let lit = Expr::Lit(Lit::Bool(Bool {
            span: DUMMY_SP,
            value: true,
        }));
        assert!(extract_ident(&lit).is_none());
    }

    #[test]
    fn js_parse_utility_is_async_function() {
        let m = parse("async function fetchData() {}");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Fn(fd))) => {
                assert!(is_async_function(&Stmt::Decl(Decl::Fn(fd.clone()))));
            }
            _ => panic!("expected fn decl"),
        }
    }

    #[test]
    fn js_parse_utility_is_await() {
        let m = parse("async function f() { await p; }");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Fn(fd))) => {
                let body = fd.function.body.as_ref().unwrap();
                match &body.stmts[0] {
                    Stmt::Expr(es) => {
                        assert!(is_await_expr(&es.expr));
                    }
                    _ => panic!("expected expr stmt"),
                }
            }
            _ => panic!("expected fn decl"),
        }
    }

    #[test]
    fn js_parse_utility_block_to_stmts() {
        let m = parse("{ let x = 1; let y = 2; }");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Block(block)) => {
                let stmts = block_to_stmts(block);
                assert_eq!(stmts.len(), 2);
            }
            _ => panic!("expected block"),
        }
    }

    #[test]
    fn js_parse_utility_fn_body_to_stmts() {
        let m = parse("function f() { let x = 1; return x; }");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Fn(fd))) => {
                let body = fd.function.body.as_ref().unwrap();
                let stmts = &body.stmts;
                assert_eq!(stmts.len(), 2);
            }
            _ => panic!("expected fn decl"),
        }
    }

    #[test]
    fn js_parse_utility_arrow_body_to_stmts_block() {
        let m = parse("const f = (x) => { return x + 1; };");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Arrow(arrow) => {
                        let stmts = arrow_body_to_stmts(&arrow.body);
                        assert_eq!(stmts.len(), 1);
                    }
                    _ => panic!("expected arrow"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_utility_arrow_body_to_stmts_expr() {
        let m = parse("const f = (x) => x + 1;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Arrow(arrow) => {
                        let stmts = arrow_body_to_stmts(&arrow.body);
                        assert_eq!(stmts.len(), 1);
                    }
                    _ => panic!("expected arrow"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_negative_float() {
        let m = parse("let x = -3.14;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Unary(unary) => {
                        assert!(matches!(unary.op, UnaryOp::Minus));
                    }
                    _ => panic!("expected unary minus"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_hex_literal() {
        let m = parse("let x = 0xff;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Lit(Lit::Num(n)) => assert_eq!(n.value, 255.0),
                    _ => panic!("expected number literal"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_binary_literal() {
        let m = parse("let x = 0b1010;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => {
                let init = vd.decls[0].init.as_ref().unwrap();
                match &**init {
                    Expr::Lit(Lit::Num(n)) => assert_eq!(n.value, 10.0),
                    _ => panic!("expected number literal"),
                }
            }
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_parse_js_empty() {
        let m = parse_js("");
        assert!(m.unwrap().body.is_empty());
    }

    #[test]
    fn js_parse_parse_js_invalid_syntax() {
        let result = parse_js("let = = = ;");
        assert!(result.is_err());
    }

    #[test]
    fn js_parse_multiple_statements() {
        let m = parse("let a = 1; let b = 2; let c = 3;");
        assert_eq!(m.body.len(), 3);
    }

    #[test]
    fn js_parse_nested_destructuring() {
        let m = parse("let {a: {b, c}} = obj;");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(vd))) => match &vd.decls[0].name {
                Pat::Object(obj) => assert_eq!(obj.props.len(), 1),
                _ => panic!("expected object pattern"),
            },
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn js_parse_rest_params() {
        let m = parse("function f(...args) {}");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Fn(fd))) => {
                assert_eq!(fd.function.params.len(), 1);
            }
            _ => panic!("expected fn decl"),
        }
    }

    #[test]
    fn js_parse_default_params() {
        let m = parse("function f(x = 10) {}");
        match &m.body[0] {
            ModuleItem::Stmt(Stmt::Decl(Decl::Fn(fd))) => {
                assert_eq!(fd.function.params.len(), 1);
            }
            _ => panic!("expected fn decl"),
        }
    }

    #[test]
    fn js_parse_import_declaration() {
        let m = parse("import { foo } from 'bar';");
        match &m.body[0] {
            ModuleItem::ModuleDecl(ModuleDecl::Import(_)) => {}
            _ => panic!("expected import"),
        }
    }

    #[test]
    fn js_parse_export_declaration() {
        let m = parse("export function hello() {}");
        match &m.body[0] {
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(_)) => {}
            _ => panic!("expected export decl"),
        }
    }
}
