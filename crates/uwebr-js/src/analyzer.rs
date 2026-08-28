use crate::context::Context;
use crate::parser::ParsedModule;
use crate::types::*;
use anyhow::Result;
use swc_ecma_ast::{BinaryOp, BindingIdent, Callee, Expr as SwcExpr, Lit as SwcLit};

fn atom_str(atom: &swc_atoms::Atom) -> String {
    atom.as_str().to_string()
}

struct Analyzer {
    ctx: Context,
}

impl Analyzer {
    fn new() -> Self {
        Self {
            ctx: Context::new(),
        }
    }

    fn infer_type_from_expr(&mut self, expr: &SwcExpr) -> Type {
        match expr {
            SwcExpr::Lit(lit) => match lit {
                SwcLit::Num(n) => {
                    if n.value.fract() == 0.0 {
                        Type::I64
                    } else {
                        Type::F64
                    }
                }
                SwcLit::Str(_) => Type::String,
                SwcLit::Bool(_) => Type::Bool,
                SwcLit::Null(_) => Type::Option(Box::new(Type::Any)),
                SwcLit::Regex(_) => Type::String,
                _ => Type::Any,
            },
            SwcExpr::Ident(id) => self
                .ctx
                .lookup_var(&atom_str(&id.sym))
                .cloned()
                .unwrap_or(Type::Any),
            SwcExpr::Bin(bin) => match bin.op {
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                    let lt = self.infer_type_from_expr(&bin.left);
                    let rt = self.infer_type_from_expr(&bin.right);
                    if lt == rt {
                        lt
                    } else if matches!((&lt, &rt), (Type::String, Type::String)) {
                        Type::String
                    } else {
                        Type::Any
                    }
                }
                BinaryOp::EqEq
                | BinaryOp::NotEq
                | BinaryOp::EqEqEq
                | BinaryOp::NotEqEq
                | BinaryOp::Lt
                | BinaryOp::LtEq
                | BinaryOp::Gt
                | BinaryOp::GtEq
                | BinaryOp::LogicalAnd
                | BinaryOp::LogicalOr => Type::Bool,
                _ => Type::I64,
            },
            SwcExpr::Unary(unary) => {
                use swc_ecma_ast::UnaryOp;
                match unary.op {
                    UnaryOp::Bang => Type::Bool,
                    UnaryOp::Minus | UnaryOp::Plus | UnaryOp::Tilde => Type::I64,
                    UnaryOp::TypeOf => Type::String,
                    UnaryOp::Void => Type::Void,
                    _ => Type::Any,
                }
            }
            SwcExpr::Assign(assign) => self.infer_type_from_expr(&assign.right),
            SwcExpr::Call(call) => {
                if let Callee::Expr(expr) = &call.callee {
                    if let SwcExpr::Ident(id) = &**expr {
                        return match atom_str(&id.sym).as_str() {
                            "String" | "String.fromCharCode" => Type::String,
                            "Number" | "parseInt" | "parseFloat" => Type::F64,
                            "Boolean" => Type::Bool,
                            "Array" | "Array.from" | "Array.isArray" => {
                                Type::Vec(Box::new(Type::Any))
                            }
                            "Promise" => Type::Any,
                            "Map" => Type::HashMap(Box::new(Type::Any), Box::new(Type::Any)),
                            "Set" => Type::Vec(Box::new(Type::Any)),
                            "Object.keys" | "Object.values" | "Object.entries" => {
                                Type::Vec(Box::new(Type::Any))
                            }
                            _ => Type::Any,
                        };
                    }
                }
                Type::Any
            }
            SwcExpr::Fn(_) => Type::Any,
            SwcExpr::Arrow(_) => Type::Any,
            SwcExpr::Array(arr) => {
                if let Some(Some(elem)) = arr.elems.first() {
                    Type::Vec(Box::new(self.infer_type_from_expr(&elem.expr)))
                } else {
                    Type::Vec(Box::new(Type::Any))
                }
            }
            SwcExpr::Object(_) => Type::HashMap(Box::new(Type::String), Box::new(Type::Any)),
            SwcExpr::Tpl(_) => Type::String,
            SwcExpr::Cond(cond) => {
                let ct = self.infer_type_from_expr(&cond.cons);
                let at = self.infer_type_from_expr(&cond.alt);
                if ct == at {
                    ct
                } else {
                    Type::Any
                }
            }
            SwcExpr::Seq(seq) => seq
                .exprs
                .last()
                .map(|e| self.infer_type_from_expr(e))
                .unwrap_or(Type::Any),
            SwcExpr::Paren(paren) => self.infer_type_from_expr(&paren.expr),
            SwcExpr::Await(await_expr) => self.infer_type_from_expr(&await_expr.arg),
            _ => Type::Any,
        }
    }

    fn process_function(&mut self, name: &str, function: &swc_ecma_ast::Function, is_async: bool) {
        let mut param_defs: Vec<ParamDef> = Vec::new();
        for (i, p) in function.params.iter().enumerate() {
            let ty = p
                .pat
                .as_ident()
                .and_then(|id| {
                    id.type_ann
                        .as_ref()
                        .map(|t| self.ts_type_to_type(&t.type_ann))
                })
                .or_else(|| {
                    p.pat
                        .as_ident()
                        .and_then(|id| self.ctx.lookup_var(&atom_str(&id.id.sym)).cloned())
                })
                .unwrap_or(Type::Any);
            param_defs.push(ParamDef {
                name: p
                    .pat
                    .as_ident()
                    .map(|id| atom_str(&id.id.sym))
                    .unwrap_or_else(|| format!("arg{}", i)),
                ty,
                default: None,
            });
        }
        let ret_type = if let Some(rt) = &function.return_type {
            self.ts_type_to_type(&rt.type_ann)
        } else if let Some(body) = &function.body {
            self.infer_return_type(&body.stmts)
        } else {
            Type::Void
        };

        self.ctx.define_function(
            name,
            FunctionSig {
                params: param_defs,
                return_type: ret_type,
                is_async,
            },
        );
    }

    fn infer_return_type(&mut self, body: &[swc_ecma_ast::Stmt]) -> Type {
        for stmt in body {
            if let swc_ecma_ast::Stmt::Return(ret) = stmt {
                if let Some(ref expr) = ret.arg {
                    return self.infer_type_from_expr(expr);
                }
            }
        }
        Type::Void
    }

    fn ts_type_to_type(&self, type_ann: &swc_ecma_ast::TsType) -> Type {
        use swc_ecma_ast::*;
        match type_ann {
            TsType::TsKeywordType(kw) => match kw.kind {
                TsKeywordTypeKind::TsBooleanKeyword => Type::Bool,
                TsKeywordTypeKind::TsNumberKeyword => Type::F64,
                TsKeywordTypeKind::TsStringKeyword => Type::String,
                TsKeywordTypeKind::TsVoidKeyword => Type::Void,
                TsKeywordTypeKind::TsNeverKeyword => Type::Never,
                TsKeywordTypeKind::TsAnyKeyword => Type::Any,
                TsKeywordTypeKind::TsNullKeyword | TsKeywordTypeKind::TsUndefinedKeyword => {
                    Type::Option(Box::new(Type::Any))
                }
                _ => Type::Any,
            },
            TsType::TsTypeRef(TsTypeRef { type_name, .. }) => {
                if let TsEntityName::Ident(id) = type_name {
                    match atom_str(&id.sym).as_str() {
                        "Array" | "Vec" => Type::Vec(Box::new(Type::Any)),
                        "Promise" => Type::Any,
                        "Map" => Type::HashMap(Box::new(Type::Any), Box::new(Type::Any)),
                        "Set" => Type::Vec(Box::new(Type::Any)),
                        _ => Type::Struct(StructDef {
                            name: atom_str(&id.sym),
                            fields: Vec::new(),
                            impls: Vec::new(),
                        }),
                    }
                } else {
                    Type::Any
                }
            }
            TsType::TsArrayType(TsArrayType { elem_type, .. }) => {
                Type::Vec(Box::new(self.ts_type_to_type(elem_type)))
            }
            TsType::TsOptionalType(TsOptionalType { type_ann, .. }) => {
                Type::Option(Box::new(self.ts_type_to_type(type_ann)))
            }
            TsType::TsTupleType(TsTupleType { elem_types, .. }) => Type::Tuple(
                elem_types
                    .iter()
                    .map(|t| self.ts_type_to_type(&t.ty))
                    .collect(),
            ),
            TsType::TsParenthesizedType(TsParenthesizedType { type_ann, .. }) => {
                self.ts_type_to_type(type_ann)
            }
            _ => Type::Any,
        }
    }
}

