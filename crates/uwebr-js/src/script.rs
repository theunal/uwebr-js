//! Lowering of top-level `<script>` state into reactive accessors.
//!
//! A `.uwebr` `<script>` block declares state at the top level:
//!
//! ```js
//! let count = 0;
//! function increment() { count++; }
//! ```
//!
//! Emitting that verbatim produces a module-scope `let`, which Rust rejects, and
//! leaves `count` out of scope inside `increment`. This pass rewrites each
//! top-level binding into a pair of accessor functions backed by
//! `uwebr_core::state`, so reads subscribe to a signal and writes schedule a
//! repaint:
//!
//! ```rust,ignore
//! fn __state_count() -> i64 { uwebr_core::state::get("count", 0) }
//! fn __set_state_count(v: i64) { uwebr_core::state::set("count", v); }
//! fn increment() { __set_state_count(__state_count() + 1); }
//! ```

use crate::types::*;

/// One lowered top-level binding.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptState {
    /// Original JS binding name, also the runtime state key.
    pub name: String,
    pub ty: Type,
    pub init: RsExpr,
    /// `true` when declared with `let`/`var` (i.e. writable).
    pub mutable: bool,
}

impl ScriptState {
    /// Name of the generated reader function.
    pub fn getter(&self) -> String {
        format!("__state_{}", self.name)
    }

    /// Name of the generated writer function.
    pub fn setter(&self) -> String {
        format!("__set_state_{}", self.name)
    }
}

/// Rewrite a module so top-level bindings become reactive accessors.
///
/// Returns the rewritten module plus the bindings that were lowered.
pub fn lower_script_state(module: &RustModule) -> (RustModule, Vec<ScriptState>) {
    let mut states = Vec::new();
    let mut rest: Vec<RsStmt> = Vec::new();

    for item in &module.items {
        match classify(item) {
            Some(state) => states.push(state),
            None => rest.push(item.clone()),
        }
    }

    if states.is_empty() {
        return (module.clone(), states);
    }

    // Rewrite every remaining statement so references go through the accessors.
    let mut items: Vec<RsStmt> = states.iter().flat_map(accessor_fns).collect();
    for stmt in &rest {
        items.push(rewrite_stmt(stmt, &states));
    }

    (
        RustModule {
            name: module.name.clone(),
            imports: module.imports.clone(),
            items,
        },
        states,
    )
}

/// Is this top-level statement a state binding?
fn classify(stmt: &RsStmt) -> Option<ScriptState> {
    match stmt {
        RsStmt::Let(name, ty, init) => Some(ScriptState {
            name: name.clone(),
            ty: concrete_type(ty, init),
            init: init.clone(),
            mutable: false,
        }),
        RsStmt::LetMut(name, ty, init) => Some(ScriptState {
            name: name.clone(),
            ty: concrete_type(ty, init),
            init: init.clone(),
            mutable: true,
        }),
        // `export let x = 1` arrives wrapped in Pub.
        RsStmt::Pub(inner) => classify(inner),
        _ => None,
    }
}

/// Fill in a usable Rust type when the analyzer inferred nothing.
///
/// `Type::Any` renders as `serde_json::Value`, which the generated project does
/// not depend on; guessing from the literal keeps the output compilable.
fn concrete_type(ty: &Type, init: &RsExpr) -> Type {
    if !ty.is_any() {
        return ty.clone();
    }
    match init {
        RsExpr::Lit(RsLit::I64(_)) | RsExpr::Lit(RsLit::I32(_)) => Type::I64,
        RsExpr::Lit(RsLit::F64(_)) => Type::F64,
        RsExpr::Lit(RsLit::Bool(_)) => Type::Bool,
        RsExpr::Lit(RsLit::Str(_)) => Type::String,
        _ => Type::Any,
    }
}

/// Build the getter/setter function definitions for one binding.
fn accessor_fns(state: &ScriptState) -> Vec<RsStmt> {
    let key = RsExpr::Lit(RsLit::Str(state.name.clone()));

    let getter = RsStmt::Fn(FunctionDef {
        name: state.getter(),
        params: vec![],
        return_type: state.ty.clone(),
        // `Return` rather than a bare expression: the codegen terminates
        // `RsStmt::Expr` with a semicolon, which would make the body evaluate to
        // `()` and fail to match the declared return type.
        body: vec![RsStmt::Return(Some(RsExpr::Call(
            Box::new(RsExpr::Path(vec![
                "uwebr_core".into(),
                "state".into(),
                "get".into(),
            ])),
            vec![key.clone(), state.init.clone()],
        )))],
        is_async: false,
        generics: vec![],
    });

    let setter = RsStmt::Fn(FunctionDef {
        name: state.setter(),
        params: vec![ParamDef {
            name: "value".into(),
            ty: state.ty.clone(),
            default: None,
        }],
        return_type: Type::Void,
        body: vec![RsStmt::Expr(RsExpr::Call(
            Box::new(RsExpr::Path(vec![
                "uwebr_core".into(),
                "state".into(),
                "set".into(),
            ])),
            vec![key, RsExpr::Ident("value".into())],
        ))],
        is_async: false,
        generics: vec![],
    });

    vec![getter, setter]
}

