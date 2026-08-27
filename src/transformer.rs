use crate::context::Context;
use crate::parser::ParsedModule;
use crate::types::*;
use anyhow::Result;
use swc_ecma_ast::{
    AssignTarget, BindingIdent, Callee as SwcCallee, ClassMember, Decl, Expr as SwcExpr,
    ImportSpecifier, Lit as SwcLit, MemberProp, ModuleDecl, ModuleItem, Prop, PropName,
    PropOrSpread, UnaryOp as SwcUnaryOp, VarDeclarator,
};

fn wtf8_to_string(w: &swc_atoms::Wtf8Atom) -> String {
    w.to_atom_lossy().into_owned().as_str().to_string()
}

fn atom_to_string(atom: &swc_atoms::Atom) -> String {
    atom.as_str().to_string()
}

fn block_stmts(stmt: &swc_ecma_ast::Stmt) -> Vec<swc_ecma_ast::Stmt> {
    match stmt {
        swc_ecma_ast::Stmt::Block(block) => block.stmts.clone(),
        _ => vec![stmt.clone()],
    }
}

pub struct Transformer {
    ctx: Context,
}

impl Transformer {
    pub fn new() -> Self {
        Self {
            ctx: Context::new(),
        }
    }

    pub fn with_context(ctx: Context) -> Self {
        Self { ctx }
    }

    fn pat_to_names(pat: &swc_ecma_ast::Pat) -> Vec<String> {
        match pat {
            swc_ecma_ast::Pat::Ident(id) => vec![atom_to_string(&id.id.sym)],
            swc_ecma_ast::Pat::Array(arr) => {
                arr.elems.iter().filter_map(|e| {
                    e.as_ref().map(|p| Self::pat_to_names(p)).map(|mut v| {
                        if v.len() == 1 {
                            v.remove(0)
                        } else {
                            v.join(", ")
                        }
                    })
                }).collect()
            }
            swc_ecma_ast::Pat::Object(obj) => {
                obj.props.iter().filter_map(|prop| {
                    match prop {
                        swc_ecma_ast::ObjectPatProp::Assign(assign) => {
                            Some(atom_to_string(&assign.key.id.sym))
                        }
                        swc_ecma_ast::ObjectPatProp::KeyValue(kv) => {
                            match &*kv.value {
                                swc_ecma_ast::Pat::Ident(id) => Some(atom_to_string(&id.id.sym)),
                                _ => None,
                            }
                        }
                        _ => None,
                    }
                }).collect()
            }
            _ => vec![],
        }
    }

