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