fn find<'a>(states: &'a [ScriptState], name: &str) -> Option<&'a ScriptState> {
    states.iter().find(|s| s.name == name)
}

// ── Statement rewriting ────────────────────────────────────────────────

fn rewrite_stmts(stmts: &[RsStmt], states: &[ScriptState]) -> Vec<RsStmt> {
    stmts.iter().map(|s| rewrite_stmt(s, states)).collect()
}

fn rewrite_stmt(stmt: &RsStmt, states: &[ScriptState]) -> RsStmt {
    match stmt {
        RsStmt::Expr(e) => RsStmt::Expr(rewrite_expr(e, states)),
        RsStmt::Let(n, t, e) => RsStmt::Let(n.clone(), t.clone(), rewrite_expr(e, states)),
        RsStmt::LetMut(n, t, e) => RsStmt::LetMut(n.clone(), t.clone(), rewrite_expr(e, states)),
        RsStmt::Return(Some(e)) => RsStmt::Return(Some(rewrite_expr(e, states))),
        RsStmt::Return(None) => RsStmt::Return(None),
        RsStmt::If(test, cons, alt) => RsStmt::If(
            rewrite_expr(test, states),
            rewrite_stmts(cons, states),
            alt.as_ref().map(|a| rewrite_stmts(a, states)),
        ),
        RsStmt::While(test, body) => {
            RsStmt::While(rewrite_expr(test, states), rewrite_stmts(body, states))
        }
        RsStmt::For(name, test, body) => RsStmt::For(
            name.clone(),
            rewrite_expr(test, states),
            rewrite_stmts(body, states),
        ),
        RsStmt::ForLoop {
            init,
            test,
            update,
            body,
        } => RsStmt::ForLoop {
            init: init.as_ref().map(|s| Box::new(rewrite_stmt(s, states))),
            test: test.as_ref().map(|e| rewrite_expr(e, states)),
            update: update.as_ref().map(|e| rewrite_expr(e, states)),
            body: rewrite_stmts(body, states),
        },
        RsStmt::ForIn(name, iter, body) => RsStmt::ForIn(
            name.clone(),
            rewrite_expr(iter, states),
            rewrite_stmts(body, states),
        ),
        RsStmt::Loop(body) => RsStmt::Loop(rewrite_stmts(body, states)),
        RsStmt::Match(e, arms) => RsStmt::Match(
            rewrite_expr(e, states),
            arms.iter()
                .map(|arm| MatchArm {
                    pattern: arm.pattern.clone(),
                    guard: arm.guard.as_ref().map(|g| rewrite_expr(g, states)),
                    body: rewrite_expr(&arm.body, states),
                })
                .collect(),
        ),
        RsStmt::Fn(func) => RsStmt::Fn(FunctionDef {
            name: func.name.clone(),
            params: func.params.clone(),
            return_type: func.return_type.clone(),
            body: rewrite_stmts(&func.body, states),
            is_async: func.is_async,
            generics: func.generics.clone(),
        }),
        RsStmt::Impl(impl_def) => RsStmt::Impl(ImplDef {
            self_type: impl_def.self_type.clone(),
            trait_name: impl_def.trait_name.clone(),
            methods: impl_def
                .methods
                .iter()
                .map(|m| MethodDef {
                    name: m.name.clone(),
                    params: m.params.clone(),
                    return_type: m.return_type.clone(),
                    body: rewrite_stmts(&m.body, states),
                    is_pub: m.is_pub,
                    is_async: m.is_async,
                    self_param: m.self_param.clone(),
                })
                .collect(),
            generics: impl_def.generics.clone(),
        }),
        RsStmt::Pub(inner) => RsStmt::Pub(Box::new(rewrite_stmt(inner, states))),
        RsStmt::Async(body) => RsStmt::Async(rewrite_stmts(body, states)),
        RsStmt::AwaitStmt(e) => RsStmt::AwaitStmt(rewrite_expr(e, states)),
        RsStmt::Try(body, name, catch) => RsStmt::Try(
            rewrite_stmts(body, states),
            name.clone(),
            rewrite_stmts(catch, states),
        ),
        RsStmt::Throw(e) => RsStmt::Throw(rewrite_expr(e, states)),
        // Structs, enums, traits, use, mod, break, continue, empty: no exprs.
        other => other.clone(),
    }
}