    fn transform_expr(&self, expr: &SwcExpr) -> RsExpr {
        match expr {
            SwcExpr::Lit(lit) => RsExpr::Lit(self.transform_lit(lit)),
            SwcExpr::Ident(id) => RsExpr::Ident(atom_to_string(&id.sym)),
            SwcExpr::Bin(bin) => {
                if let swc_ecma_ast::BinaryOp::NullishCoalescing = bin.op {
                    RsExpr::NullishCoalesce(
                        Box::new(self.transform_expr(&bin.left)),
                        Box::new(self.transform_expr(&bin.right)),
                    )
                } else {
                    RsExpr::Binary(
                        self.transform_bin_op(&bin.op),
                        Box::new(self.transform_expr(&bin.left)),
                        Box::new(self.transform_expr(&bin.right)),
                    )
                }
            }
            SwcExpr::Unary(unary) => RsExpr::Unary(
                self.transform_unary_op(&unary.op),
                Box::new(self.transform_expr(&unary.arg)),
            ),
            SwcExpr::Assign(assign) => {
                let left = match &assign.left {
                    AssignTarget::Simple(simple) => self.transform_simple_assign_target(simple),
                    _ => RsExpr::Lit(RsLit::Null),
                };
                RsExpr::Assign(
                    self.transform_assign_op(&assign.op),
                    Box::new(left),
                    Box::new(self.transform_expr(&assign.right)),
                )
            }
            SwcExpr::Call(call) => {
                let args: Vec<RsExpr> = call
                    .args
                    .iter()
                    .map(|a| {
                        if a.spread.is_some() {
                            RsExpr::Spread(vec![self.transform_expr(&a.expr)])
                        } else {
                            self.transform_expr(&a.expr)
                        }
                    })
                    .collect();
                if let SwcCallee::Expr(expr) = &call.callee {
                    if let SwcExpr::Member(member) = &**expr {
                        if let SwcExpr::Ident(obj) = &*member.obj {
                            if atom_to_string(&obj.sym) == "console" {
                                if let MemberProp::Ident(prop) = &member.prop {
                                    let method = atom_to_string(&prop.sym);
                                    return RsExpr::Call(
                                        Box::new(RsExpr::Ident(format!("console_{}", method))),
                                        args,
                                    );
                                }
                            }
                        }
                    }
                }
                if let SwcCallee::Expr(expr) = &call.callee {
                    if let SwcExpr::Ident(id) = &**expr {
                        let name = atom_to_string(&id.sym);
                        if name == "Array" && args.len() == 1 {
                            let size = args.into_iter().next().unwrap();
                            return RsExpr::Call(
                                Box::new(RsExpr::Ident("vec".to_string())),
                                vec![RsExpr::Lit(RsLit::Null), size],
                            );
                        }
                    }
                    if let SwcExpr::Member(member) = &**expr {
                        if let SwcExpr::Ident(obj) = &*member.obj {
                            let obj_name = atom_to_string(&obj.sym);
                            if obj_name == "Array" {
                                if let MemberProp::Ident(prop) = &member.prop {
                                    let method = atom_to_string(&prop.sym);
                                    if method == "from" {
                                        if let Some(first_arg) = args.first() {
                                            return first_arg.clone();
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                let callee = self.transform_callee(&call.callee);
                if let RsExpr::Member(ref obj, ref method) = callee {
                    let iter_methods = [
                        "filter", "map", "reduce", "forEach", "find", "some", "every",
                        "flatMap", "flat", "findIndex", "keys", "values", "entries",
                        "lastIndexOf",
                    ];
                    if iter_methods.contains(&method.as_str()) {
                        let iter_obj = RsExpr::MethodCall(
                            obj.clone(),
                            "iter".to_string(),
                            vec![],
                        );
                        return RsExpr::MethodCall(
                            Box::new(iter_obj),
                            method.clone(),
                            args,
                        );
                    }
                    return RsExpr::MethodCall(
                        obj.clone(),
                        method.clone(),
                        args,
                    );
                }
                RsExpr::Call(Box::new(callee), args)
            }
            SwcExpr::New(new_expr) => {
                let callee = self.transform_expr(&new_expr.callee);
                let args: Vec<RsExpr> = new_expr
                    .args
                    .as_ref()
                    .map(|a| a.iter().map(|a| self.transform_expr(&a.expr)).collect())
                    .unwrap_or_default();
                if let RsExpr::Ident(name) = callee {
                    RsExpr::New(name, args)
                } else {
                    RsExpr::Call(Box::new(callee), args)
                }
            }
            SwcExpr::Member(member) => {
                let obj = self.transform_expr(&member.obj);
                match &member.prop {
                    MemberProp::Ident(id) => RsExpr::Member(Box::new(obj), atom_to_string(&id.sym)),
                    MemberProp::Computed(computed) => {
                        RsExpr::Index(Box::new(obj), Box::new(self.transform_expr(&computed.expr)))
                    }
                    _ => RsExpr::Member(Box::new(obj), "unknown".to_string()),
                }
            }
            SwcExpr::Arrow(arrow) => {
                let params: Vec<ParamDef> = arrow
                    .params
                    .iter()
                    .enumerate()
                    .map(|(i, p)| ParamDef {
                        name: p
                            .as_ident()
                            .map(|id| atom_to_string(&id.id.sym))
                            .unwrap_or_else(|| format!("arg{}", i)),
                        ty: Type::Any,
                        default: None,
                    })
                    .collect();
                let body = match &*arrow.body {
                    swc_ecma_ast::ArrowFunctionBody::FunctionBody(body) => {
                        self.transform_stmts(&body.stmts)
                    }
                    swc_ecma_ast::ArrowFunctionBody::Expr(expr) => {
                        vec![RsStmt::Expr(self.transform_expr(expr))]
                    }
                };
                RsExpr::ArrowFunction(params, Type::Void, body)
            }
            SwcExpr::Fn(fn_expr) => {
                let params: Vec<ParamDef> = fn_expr
                    .function
                    .params
                    .iter()
                    .enumerate()
                    .map(|(i, p)| ParamDef {
                        name: p
                            .pat
                            .as_ident()
                            .map(|id| atom_to_string(&id.id.sym))
                            .unwrap_or_else(|| format!("arg{}", i)),
                        ty: Type::Any,
                        default: None,
                    })
                    .collect();
                let body = fn_expr
                    .function
                    .body
                    .as_ref()
                    .map(|b| self.transform_stmts(&b.stmts))
                    .unwrap_or_default();
                RsExpr::ArrowFunction(params, Type::Void, body)
            }
            SwcExpr::Array(arr) => {
                let elems: Vec<RsExpr> = arr
                    .elems
                    .iter()
                    .filter_map(|e| e.as_ref().map(|e| {
                        if e.spread.is_some() {
                            RsExpr::Spread(vec![self.transform_expr(&e.expr)])
                        } else {
                            self.transform_expr(&e.expr)
                        }
                    }))
                    .collect();
                RsExpr::Array(elems)
            }
            SwcExpr::Object(obj) => {
                let mut props: Vec<(String, RsExpr)> = Vec::new();
                let mut spreads: Vec<RsExpr> = Vec::new();
                for p in &obj.props {
                    match p {
                        PropOrSpread::Prop(prop) => {
                            match &**prop {
                                Prop::KeyValue(kv) => {
                                    let key = match &kv.key {
                                        PropName::Ident(id) => atom_to_string(&id.sym),
                                        PropName::Str(s) => wtf8_to_string(&s.value),
                                        PropName::Num(n) => format!("{}", n.value),
                                        _ => "unknown".to_string(),
                                    };
                                    props.push((key, self.transform_expr(&kv.value)));
                                }
                                Prop::Shorthand(ident) => {
                                    let name = atom_to_string(&ident.sym);
                                    props.push((name.clone(), RsExpr::Ident(name)));
                                }
                                Prop::Method(method) => {
                                    if let Some(key_name) = Self::transform_prop_name_to_name(&method.key) {
                                        let params: Vec<ParamDef> = method.function.params.iter().enumerate().map(|(i, p)| {
                                            ParamDef {
                                                name: p.pat.as_ident()
                                                    .map(|id| atom_to_string(&id.id.sym))
                                                    .unwrap_or_else(|| format!("arg{}", i)),
                                                ty: Type::Any,
                                                default: None,
                                            }
                                        }).collect();
                                        let body = method.function.body.as_ref()
                                            .map(|b| self.transform_stmts(&b.stmts))
                                            .unwrap_or_default();
                                        props.push((key_name, RsExpr::ArrowFunction(params, Type::Void, body)));
                                    }
                                }
                                Prop::Getter(getter) => {
                                    if let Some(key_name) = Self::transform_prop_name_to_name(&getter.key) {
                                        let body = getter.function.body.as_ref()
                                            .map(|b| self.transform_stmts(&b.stmts))
                                            .unwrap_or_default();
                                        props.push((key_name, RsExpr::ArrowFunction(vec![], Type::Void, body)));
                                    }
                                }
                                Prop::Setter(setter) => {
                                    if let Some(key_name) = Self::transform_prop_name_to_name(&setter.key) {
                                        let param = setter.function.params.first().and_then(|p| {
                                            p.pat.as_ident().map(|id| {
                                                ParamDef {
                                                    name: atom_to_string(&id.id.sym),
                                                    ty: Type::Any,
                                                    default: None,
                                                }
                                            })
                                        }).unwrap_or_else(|| ParamDef {
                                            name: "value".to_string(),
                                            ty: Type::Any,
                                            default: None,
                                        });
                                        let body = setter.function.body.as_ref()
                                            .map(|b| self.transform_stmts(&b.stmts))
                                            .unwrap_or_default();
                                        props.push((key_name, RsExpr::ArrowFunction(vec![param], Type::Void, body)));
                                    }
                                }
                                _ => {}
                            }
                        }
                        PropOrSpread::Spread(spread) => {
                            spreads.push(self.transform_expr(&spread.expr));
                        }
                    }
                }
                if spreads.is_empty() {
                    RsExpr::Object(props)
                } else {
                    let mut all_entries: Vec<RsExpr> = Vec::new();
                    for spread in spreads {
                        all_entries.push(RsExpr::MethodCall(
                            Box::new(spread),
                            "iter".to_string(),
                            vec![],
                        ));
                    }
                    for (k, v) in props {
                        all_entries.push(RsExpr::Array(vec![
                            RsExpr::Lit(RsLit::Str(k)),
                            v,
                        ]));
                    }
                    RsExpr::MethodCall(
                        Box::new(RsExpr::Lit(RsLit::Null)),
                        "collect".to_string(),
                        all_entries,
                    )
                }
            }
            SwcExpr::Tpl(tpl) => {
                if tpl.exprs.is_empty() {
                    let mut s = String::new();
                    for quasi in &tpl.quasis {
                        s.push_str(&quasi.raw.to_string());
                    }
                    RsExpr::Lit(RsLit::Str(s))
                } else {
                    let mut format_str = String::new();
                    let mut args = Vec::new();
                    for (i, quasi) in tpl.quasis.iter().enumerate() {
                        let raw = quasi.raw.to_string();
                        format_str.push_str(&raw.replace('{', "{{").replace('}', "}}"));
                        if i < tpl.exprs.len() {
                            format_str.push_str("{}");
                            args.push(self.transform_expr(&tpl.exprs[i]));
                        }
                    }
                    RsExpr::Call(
                        Box::new(RsExpr::Ident("format".to_string())),
                        {
                            let mut call_args = vec![RsExpr::Lit(RsLit::Str(format_str))];
                            call_args.extend(args);
                            call_args
                        },
                    )
                }
            }
            SwcExpr::TaggedTpl(tagged) => {
                let tag = self.transform_expr(&tagged.tag);
                let mut format_str = String::new();
                let mut args = Vec::new();
                for (i, quasi) in tagged.tpl.quasis.iter().enumerate() {
                    let raw = quasi.raw.to_string();
                    format_str.push_str(&raw.replace('{', "{{").replace('}', "}}"));
                    if i < tagged.tpl.exprs.len() {
                        format_str.push_str("{}");
                        args.push(self.transform_expr(&tagged.tpl.exprs[i]));
                    }
                }
                let mut call_args = vec![RsExpr::Lit(RsLit::Str(format_str))];
                call_args.extend(args);
                RsExpr::Call(Box::new(tag), call_args)
            }
            SwcExpr::Cond(cond) => RsExpr::If(
                Box::new(self.transform_expr(&cond.test)),
                vec![RsStmt::Expr(self.transform_expr(&cond.cons))],
                Some(vec![RsStmt::Expr(self.transform_expr(&cond.alt))]),
            ),
            SwcExpr::Seq(seq) => seq
                .exprs
                .last()
                .map(|e| self.transform_expr(e))
                .unwrap_or(RsExpr::Lit(RsLit::Null)),
            SwcExpr::Paren(paren) => self.transform_expr(&paren.expr),
            SwcExpr::OptChain(opt) => {
                let inner = match &*opt.base {
                    swc_ecma_ast::OptChainBase::Member(member) => {
                        let obj = self.transform_expr(&member.obj);
                        match &member.prop {
                            MemberProp::Ident(id) => {
                                let prop = atom_to_string(&id.sym);
                                RsExpr::Member(Box::new(obj), prop)
                            }
                            MemberProp::Computed(computed) => {
                                let key = self.transform_expr(&computed.expr);
                                RsExpr::Index(Box::new(obj), Box::new(key))
                            }
                            _ => RsExpr::Lit(RsLit::Null),
                        }
                    }
                    swc_ecma_ast::OptChainBase::Call(call) => {
                        let callee = self.transform_expr(&call.callee);
                        let args: Vec<RsExpr> = call.args.iter().map(|a| self.transform_expr(&a.expr)).collect();
                        RsExpr::Call(Box::new(callee), args)
                    }
                };
                RsExpr::OptionalChain(Box::new(inner))
            }
            SwcExpr::Update(update) => {
                let arg = self.transform_expr(&update.arg);
                let one = RsExpr::Lit(RsLit::I64(1));
                match update.op {
                    swc_ecma_ast::UpdateOp::PlusPlus => {
                        RsExpr::Assign(AssignOp::AddAssign, Box::new(arg), Box::new(one))
                    }
                    swc_ecma_ast::UpdateOp::MinusMinus => {
                        RsExpr::Assign(AssignOp::SubAssign, Box::new(arg), Box::new(one))
                    }
                }
            }
            SwcExpr::Await(await_expr) => {
                RsExpr::Await(Box::new(self.transform_expr(&await_expr.arg)))
            }
            SwcExpr::Yield(yield_expr) => {
                if let Some(arg) = &yield_expr.arg {
                    RsExpr::Call(
                        Box::new(RsExpr::Ident("yield".to_string())),
                        vec![self.transform_expr(arg)],
                    )
                } else {
                    RsExpr::Call(Box::new(RsExpr::Ident("yield".to_string())), Vec::new())
                }
            }
            SwcExpr::This(_) => RsExpr::Ident("self".to_string()),
            _ => RsExpr::Lit(RsLit::Null),
        }
    }

    fn transform_simple_assign_target(&self, target: &swc_ecma_ast::SimpleAssignTarget) -> RsExpr {
        match target {
            swc_ecma_ast::SimpleAssignTarget::Ident(id) => {
                RsExpr::Ident(atom_to_string(&id.id.sym))
            }
            swc_ecma_ast::SimpleAssignTarget::Member(member) => {
                let obj = self.transform_expr(&member.obj);
                match &member.prop {
                    MemberProp::Ident(id) => RsExpr::Member(Box::new(obj), atom_to_string(&id.sym)),
                    _ => RsExpr::Member(Box::new(obj), "unknown".to_string()),
                }
            }
            swc_ecma_ast::SimpleAssignTarget::Paren(paren) => self.transform_expr(&paren.expr),
            _ => RsExpr::Lit(RsLit::Null),
        }
    }

    fn transform_lit(&self, lit: &SwcLit) -> RsLit {
        match lit {
            SwcLit::Num(n) => {
                if n.value.fract() == 0.0 {
                    RsLit::I64(n.value as i64)
                } else {
                    RsLit::F64(n.value)
                }
            }
            SwcLit::Str(s) => RsLit::Str(wtf8_to_string(&s.value)),
            SwcLit::Bool(b) => RsLit::Bool(b.value),
            SwcLit::Null(_) => RsLit::Null,
            SwcLit::Regex(r) => RsLit::Str(atom_to_string(&r.exp)),
            _ => RsLit::Null,
        }
    }

    fn transform_lit_as_lit(&self, expr: &SwcExpr) -> RsLit {
        match expr {
            SwcExpr::Lit(lit) => self.transform_lit(lit),
            SwcExpr::Ident(id) => RsLit::Str(atom_to_string(&id.sym)),
            _ => RsLit::Null,
        }
    }

    fn transform_bin_op(&self, op: &swc_ecma_ast::BinaryOp) -> BinOp {
        match op {
            swc_ecma_ast::BinaryOp::Add => BinOp::Add,
            swc_ecma_ast::BinaryOp::Sub => BinOp::Sub,
            swc_ecma_ast::BinaryOp::Mul => BinOp::Mul,
            swc_ecma_ast::BinaryOp::Div => BinOp::Div,
            swc_ecma_ast::BinaryOp::Mod => BinOp::Mod,
            swc_ecma_ast::BinaryOp::EqEq => BinOp::Eq,
            swc_ecma_ast::BinaryOp::NotEq => BinOp::Neq,
            swc_ecma_ast::BinaryOp::EqEqEq => BinOp::StrictEq,
            swc_ecma_ast::BinaryOp::NotEqEq => BinOp::StrictNeq,
            swc_ecma_ast::BinaryOp::Lt => BinOp::Lt,
            swc_ecma_ast::BinaryOp::LtEq => BinOp::Lte,
            swc_ecma_ast::BinaryOp::Gt => BinOp::Gt,
            swc_ecma_ast::BinaryOp::GtEq => BinOp::Gte,
            swc_ecma_ast::BinaryOp::LogicalAnd => BinOp::And,
            swc_ecma_ast::BinaryOp::LogicalOr => BinOp::Or,
            swc_ecma_ast::BinaryOp::BitAnd => BinOp::BitAnd,
            swc_ecma_ast::BinaryOp::BitOr => BinOp::BitOr,
            swc_ecma_ast::BinaryOp::BitXor => BinOp::BitXor,
            swc_ecma_ast::BinaryOp::LShift => BinOp::Shl,
            swc_ecma_ast::BinaryOp::RShift => BinOp::Shr,
            swc_ecma_ast::BinaryOp::ZeroFillRShift => BinOp::UnsignedShr,
            _ => BinOp::Add,
        }
    }

    fn transform_unary_op(&self, op: &SwcUnaryOp) -> UnaryOp {
        match op {
            SwcUnaryOp::Minus => UnaryOp::Neg,
            SwcUnaryOp::Bang => UnaryOp::Not,
            SwcUnaryOp::Tilde => UnaryOp::BitNot,
            SwcUnaryOp::TypeOf => UnaryOp::TypeOf,
            SwcUnaryOp::Void => UnaryOp::Void,
            _ => UnaryOp::Not,
        }
    }

    fn transform_assign_op(&self, op: &swc_ecma_ast::AssignOp) -> AssignOp {
        match op {
            swc_ecma_ast::AssignOp::Assign => AssignOp::Assign,
            swc_ecma_ast::AssignOp::AddAssign => AssignOp::AddAssign,
            swc_ecma_ast::AssignOp::SubAssign => AssignOp::SubAssign,
            swc_ecma_ast::AssignOp::MulAssign => AssignOp::MulAssign,
            swc_ecma_ast::AssignOp::DivAssign => AssignOp::DivAssign,
            swc_ecma_ast::AssignOp::ModAssign => AssignOp::ModAssign,
            _ => AssignOp::Assign,
        }
    }

    fn transform_callee(&self, callee: &swc_ecma_ast::Callee) -> RsExpr {
        match callee {
            SwcCallee::Expr(expr) => self.transform_expr(expr),
            SwcCallee::Super(_) => RsExpr::Ident("super".to_string()),
            SwcCallee::Import(_) => RsExpr::Ident("import".to_string()),
        }
    }

    fn transform_prop_name_to_name(key: &PropName) -> Option<String> {
        match key {
            PropName::Ident(id) => Some(atom_to_string(&id.sym)),
            PropName::Str(s) => Some(wtf8_to_string(&s.value)),
            PropName::Num(n) => Some(format!("{}", n.value)),
            _ => None,
        }
    }

    fn transform_stmt(&self, stmt: &swc_ecma_ast::Stmt) -> Vec<RsStmt> {
        match stmt {
            swc_ecma_ast::Stmt::Decl(Decl::Fn(fn_decl)) => {
                let name = atom_to_string(&fn_decl.ident.sym);
                let sig = self.ctx.lookup_function(&name);
                let params: Vec<ParamDef> = fn_decl
                    .function
                    .params
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let pname = p.pat.as_ident()
                            .map(|id| atom_to_string(&id.id.sym))
                            .unwrap_or_else(|| format!("arg{}", i));
                        let ty = sig.and_then(|s| s.params.get(i).map(|pd| pd.ty.clone()))
                            .or_else(|| {
                                p.pat.as_ident().and_then(|id| {
                                    id.type_ann.as_ref().map(|t| self.ts_type_to_type(&t.type_ann))
                                })
                            })
                            .unwrap_or(Type::Any);
                        ParamDef { name: pname, ty, default: None }
                    })
                    .collect();
                let ret = sig.map(|s| s.return_type.clone()).unwrap_or_else(|| {
                    if fn_decl.function.return_type.is_some() {
                        Type::Any
                    } else {
                        Type::Void
                    }
                });
                let body = fn_decl
                    .function
                    .body
                    .as_ref()
                    .map(|b| self.transform_stmts(&b.stmts))
                    .unwrap_or_default();
                vec![RsStmt::Fn(FunctionDef {
                    name,
                    params,
                    return_type: ret,
                    body,
                    is_async: fn_decl.function.is_async,
                    generics: Vec::new(),
                })]
            }
            swc_ecma_ast::Stmt::Decl(Decl::Class(class_decl)) => {
                let name = atom_to_string(&class_decl.ident.sym);
                let mut fields = Vec::new();
                let mut methods = Vec::new();
                for member in &class_decl.class.body {
                    match member {
                        ClassMember::ClassProp(prop) => {
                            if let Some(key_name) = Self::transform_prop_name_to_name(&prop.key) {
                                fields.push(FieldDef {
                                    name: key_name,
                                    ty: Type::Any,
                                    is_pub: true,
                                });
                            }
                        }
                        ClassMember::Method(method) => {
                            if let Some(key_name) = Self::transform_prop_name_to_name(&method.key) {
                                let params: Vec<ParamDef> = method
                                    .function
                                    .params
                                    .iter()
                                    .enumerate()
                                    .map(|(i, p)| ParamDef {
                                        name: p
                                            .pat
                                            .as_ident()
                                            .map(|id| atom_to_string(&id.id.sym))
                                            .unwrap_or_else(|| format!("arg{}", i)),
                                        ty: Type::Any,
                                        default: None,
                                    })
                                    .collect();
                                let is_ctor = key_name == "constructor";
                                let body = method
                                    .function
                                    .body
                                    .as_ref()
                                    .map(|b| self.transform_stmts(&b.stmts))
                                    .unwrap_or_default();
                                methods.push(MethodDef {
                                    name: if is_ctor { "new".to_string() } else { key_name },
                                    params,
                                    return_type: if is_ctor {
                                        Type::Struct(StructDef {
                                            name: name.clone(),
                                            fields: Vec::new(),
                                            impls: Vec::new(),
                                        })
                                    } else {
                                        Type::Void
                                    },
                                    body,
                                    is_pub: true,
                                    is_async: method.function.is_async,
                                    self_param: if is_ctor || method.is_static {
                                        None
                                    } else {
                                        Some(SelfParam::SelfRef)
                                    },
                                });
                            }
                        }
                        _ => {}
                    }
                }
                vec![
                    RsStmt::Struct(StructDef {
                        name: name.clone(),
                        fields,
                        impls: Vec::new(),
                    }),
                    RsStmt::Impl(ImplDef {
                        self_type: Type::Struct(StructDef {
                            name,
                            fields: Vec::new(),
                            impls: Vec::new(),
                        }),
                        trait_name: None,
                        methods,
                        generics: Vec::new(),
                    }),
                ]
            }
            swc_ecma_ast::Stmt::Decl(Decl::Var(var_decl)) => var_decl
                .decls
                .iter()
                .flat_map(|decl| {
                    let init = decl
                        .init
                        .as_ref()
                        .map(|e| self.transform_expr(e))
                        .unwrap_or(RsExpr::Lit(RsLit::Null));
                    match &decl.name {
                        swc_ecma_ast::Pat::Ident(BindingIdent { id, .. }) => {
                            let name = atom_to_string(&id.sym);
                            let ty = self.ctx.lookup_var(&name).cloned().unwrap_or(Type::Any);
                            vec![match var_decl.kind {
                                swc_ecma_ast::VarDeclKind::Const => RsStmt::Let(name, ty, init),
                                _ => RsStmt::LetMut(name, ty, init),
                            }]
                        }
                        swc_ecma_ast::Pat::Array(arr) => {
                            arr.elems.iter().enumerate().filter_map(|(i, elem)| {
                                elem.as_ref().map(|pat| {
                                    let names = Self::pat_to_names(pat);
                                    let name = names.first().cloned().unwrap_or_else(|| format!("_{}", i));
                                    let ty = self.ctx.lookup_var(&name).cloned().unwrap_or(Type::Any);
                                    let elem_init = RsExpr::Index(
                                        Box::new(init.clone()),
                                        Box::new(RsExpr::Lit(RsLit::I64(i as i64))),
                                    );
                                    match var_decl.kind {
                                        swc_ecma_ast::VarDeclKind::Const => RsStmt::Let(name, ty, elem_init),
                                        _ => RsStmt::LetMut(name, ty, elem_init),
                                    }
                                })
                            }).collect()
                        }
                        swc_ecma_ast::Pat::Object(obj) => {
                            obj.props.iter().filter_map(|prop| {
                                match prop {
                                    swc_ecma_ast::ObjectPatProp::Assign(assign) => {
                                        let name = atom_to_string(&assign.key.id.sym);
                                        let ty = self.ctx.lookup_var(&name).cloned().unwrap_or(Type::Any);
                                        let prop_init = RsExpr::Member(
                                            Box::new(init.clone()),
                                            name.clone(),
                                        );
                                        Some(match var_decl.kind {
                                            swc_ecma_ast::VarDeclKind::Const => RsStmt::Let(name, ty, prop_init),
                                            _ => RsStmt::LetMut(name, ty, prop_init),
                                        })
                                    }
                                    swc_ecma_ast::ObjectPatProp::KeyValue(kv) => {
                                        if let swc_ecma_ast::PropName::Ident(key) = &kv.key {
                                            let key_name = atom_to_string(&key.sym);
                                            if let swc_ecma_ast::Pat::Ident(id) = &*kv.value {
                                                let name = atom_to_string(&id.id.sym);
                                                let ty = self.ctx.lookup_var(&name).cloned().unwrap_or(Type::Any);
                                                let prop_init = RsExpr::Member(
                                                    Box::new(init.clone()),
                                                    key_name,
                                                );
                                                Some(match var_decl.kind {
                                                    swc_ecma_ast::VarDeclKind::Const => RsStmt::Let(name, ty, prop_init),
                                                    _ => RsStmt::LetMut(name, ty, prop_init),
                                                })
                                            } else {
                                                None
                                            }
                                        } else {
                                            None
                                        }
                                    }
                                    _ => None,
                                }
                            }).collect()
                        }
                        _ => vec![],
                    }
                })
                .collect(),
            swc_ecma_ast::Stmt::Expr(expr_stmt) => {
                vec![RsStmt::Expr(self.transform_expr(&expr_stmt.expr))]
            }
            swc_ecma_ast::Stmt::Return(ret) => vec![RsStmt::Return(
                ret.arg.as_ref().map(|e| self.transform_expr(e)),
            )],
            swc_ecma_ast::Stmt::If(if_stmt) => {
                let test = self.transform_expr(&if_stmt.test);
                let cons: Vec<RsStmt> = self.transform_stmts(&block_stmts(&if_stmt.cons));
                let alt = if_stmt
                    .alt
                    .as_ref()
                    .map(|a| self.transform_stmts(&block_stmts(a)));
                vec![RsStmt::If(test, cons, alt)]
            }
            swc_ecma_ast::Stmt::For(for_stmt) => {
                let init = for_stmt.init.as_ref().map(|init| {
                    match init {
                        swc_ecma_ast::VarDeclOrExpr::VarDecl(decl) => {
                            if let Some(VarDeclarator {
                                name: swc_ecma_ast::Pat::Ident(BindingIdent { id, .. }),
                                init: Some(init_expr),
                                ..
                            }) = decl.decls.first()
                            {
                                let init_expr = self.transform_expr(init_expr);
                                let name = atom_to_string(&id.sym);
                                let ty = self.ctx.lookup_var(&name).cloned().unwrap_or(Type::Any);
                                match decl.kind {
                                    swc_ecma_ast::VarDeclKind::Const => Box::new(RsStmt::Let(name, ty, init_expr)),
                                    _ => Box::new(RsStmt::LetMut(name, ty, init_expr)),
                                }
                            } else {
                                Box::new(RsStmt::Empty)
                            }
                        }
                        swc_ecma_ast::VarDeclOrExpr::Expr(expr) => {
                            Box::new(RsStmt::Expr(self.transform_expr(expr)))
                        }
                    }
                });
                let test = for_stmt.test.as_ref().map(|e| self.transform_expr(e));
                let update = for_stmt.update.as_ref().map(|e| self.transform_expr(e));
                let body = self.transform_stmts(&block_stmts(&for_stmt.body));
                vec![RsStmt::ForLoop { init, test, update, body }]
            }
            swc_ecma_ast::Stmt::While(while_stmt) => {
                let test = self.transform_expr(&while_stmt.test);
                let body = self.transform_stmts(&block_stmts(&while_stmt.body));
                vec![RsStmt::While(test, body)]
            }
            swc_ecma_ast::Stmt::Try(try_stmt) => {
                let try_body = self.transform_stmts(&try_stmt.block.stmts);
                if let Some(handler) = &try_stmt.handler {
                    let catch_name = handler
                        .param
                        .as_ref()
                        .map(|p| {
                            if let swc_ecma_ast::Pat::Ident(id) = p {
                                atom_to_string(&id.id.sym)
                            } else {
                                "e".to_string()
                            }
                        })
                        .unwrap_or_else(|| "e".to_string());
                    let catch_body = self.transform_stmts(&handler.body.stmts);
                    vec![RsStmt::Try(try_body, catch_name, catch_body)]
                } else {
                    try_body
                }
            }
            swc_ecma_ast::Stmt::Throw(throw) => {
                vec![RsStmt::Throw(self.transform_expr(&throw.arg))]
            }
            swc_ecma_ast::Stmt::Break(_) => vec![RsStmt::Break],
            swc_ecma_ast::Stmt::Continue(_) => vec![RsStmt::Continue],
            swc_ecma_ast::Stmt::Switch(switch) => {
                let discriminant = self.transform_expr(&switch.discriminant);
                let arms: Vec<MatchArm> = switch
                    .cases
                    .iter()
                    .map(|case| {
                        let pattern = case
                            .test
                            .as_ref()
                            .map(|t| Pattern::Lit(self.transform_lit_as_lit(t)))
                            .unwrap_or(Pattern::Wildcard);
                        let stmts: Vec<RsStmt> = case
                            .cons
                            .iter()
                            .flat_map(|s| self.transform_stmt(s))
                            .collect();
                        let body = if stmts.len() == 1 {
                            match &stmts[0] {
                                RsStmt::Expr(e) => e.clone(),
                                _ => RsExpr::Block(stmts),
                            }
                        } else {
                            RsExpr::Block(stmts)
                        };
                        MatchArm {
                            pattern,
                            guard: None,
                            body,
                        }
                    })
                    .collect();
                vec![RsStmt::Match(discriminant, arms)]
            }
            swc_ecma_ast::Stmt::DoWhile(while_stmt) => {
                let test = self.transform_expr(&while_stmt.test);
                let body = self.transform_stmts(&block_stmts(&while_stmt.body));
                vec![RsStmt::Loop({
                    let mut full_body = body;
                    full_body.push(RsStmt::If(
                        test,
                        vec![],
                        Some(vec![RsStmt::Break]),
                    ));
                    full_body
                })]
            }
            swc_ecma_ast::Stmt::Empty(_) => vec![RsStmt::Empty],
            swc_ecma_ast::Stmt::ForIn(for_in) => {
                let name = match &for_in.left {
                    swc_ecma_ast::ForHead::VarDecl(decl) => {
                        decl.decls.first().and_then(|d| {
                            if let swc_ecma_ast::Pat::Ident(id) = &d.name {
                                Some(atom_to_string(&id.id.sym))
                            } else {
                                None
                            }
                        }).unwrap_or_else(|| "_".to_string())
                    }
                    swc_ecma_ast::ForHead::Pat(pat) => {
                        if let swc_ecma_ast::Pat::Ident(id) = &**pat {
                            atom_to_string(&id.sym)
                        } else {
                            "_".to_string()
                        }
                    }
                    _ => "_".to_string(),
                };
                let right = self.transform_expr(&for_in.right);
                let body = self.transform_stmts(&block_stmts(&for_in.body));
                vec![RsStmt::ForIn(name, right, body)]
            }
            swc_ecma_ast::Stmt::ForOf(for_of) => {
                let name = match &for_of.left {
                    swc_ecma_ast::ForHead::VarDecl(decl) => {
                        decl.decls.first().and_then(|d| {
                            if let swc_ecma_ast::Pat::Ident(id) = &d.name {
                                Some(atom_to_string(&id.id.sym))
                            } else {
                                None
                            }
                        }).unwrap_or_else(|| "_".to_string())
                    }
                    swc_ecma_ast::ForHead::Pat(pat) => {
                        if let swc_ecma_ast::Pat::Ident(id) = &**pat {
                            atom_to_string(&id.sym)
                        } else {
                            "_".to_string()
                        }
                    }
                    _ => "_".to_string(),
                };
                let right = self.transform_expr(&for_of.right);
                let body = self.transform_stmts(&block_stmts(&for_of.body));
                vec![RsStmt::ForIn(name, right, body)]
            }
            _ => vec![RsStmt::Empty],
        }
    }

    fn transform_stmts(&self, stmts: &[swc_ecma_ast::Stmt]) -> Vec<RsStmt> {
        stmts.iter().flat_map(|s| self.transform_stmt(s)).collect()
    }

    pub fn transform_module(&mut self, module: &ParsedModule) -> Result<RustModule> {
        let mut all_stmts = Vec::new();
        let mut imports = Vec::new();
        for item in &module.body {
            match item {
                ModuleItem::ModuleDecl(ModuleDecl::Import(import)) => {
                    let path = wtf8_to_string(&import.src.value);
                    let items: Vec<String> = import
                        .specifiers
                        .iter()
                        .filter_map(|s| match s {
                            ImportSpecifier::Named(n) => Some(atom_to_string(&n.local.sym)),
                            ImportSpecifier::Default(d) => Some(atom_to_string(&d.local.sym)),
                            ImportSpecifier::Namespace(ns) => {
                                Some(format!("* as {}", atom_to_string(&ns.local.sym)))
                            }
                        })
                        .collect();
                    imports.push(RsImport {
                        path,
                        items,
                        is_glob: import
                            .specifiers
                            .iter()
                            .any(|s| matches!(s, ImportSpecifier::Namespace(..))),
                    });
                }
                ModuleItem::Stmt(stmt) => {
                    all_stmts.extend(self.transform_stmt(stmt));
                }
                ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export)) => {
                    let stmt = swc_ecma_ast::Stmt::Decl(export.decl.clone());
                    let mut stmts = self.transform_stmt(&stmt);
                    for s in &mut stmts {
                        *s = RsStmt::Pub(Box::new(s.clone()));
                    }
                    all_stmts.extend(stmts);
                }
                _ => {}
            }
        }
        Ok(RustModule {
            name: "main".to_string(),
            imports,
            items: all_stmts,
        })
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
                    match atom_to_string(&id.sym).as_str() {
                        "Array" | "Vec" => Type::Vec(Box::new(Type::Any)),
                        "Promise" => Type::Any,
                        "Map" => Type::HashMap(Box::new(Type::Any), Box::new(Type::Any)),
                        "Set" => Type::Vec(Box::new(Type::Any)),
                        _ => Type::Struct(StructDef {
                            name: atom_to_string(&id.sym),
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
            _ => Type::Any,
        }
    }
}

pub fn transform(analyzed: &(crate::context::Context, Vec<RsStmt>)) -> Result<RustModule> {
    let (_ctx, stmts) = analyzed;
    Ok(RustModule {
        name: "main".to_string(),
        imports: Vec::new(),
        items: stmts.clone(),
    })
}