pub fn analyze(module: &ParsedModule) -> Result<(Context, Vec<RsStmt>)> {
    let mut analyzer = Analyzer::new();
    let mut stmts = Vec::new();

    for item in &module.body {
        match item {
            swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(swc_ecma_ast::Decl::Fn(
                fn_decl,
            ))) => {
                let name = atom_str(&fn_decl.ident.sym);
                analyzer.process_function(&name, &fn_decl.function, fn_decl.function.is_async);
                stmts.push(RsStmt::Fn(FunctionDef {
                    name,
                    params: Vec::new(),
                    return_type: Type::Void,
                    body: Vec::new(),
                    is_async: fn_decl.function.is_async,
                    generics: Vec::new(),
                }));
            }
            swc_ecma_ast::ModuleItem::Stmt(swc_ecma_ast::Stmt::Decl(swc_ecma_ast::Decl::Var(
                var_decl,
            ))) => {
                for decl in &var_decl.decls {
                    if let swc_ecma_ast::Pat::Ident(BindingIdent { id, .. }) = &decl.name {
                        let ty = decl
                            .init
                            .as_ref()
                            .map(|i| analyzer.infer_type_from_expr(i))
                            .unwrap_or(Type::Any);
                        analyzer.ctx.define_var(&atom_str(&id.sym), ty);
                    }
                }
            }
            swc_ecma_ast::ModuleItem::ModuleDecl(swc_ecma_ast::ModuleDecl::Import(import)) => {
                let _path = import.src.value.to_atom_lossy().into_owned().as_str().to_string();
                for specifier in &import.specifiers {
                    let name = match specifier {
                        swc_ecma_ast::ImportSpecifier::Named(n) => atom_str(&n.local.sym),
                        swc_ecma_ast::ImportSpecifier::Default(d) => atom_str(&d.local.sym),
                        swc_ecma_ast::ImportSpecifier::Namespace(ns) => atom_str(&ns.local.sym),
                    };
                    analyzer.ctx.define_var(&name, Type::Any);
                }
            }
            _ => {}
        }
    }

    Ok((analyzer.ctx, stmts))
}