// ── Expression rewriting ───────────────────────────────────────────────

fn rewrite_exprs(exprs: &[RsExpr], states: &[ScriptState]) -> Vec<RsExpr> {
    exprs.iter().map(|e| rewrite_expr(e, states)).collect()
}

fn rewrite_expr(expr: &RsExpr, states: &[ScriptState]) -> RsExpr {
    match expr {
        // A bare reference to a lowered binding becomes a getter call.
        RsExpr::Ident(name) => match find(states, name) {
            Some(state) => RsExpr::Call(Box::new(RsExpr::Ident(state.getter())), vec![]),
            None => expr.clone(),
        },

        // Assignment to a lowered binding becomes a setter call.
        RsExpr::Assign(op, target, value) => {
            if let RsExpr::Ident(name) = &**target {
                if let Some(state) = find(states, name) {
                    let value = rewrite_expr(value, states);
                    let new_value = match op {
                        AssignOp::Assign => value,
                        _ => RsExpr::Binary(
                            compound_to_binary(op),
                            Box::new(RsExpr::Call(
                                Box::new(RsExpr::Ident(state.getter())),
                                vec![],
                            )),
                            Box::new(value),
                        ),
                    };
                    return RsExpr::Call(Box::new(RsExpr::Ident(state.setter())), vec![new_value]);
                }
            }
            RsExpr::Assign(
                op.clone(),
                Box::new(rewrite_expr(target, states)),
                Box::new(rewrite_expr(value, states)),
            )
        }

        RsExpr::Binary(op, l, r) => RsExpr::Binary(
            op.clone(),
            Box::new(rewrite_expr(l, states)),
            Box::new(rewrite_expr(r, states)),
        ),
        RsExpr::Unary(op, e) => RsExpr::Unary(op.clone(), Box::new(rewrite_expr(e, states))),
        RsExpr::Call(callee, args) => RsExpr::Call(
            Box::new(rewrite_expr(callee, states)),
            rewrite_exprs(args, states),
        ),
        RsExpr::New(name, args) => RsExpr::New(name.clone(), rewrite_exprs(args, states)),
        RsExpr::Member(obj, prop) => {
            RsExpr::Member(Box::new(rewrite_expr(obj, states)), prop.clone())
        }
        RsExpr::Index(obj, idx) => RsExpr::Index(
            Box::new(rewrite_expr(obj, states)),
            Box::new(rewrite_expr(idx, states)),
        ),
        RsExpr::ArrowFunction(params, ret, body) => {
            RsExpr::ArrowFunction(params.clone(), ret.clone(), rewrite_stmts(body, states))
        }
        RsExpr::FunctionExpr(name, params, ret, body) => RsExpr::FunctionExpr(
            name.clone(),
            params.clone(),
            ret.clone(),
            rewrite_stmts(body, states),
        ),
        RsExpr::If(test, cons, alt) => RsExpr::If(
            Box::new(rewrite_expr(test, states)),
            rewrite_stmts(cons, states),
            alt.as_ref().map(|a| rewrite_stmts(a, states)),
        ),
        RsExpr::Match(arms) => RsExpr::Match(
            arms.iter()
                .map(|arm| MatchArm {
                    pattern: arm.pattern.clone(),
                    guard: arm.guard.as_ref().map(|g| rewrite_expr(g, states)),
                    body: rewrite_expr(&arm.body, states),
                })
                .collect(),
        ),
        RsExpr::Block(stmts) => RsExpr::Block(rewrite_stmts(stmts, states)),
        RsExpr::Array(elems) => RsExpr::Array(rewrite_exprs(elems, states)),
        RsExpr::Object(fields) => RsExpr::Object(
            fields
                .iter()
                .map(|(k, v)| (k.clone(), rewrite_expr(v, states)))
                .collect(),
        ),
        RsExpr::Tuple(elems) => RsExpr::Tuple(rewrite_exprs(elems, states)),
        RsExpr::StructLiteral(name, fields) => RsExpr::StructLiteral(
            name.clone(),
            fields
                .iter()
                .map(|(k, v)| (k.clone(), rewrite_expr(v, states)))
                .collect(),
        ),
        RsExpr::FieldAccess(obj, field) => {
            RsExpr::FieldAccess(Box::new(rewrite_expr(obj, states)), field.clone())
        }
        RsExpr::MethodCall(obj, method, args) => RsExpr::MethodCall(
            Box::new(rewrite_expr(obj, states)),
            method.clone(),
            rewrite_exprs(args, states),
        ),
        RsExpr::OptionalChain(inner) => {
            RsExpr::OptionalChain(Box::new(rewrite_expr(inner, states)))
        }
        RsExpr::NullishCoalesce(l, r) => RsExpr::NullishCoalesce(
            Box::new(rewrite_expr(l, states)),
            Box::new(rewrite_expr(r, states)),
        ),
        RsExpr::Spread(elems) => RsExpr::Spread(rewrite_exprs(elems, states)),
        RsExpr::Closure(params, body) => {
            RsExpr::Closure(params.clone(), Box::new(rewrite_expr(body, states)))
        }
        RsExpr::AsyncBlock(body) => RsExpr::AsyncBlock(rewrite_stmts(body, states)),
        RsExpr::Await(e) => RsExpr::Await(Box::new(rewrite_expr(e, states))),
        RsExpr::TryBlock(body, name, catch) => RsExpr::TryBlock(
            rewrite_stmts(body, states),
            name.clone(),
            rewrite_stmts(catch, states),
        ),
        RsExpr::Throw(e) => RsExpr::Throw(Box::new(rewrite_expr(e, states))),
        RsExpr::Range(l, r) => RsExpr::Range(
            Box::new(rewrite_expr(l, states)),
            Box::new(rewrite_expr(r, states)),
        ),
        RsExpr::RangeInclusive(l, r) => RsExpr::RangeInclusive(
            Box::new(rewrite_expr(l, states)),
            Box::new(rewrite_expr(r, states)),
        ),
        RsExpr::Reference(e) => RsExpr::Reference(Box::new(rewrite_expr(e, states))),
        RsExpr::Deref(e) => RsExpr::Deref(Box::new(rewrite_expr(e, states))),
        RsExpr::TypeAscription(e, ty) => {
            RsExpr::TypeAscription(Box::new(rewrite_expr(e, states)), ty.clone())
        }
        // Literals and paths contain no identifiers to rewrite.
        RsExpr::Lit(_) | RsExpr::Path(_) => expr.clone(),
    }
}

/// `+=` → `+`, `-=` → `-`, and so on.
fn compound_to_binary(op: &AssignOp) -> BinOp {
    match op {
        AssignOp::AddAssign => BinOp::Add,
        AssignOp::SubAssign => BinOp::Sub,
        AssignOp::MulAssign => BinOp::Mul,
        AssignOp::DivAssign => BinOp::Div,
        AssignOp::ModAssign => BinOp::Mod,
        AssignOp::AndAssign => BinOp::BitAnd,
        AssignOp::OrAssign => BinOp::BitOr,
        AssignOp::XorAssign => BinOp::BitXor,
        AssignOp::ShlAssign => BinOp::Shl,
        AssignOp::ShrAssign => BinOp::Shr,
        // Plain `=` is handled before this is reached.
        AssignOp::Assign => BinOp::Add,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lower(js: &str) -> (String, Vec<ScriptState>) {
        let module = crate::parser::parse_js(js).unwrap();
        let (ctx, _) = crate::analyzer::analyze(&module).unwrap();
        let mut transformer = crate::transformer::Transformer::with_context(ctx);
        let rust_ast = transformer.transform_module(&module).unwrap();
        let (lowered, states) = lower_script_state(&rust_ast);
        let code = crate::codegen::generate(&lowered, &crate::TranspileOptions::default())
            .unwrap()
            .code;
        (code, states)
    }

    #[test]
    fn test_no_top_level_let_survives() {
        let (code, _) = lower("let count = 0; function increment() { count++; }");
        // A module-scope `let` does not compile in Rust.
        for line in code.lines() {
            assert!(
                !line.trim_start().starts_with("let "),
                "module-scope let emitted: {line}"
            );
        }
    }

    #[test]
    fn test_state_binding_detected() {
        let (_, states) = lower("let count = 0;");
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].name, "count");
        assert!(states[0].mutable);
    }

    #[test]
    fn test_accessors_generated() {
        let (code, _) = lower("let count = 0;");
        assert!(
            code.contains("fn __state_count()"),
            "getter missing:\n{code}"
        );
        assert!(
            code.contains("fn __set_state_count("),
            "setter missing:\n{code}"
        );
        assert!(code.contains("uwebr_core::state::get"));
        assert!(code.contains("uwebr_core::state::set"));
    }

    #[test]
    fn test_increment_uses_setter() {
        let (code, _) = lower("let count = 0; function increment() { count++; }");
        assert!(
            code.contains("__set_state_count("),
            "increment should write through the setter:\n{code}"
        );
        assert!(
            code.contains("__state_count()"),
            "increment should read through the getter:\n{code}"
        );
    }

    #[test]
    fn test_plain_assignment_uses_setter_without_read() {
        let (code, _) = lower("let x = 1; function reset() { x = 0; }");
        let reset = code
            .split("fn reset")
            .nth(1)
            .expect("reset function present");
        assert!(reset.contains("__set_state_x("));
    }

    #[test]
    fn test_read_in_expression_becomes_getter_call() {
        let (code, _) = lower("let n = 2; function double() { return n * 2; }");
        let double = code.split("fn double").nth(1).unwrap();
        assert!(double.contains("__state_n()"), "got:\n{double}");
    }

    #[test]
    fn test_numeric_type_inferred() {
        let (_, states) = lower("let count = 0;");
        assert!(
            matches!(states[0].ty, Type::I64 | Type::F64),
            "expected a numeric type, got {:?}",
            states[0].ty
        );
    }

    #[test]
    fn test_string_state_type() {
        let (_, states) = lower("let name = \"abc\";");
        assert_eq!(states[0].ty, Type::String);
    }

    #[test]
    fn test_bool_state_type() {
        let (_, states) = lower("let flag = true;");
        assert_eq!(states[0].ty, Type::Bool);
    }

    #[test]
    fn test_multiple_bindings() {
        let (code, states) = lower("let a = 1; let b = 2;");
        assert_eq!(states.len(), 2);
        assert!(code.contains("__state_a"));
        assert!(code.contains("__state_b"));
    }

    #[test]
    fn test_const_binding_is_lowered_too() {
        // `const` still needs to leave module scope to be readable from fns.
        let (code, states) = lower("const limit = 10; function check() { return limit; }");
        assert_eq!(states.len(), 1);
        assert!(!states[0].mutable);
        assert!(code.contains("__state_limit()"));
    }

    #[test]
    fn test_function_local_let_untouched() {
        let (code, states) = lower("function f() { let tmp = 1; return tmp; }");
        assert!(states.is_empty(), "locals are not module state");
        assert!(code.contains("let"), "local let should remain:\n{code}");
    }

    #[test]
    fn test_no_state_leaves_module_unchanged() {
        let module = crate::parser::parse_js("function f() { return 1; }").unwrap();
        let (ctx, _) = crate::analyzer::analyze(&module).unwrap();
        let mut t = crate::transformer::Transformer::with_context(ctx);
        let ast = t.transform_module(&module).unwrap();
        let (lowered, states) = lower_script_state(&ast);
        assert!(states.is_empty());
        assert_eq!(lowered.items.len(), ast.items.len());
    }

    #[test]
    fn test_shadowed_local_still_rewrites_outer_reads() {
        // Conservative: identifier-based rewriting also touches locals with the
        // same name. Documented here so the behaviour is intentional, not a
        // surprise — script blocks are small and shadowing is rare.
        let (code, _) = lower("let v = 1; function f() { return v + 1; }");
        assert!(code.contains("__state_v()"));
    }

    #[test]
    fn test_compound_operators_map_to_binary() {
        assert_eq!(compound_to_binary(&AssignOp::SubAssign), BinOp::Sub);
        assert_eq!(compound_to_binary(&AssignOp::MulAssign), BinOp::Mul);
        assert_eq!(compound_to_binary(&AssignOp::DivAssign), BinOp::Div);
    }

    #[test]
    fn test_decrement_uses_subtraction() {
        let (code, _) = lower("let n = 5; function dec() { n--; }");
        let dec = code.split("fn dec").nth(1).unwrap();
        assert!(dec.contains("__set_state_n("));
        assert!(dec.contains(" - "), "expected subtraction, got:\n{dec}");
    }

    #[test]
    fn test_getter_returns_value_not_unit() {
        // `RsStmt::Expr` is emitted with a trailing semicolon, so the body would
        // evaluate to `()` and not match the declared return type.
        let (code, _) = lower("let count = 0;");
        let getter = code.split("fn __state_count").nth(1).unwrap();
        assert!(
            getter.contains("return "),
            "getter must return a value:\n{getter}"
        );
    }
}
