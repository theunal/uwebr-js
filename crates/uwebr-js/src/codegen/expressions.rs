use crate::codegen::CodeGen;
use crate::types::*;

pub fn generate_expression(codegen: &mut CodeGen, expr: &RsExpr) {
    match expr {
        RsExpr::Lit(lit) => generate_literal(codegen, lit),
        RsExpr::Ident(name) => {
            codegen.write(&js_name_to_rust(name));
        }
        RsExpr::Path(parts) => {
            codegen.write(&parts.join("::"));
        }
        RsExpr::Binary(op, left, right) => {
            codegen.write("(");
            generate_expression(codegen, left);
            codegen.write(&format!(" {} ", bin_op_to_str(op)));
            generate_expression(codegen, right);
            codegen.write(")");
        }
        RsExpr::Unary(op, arg) => {
            match op {
                UnaryOp::Neg => codegen.write("-"),
                UnaryOp::Not => codegen.write("!"),
                UnaryOp::BitNot => codegen.write("!"),
                UnaryOp::TypeOf => {
                    codegen.write("std::any::type_name_of_val(&");
                    generate_expression(codegen, arg);
                    codegen.write(")");
                    return;
                }
                UnaryOp::Void => {
                    codegen.write("let _ = ");
                    generate_expression(codegen, arg);
                    codegen.write(";");
                    return;
                }
                _ => codegen.write(""),
            };
            generate_expression(codegen, arg);
        }
        RsExpr::Assign(op, left, right) => {
            generate_expression(codegen, left);
            codegen.write(&format!(" {} ", assign_op_to_str(op)));
            generate_expression(codegen, right);
        }
        RsExpr::Call(callee, args) => {
            if let RsExpr::Ident(name) = &**callee {
                match name.as_str() {
                    "console_log" | "console.log" => {
                        if args.is_empty() {
                            codegen.write("println!()");
                        } else if args.len() == 1 {
                            codegen.write("println!(\"{}\", ");
                            generate_expression(codegen, &args[0]);
                            codegen.write(")");
                        } else {
                            codegen.write("println!(\"");
                            for _ in 0..args.len() {
                                codegen.write("{} ");
                            }
                            codegen.write("\", ");
                            for (i, arg) in args.iter().enumerate() {
                                if i > 0 {
                                    codegen.write(", ");
                                }
                                generate_expression(codegen, arg);
                            }
                            codegen.write(")");
                        }
                        return;
                    }
                    "console_error" | "console.error" => {
                        if args.is_empty() {
                            codegen.write("eprintln!()");
                        } else if args.len() == 1 {
                            codegen.write("eprintln!(\"{}\", ");
                            generate_expression(codegen, &args[0]);
                            codegen.write(")");
                        } else {
                            codegen.write("eprintln!(\"");
                            for _ in 0..args.len() {
                                codegen.write("{} ");
                            }
                            codegen.write("\", ");
                            for (i, arg) in args.iter().enumerate() {
                                if i > 0 {
                                    codegen.write(", ");
                                }
                                generate_expression(codegen, arg);
                            }
                            codegen.write(")");
                        }
                        return;
                    }
                    "console_warn" | "console.warn" => {
                        if args.is_empty() {
                            codegen.write("eprintln!(\"[WARN]\")");
                        } else if args.len() == 1 {
                            codegen.write("eprintln!(\"[WARN] {}\", ");
                            generate_expression(codegen, &args[0]);
                            codegen.write(")");
                        } else {
                            codegen.write("eprintln!(\"[WARN] ");
                            for _ in 0..args.len() {
                                codegen.write("{} ");
                            }
                            codegen.write("\", ");
                            for (i, arg) in args.iter().enumerate() {
                                if i > 0 {
                                    codegen.write(", ");
                                }
                                generate_expression(codegen, arg);
                            }
                            codegen.write(")");
                        }
                        return;
                    }
                    _ => {}
                }
            }
            if let RsExpr::Ident(name) = &**callee {
                if name == "format" {
                    codegen.write("format!(");
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            codegen.write(", ");
                        }
                        generate_expression(codegen, arg);
                    }
                    codegen.write(")");
                    return;
                }
                if name == "vec" && args.len() == 2 && matches!(&args[0], RsExpr::Lit(RsLit::Null))
                {
                    codegen.write("vec![Default::default(); ");
                    generate_expression(codegen, &args[1]);
                    codegen.write("]");
                    return;
                }
            }
            generate_expression(codegen, callee);
            codegen.write("(");
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    codegen.write(", ");
                }
                generate_expression(codegen, arg);
            }
            codegen.write(")");
        }
        RsExpr::New(name, args) => {
            let rust_name = js_name_to_rust(name);
            codegen.write(&format!("{}::new(", rust_name));
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    codegen.write(", ");
                }
                generate_expression(codegen, arg);
            }
            codegen.write(")");
        }
        RsExpr::Member(obj, prop) => {
            generate_expression(codegen, obj);
            codegen.write(&format!(".{}", js_name_to_rust(prop)));
        }
        RsExpr::Index(obj, key) => {
            generate_expression(codegen, obj);
            codegen.write("[");
            generate_expression(codegen, key);
            codegen.write("]");
        }
        RsExpr::ArrowFunction(params, _ret, body) => {
            codegen.write("|");
            for (i, param) in params.iter().enumerate() {
                if i > 0 {
                    codegen.write(", ");
                }
                codegen.write(&format!("{}: {}", param.name, param.ty.to_rust_string()));
            }
            codegen.write("|");
            if body.len() == 1 {
                match &body[0] {
                    RsStmt::Expr(expr) => {
                        codegen.write(" ");
                        generate_expression(codegen, expr);
                    }
                    _ => {
                        codegen.write(" {");
                        codegen.indent();
                        codegen.output.push('\n');
                        for stmt in body {
                            codegen.generate_stmt(stmt);
                        }
                        codegen.dedent();
                        codegen.write_indent();
                        codegen.write("}");
                    }
                }
            } else {
                codegen.write(" {");
                codegen.indent();
                codegen.output.push('\n');
                for stmt in body {
                    codegen.generate_stmt(stmt);
                }
                codegen.dedent();
                codegen.write_indent();
                codegen.write("}");
            }
        }
        RsExpr::FunctionExpr(name, params, _ret, body) => {
            if let Some(name) = name {
                codegen.write(&format!("fn {}(", name));
            } else {
                codegen.write("fn(");
            }
            for (i, param) in params.iter().enumerate() {
                if i > 0 {
                    codegen.write(", ");
                }
                codegen.write(&format!("{}: {}", param.name, param.ty.to_rust_string()));
            }
            codegen.write(") {");
            codegen.indent();
            codegen.output.push('\n');
            for stmt in body {
                codegen.generate_stmt(stmt);
            }
            codegen.dedent();
            codegen.write_indent();
            codegen.write("}");
        }
        RsExpr::If(test, cons, alt) => {
            codegen.write("if ");
            generate_expression(codegen, test);
            codegen.writeln(" {");
            codegen.indent();
            for stmt in cons {
                codegen.generate_stmt(stmt);
            }
            codegen.dedent();
            if let Some(alt_stmts) = alt {
                codegen.write_indent();
                codegen.write("} else {");
                codegen.output.push('\n');
                codegen.indent();
                for stmt in alt_stmts {
                    codegen.generate_stmt(stmt);
                }
                codegen.dedent();
                codegen.write_indent();
                codegen.write("}");
            } else {
                codegen.write_indent();
                codegen.write("}");
            }
        }
        RsExpr::Array(elems) => {
            let has_spread = elems.iter().any(|e| matches!(e, RsExpr::Spread(_)));
            if has_spread {
                codegen.write("vec![");
                let mut first = true;
                for elem in elems {
                    match elem {
                        RsExpr::Spread(inner) => {
                            for item in inner {
                                if !first {
                                    codegen.write(", ");
                                }
                                codegen.write("&");
                                generate_expression(codegen, item);
                                codegen.write("[..]");
                                first = false;
                            }
                        }
                        _ => {
                            if !first {
                                codegen.write(", ");
                            }
                            generate_expression(codegen, elem);
                            first = false;
                        }
                    }
                }
                codegen.write("].concat()");
            } else if elems.is_empty() {
                codegen.write("Vec::new()");
            } else {
                codegen.write("vec![");
                for (i, elem) in elems.iter().enumerate() {
                    if i > 0 {
                        codegen.write(", ");
                    }
                    generate_expression(codegen, elem);
                }
                codegen.write("]");
            }
        }
        RsExpr::Object(props) => {
            codegen.write("HashMap::from([");
            for (i, (key, value)) in props.iter().enumerate() {
                if i > 0 {
                    codegen.write(", ");
                }
                codegen.write(&format!("(\"{}\".to_string(), ", key));
                generate_expression(codegen, value);
                codegen.write(")");
            }
            codegen.write("])");
        }
        RsExpr::Tuple(elems) => {
            codegen.write("(");
            for (i, elem) in elems.iter().enumerate() {
                if i > 0 {
                    codegen.write(", ");
                }
                generate_expression(codegen, elem);
            }
            codegen.write(")");
        }
        RsExpr::StructLiteral(name, fields) => {
            codegen.write(&format!("{} {{ ", js_name_to_rust(name)));
            for (i, (key, value)) in fields.iter().enumerate() {
                if i > 0 {
                    codegen.write(", ");
                }
                codegen.write(&format!("{}: ", js_name_to_rust(key)));
                generate_expression(codegen, value);
            }
            codegen.write(" }");
        }
        RsExpr::FieldAccess(obj, field) => {
            generate_expression(codegen, obj);
            codegen.write(&format!(".{}", js_name_to_rust(field)));
        }
        RsExpr::MethodCall(obj, method, args) => {
            if method == "collect" && !args.is_empty() {
                let mut has_spread = false;
                for arg in args {
                    if let RsExpr::MethodCall(_, inner_method, _) = arg {
                        if inner_method == "iter" {
                            has_spread = true;
                            break;
                        }
                    }
                }
                if has_spread {
                    let mut spread_args = Vec::new();
                    let mut regular_entries = Vec::new();
                    for arg in args {
                        if let RsExpr::MethodCall(inner, inner_method, _) = arg {
                            if inner_method == "iter" {
                                spread_args.push(&**inner);
                            } else if let RsExpr::Array(arr) = arg {
                                regular_entries.push(arr);
                            }
                        } else if let RsExpr::Array(arr) = arg {
                            regular_entries.push(arr);
                        }
                    }
                    codegen.write("HashMap::from_iter(");
                    for (i, spread) in spread_args.iter().enumerate() {
                        if i > 0 {
                            codegen.write(".chain(");
                            generate_expression(codegen, spread);
                            codegen.write(".into_iter())");
                        } else {
                            generate_expression(codegen, spread);
                            codegen.write(".into_iter()");
                        }
                    }
                    if !regular_entries.is_empty() {
                        if !spread_args.is_empty() {
                            codegen.write(".chain(");
                        }
                        codegen.write("[");
                        for (i, entry) in regular_entries.iter().enumerate() {
                            if i > 0 {
                                codegen.write(", ");
                            }
                            codegen.write("(");
                            if let Some(key_expr) = entry.first() {
                                generate_expression(codegen, key_expr);
                                codegen.write(".to_string()");
                                codegen.write(", ");
                            }
                            if let Some(val_expr) = entry.get(1) {
                                generate_expression(codegen, val_expr);
                            }
                            codegen.write(")");
                        }
                        codegen.write("].into_iter()");
                        if !spread_args.is_empty() {
                            codegen.write(")");
                        }
                    }
                    codegen.write(")");
                    return;
                }
            }
            match method.as_str() {
                "toLowerCase" => {
                    generate_expression(codegen, obj);
                    codegen.write(".to_lowercase()");
                    return;
                }
                "toUpperCase" => {
                    generate_expression(codegen, obj);
                    codegen.write(".to_uppercase()");
                    return;
                }
                "trim" => {
                    generate_expression(codegen, obj);
                    codegen.write(".trim()");
                    return;
                }
                "includes" => {
                    generate_expression(codegen, obj);
                    codegen.write(".contains(");
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            codegen.write(", ");
                        }
                        generate_expression(codegen, arg);
                    }
                    codegen.write(")");
                    return;
                }
                "repeat" => {
                    generate_expression(codegen, obj);
                    codegen.write(".repeat(");
                    if let Some(first) = args.first() {
                        generate_expression(codegen, first);
                    }
                    codegen.write(")");
                    return;
                }
                "replace" => {
                    generate_expression(codegen, obj);
                    codegen.write(".replace(");
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            codegen.write(", ");
                        }
                        generate_expression(codegen, arg);
                    }
                    codegen.write(")");
                    return;
                }
                "split" => {
                    generate_expression(codegen, obj);
                    codegen.write(".split(");
                    if let Some(first) = args.first() {
                        generate_expression(codegen, first);
                    }
                    codegen.write(").collect::<Vec<_>>()");
                    return;
                }
                "charAt" => {
                    generate_expression(codegen, obj);
                    codegen.write(".chars().nth(");
                    if let Some(first) = args.first() {
                        generate_expression(codegen, first);
                    }
                    codegen.write(")");
                    return;
                }
                "substring" | "slice" => {
                    generate_expression(codegen, obj);
                    codegen.write("[");
                    if let Some(start) = args.first() {
                        generate_expression(codegen, start);
                    }
                    codegen.write("..");
                    if let Some(end) = args.get(1) {
                        generate_expression(codegen, end);
                    }
                    codegen.write("]");
                    return;
                }
                "indexOf" => {
                    generate_expression(codegen, obj);
                    codegen.write(".position(|c| c == ");
                    if let Some(first) = args.first() {
                        generate_expression(codegen, first);
                    }
                    codegen.write(")");
                    return;
                }
                "length" => {
                    generate_expression(codegen, obj);
                    codegen.write(".len()");
                    return;
                }
                "unwrap_or_default" | "unwrap_or_else" | "unwrap_or" => {
                    generate_expression(codegen, obj);
                    codegen.write(&format!(".{}(", method));
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            codegen.write(", ");
                        }
                        generate_expression(codegen, arg);
                    }
                    codegen.write(")");
                    return;
                }
                _ => {}
            }
            generate_expression(codegen, obj);
            codegen.write(&format!(".{}(", js_name_to_rust(method)));
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    codegen.write(", ");
                }
                generate_expression(codegen, arg);
            }
            codegen.write(")");
        }
        RsExpr::OptionalChain(inner) => match &**inner {
            RsExpr::Member(obj, prop) => {
                generate_expression(codegen, obj);
                codegen.write(&format!(".as_ref().map(|v| &v.{})", js_name_to_rust(prop)));
            }
            RsExpr::Index(obj, key) => {
                generate_expression(codegen, obj);
                codegen.write(".as_ref().map(|v| &v[");
                generate_expression(codegen, key);
                codegen.write("])");
            }
            RsExpr::Call(callee, args) => {
                generate_expression(codegen, callee);
                codegen.write(".as_ref().map(|f| f(");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        codegen.write(", ");
                    }
                    generate_expression(codegen, arg);
                }
                codegen.write("))");
            }
            _ => generate_expression(codegen, inner),
        },
        RsExpr::NullishCoalesce(left, right) => {
            generate_expression(codegen, left);
            codegen.write(".unwrap_or(");
            generate_expression(codegen, right);
            codegen.write(")");
        }
        RsExpr::Await(inner) => {
            generate_expression(codegen, inner);
            codegen.write(".await");
        }
        RsExpr::Range(start, end) => {
            generate_expression(codegen, start);
            codegen.write("..");
            generate_expression(codegen, end);
        }
        RsExpr::RangeInclusive(start, end) => {
            generate_expression(codegen, start);
            codegen.write("..=");
            generate_expression(codegen, end);
        }
        RsExpr::Reference(inner) => {
            codegen.write("&");
            generate_expression(codegen, inner);
        }
        RsExpr::Deref(inner) => {
            codegen.write("*");
            generate_expression(codegen, inner);
        }
        RsExpr::TypeAscription(inner, ty) => {
            codegen.write(&format!("(<{}>", ty.to_rust_string()));
            generate_expression(codegen, inner);
            codegen.write(")");
        }
        RsExpr::Block(stmts) => {
            codegen.write("{");
            codegen.indent();
            codegen.output.push('\n');
            for stmt in stmts {
                codegen.generate_stmt(stmt);
            }
            codegen.dedent();
            codegen.write_indent();
            codegen.write("}");
        }
        RsExpr::Spread(elems) => {
            if elems.len() == 1 {
                generate_expression(codegen, &elems[0]);
            } else {
                for (i, elem) in elems.iter().enumerate() {
                    if i > 0 {
                        codegen.write(", ");
                    }
                    generate_expression(codegen, elem);
                }
            }
        }
        RsExpr::Closure(closure_params, body) => {
            codegen.write("|");
            for (i, param) in closure_params.iter().enumerate() {
                if i > 0 {
                    codegen.write(", ");
                }
                if param.by_ref {
                    codegen.write("&");
                }
                if param.is_mutable {
                    codegen.write("mut ");
                }
                codegen.write(&param.name);
            }
            codegen.write("| {");
            codegen.indent();
            codegen.output.push('\n');
            generate_expression(codegen, body);
            codegen.dedent();
            codegen.write_indent();
            codegen.write("}");
        }
        RsExpr::AsyncBlock(body) => {
            codegen.write("async {");
            codegen.indent();
            codegen.output.push('\n');
            for stmt in body {
                codegen.generate_stmt(stmt);
            }
            codegen.dedent();
            codegen.write_indent();
            codegen.write("}");
        }
        _ => {
            codegen.write("/* unsupported */");
        }
    }
}

fn generate_literal(codegen: &mut CodeGen, lit: &RsLit) {
    match lit {
        RsLit::Bool(b) => codegen.write(if *b { "true" } else { "false" }),
        RsLit::I64(n) => codegen.write(&n.to_string()),
        RsLit::F64(n) => {
            codegen.write(&n.to_string());
            if n.fract() == 0.0 {
                codegen.write(".0");
            }
        }
        RsLit::Str(s) => codegen.write(&format!("\"{}\".to_string()", s)),
        RsLit::Null => codegen.write("None"),
        _ => codegen.write("/* unsupported literal */"),
    }
}

fn bin_op_to_str(op: &BinOp) -> &str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "%",
        BinOp::Eq => "==",
        BinOp::Neq => "!=",
        BinOp::StrictEq => "==",
        BinOp::StrictNeq => "!=",
        BinOp::Lt => "<",
        BinOp::Lte => "<=",
        BinOp::Gt => ">",
        BinOp::Gte => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
        BinOp::BitAnd => "&",
        BinOp::BitOr => "|",
        BinOp::BitXor => "^",
        BinOp::Shl => "<<",
        BinOp::Shr => ">>",
        BinOp::UnsignedShr => ">>",
        BinOp::Coalesce => "??",
    }
}

fn assign_op_to_str(op: &AssignOp) -> &str {
    match op {
        AssignOp::Assign => "=",
        AssignOp::AddAssign => "+=",
        AssignOp::SubAssign => "-=",
        AssignOp::MulAssign => "*=",
        AssignOp::DivAssign => "/=",
        AssignOp::ModAssign => "%=",
        AssignOp::AndAssign => "&=",
        AssignOp::OrAssign => "|=",
        AssignOp::XorAssign => "^=",
        AssignOp::ShlAssign => "<<=",
        AssignOp::ShrAssign => ">>=",
    }
}

fn js_name_to_rust(name: &str) -> String {
    match name {
        "console" => "println".to_string(),
        "null" | "undefined" => "None".to_string(),
        "true" => "true".to_string(),
        "false" => "false".to_string(),
        "this" => "self".to_string(),
        "super" => "self".to_string(),
        _ => {
            if name.starts_with('_') {
                return name.to_string();
            }
            let mut result = String::new();
            let mut capitalize_next = false;
            for c in name.chars() {
                if c == '_' {
                    capitalize_next = true;
                } else if capitalize_next {
                    result.push(c.to_uppercase().next().unwrap());
                    capitalize_next = false;
                } else {
                    result.push(c);
                }
            }
            result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TranspileOptions;

    fn codegen_expr(expr: &RsExpr) -> String {
        let options = TranspileOptions::default();
        let mut codegen = crate::codegen::CodeGen::new(&options);
        codegen.generate_expr(expr);
        codegen.output.clone()
    }

    #[test]
    fn js_codegen_binary_add() {
        let expr = RsExpr::Binary(
            BinOp::Add,
            Box::new(RsExpr::Lit(RsLit::I64(1))),
            Box::new(RsExpr::Lit(RsLit::I64(2))),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("1"), "got: {code}");
        assert!(code.contains("+"), "got: {code}");
        assert!(code.contains("2"), "got: {code}");
    }

    #[test]
    fn js_codegen_binary_sub() {
        let expr = RsExpr::Binary(
            BinOp::Sub,
            Box::new(RsExpr::Lit(RsLit::I64(10))),
            Box::new(RsExpr::Lit(RsLit::I64(3))),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("-"), "got: {code}");
    }

    #[test]
    fn js_codegen_binary_mul() {
        let expr = RsExpr::Binary(
            BinOp::Mul,
            Box::new(RsExpr::Lit(RsLit::I64(5))),
            Box::new(RsExpr::Lit(RsLit::I64(4))),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("*"), "got: {code}");
    }

    #[test]
    fn js_codegen_binary_div() {
        let expr = RsExpr::Binary(
            BinOp::Div,
            Box::new(RsExpr::Lit(RsLit::I64(20))),
            Box::new(RsExpr::Lit(RsLit::I64(4))),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("/"), "got: {code}");
    }

    #[test]
    fn js_codegen_binary_mod() {
        let expr = RsExpr::Binary(
            BinOp::Mod,
            Box::new(RsExpr::Lit(RsLit::I64(10))),
            Box::new(RsExpr::Lit(RsLit::I64(3))),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("%"), "got: {code}");
    }

    #[test]
    fn js_codegen_binary_eq() {
        let expr = RsExpr::Binary(
            BinOp::Eq,
            Box::new(RsExpr::Ident("a".into())),
            Box::new(RsExpr::Ident("b".into())),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("=="), "got: {code}");
    }

    #[test]
    fn js_codegen_binary_neq() {
        let expr = RsExpr::Binary(
            BinOp::Neq,
            Box::new(RsExpr::Ident("a".into())),
            Box::new(RsExpr::Ident("b".into())),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("!="), "got: {code}");
    }

    #[test]
    fn js_codegen_binary_lt() {
        let expr = RsExpr::Binary(
            BinOp::Lt,
            Box::new(RsExpr::Ident("a".into())),
            Box::new(RsExpr::Ident("b".into())),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("<"), "got: {code}");
    }

    #[test]
    fn js_codegen_binary_gt() {
        let expr = RsExpr::Binary(
            BinOp::Gt,
            Box::new(RsExpr::Ident("a".into())),
            Box::new(RsExpr::Ident("b".into())),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains(">"), "got: {code}");
    }

    #[test]
    fn js_codegen_binary_and() {
        let expr = RsExpr::Binary(
            BinOp::And,
            Box::new(RsExpr::Ident("a".into())),
            Box::new(RsExpr::Ident("b".into())),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("&&"), "got: {code}");
    }

    #[test]
    fn js_codegen_binary_or() {
        let expr = RsExpr::Binary(
            BinOp::Or,
            Box::new(RsExpr::Ident("a".into())),
            Box::new(RsExpr::Ident("b".into())),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("||"), "got: {code}");
    }

    #[test]
    fn js_codegen_unary_neg() {
        let expr = RsExpr::Unary(UnaryOp::Neg, Box::new(RsExpr::Lit(RsLit::I64(5))));
        let code = codegen_expr(&expr);
        assert!(code.contains("-"), "got: {code}");
        assert!(code.contains("5"), "got: {code}");
    }

    #[test]
    fn js_codegen_unary_not() {
        let expr = RsExpr::Unary(UnaryOp::Not, Box::new(RsExpr::Ident("flag".into())));
        let code = codegen_expr(&expr);
        assert!(code.contains("!"), "got: {code}");
    }

    #[test]
    fn js_codegen_unary_typeof() {
        let expr = RsExpr::Unary(UnaryOp::TypeOf, Box::new(RsExpr::Ident("x".into())));
        let code = codegen_expr(&expr);
        assert!(code.contains("type_name_of_val"), "got: {code}");
    }

    #[test]
    fn js_codegen_assign_simple() {
        let expr = RsExpr::Assign(
            AssignOp::Assign,
            Box::new(RsExpr::Ident("x".into())),
            Box::new(RsExpr::Lit(RsLit::I64(10))),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("="), "got: {code}");
        assert!(code.contains("10"), "got: {code}");
    }

    #[test]
    fn js_codegen_assign_add() {
        let expr = RsExpr::Assign(
            AssignOp::AddAssign,
            Box::new(RsExpr::Ident("x".into())),
            Box::new(RsExpr::Lit(RsLit::I64(5))),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("+="), "got: {code}");
    }

    #[test]
    fn js_codegen_call_simple() {
        let expr = RsExpr::Call(
            Box::new(RsExpr::Ident("add".into())),
            vec![RsExpr::Lit(RsLit::I64(1)), RsExpr::Lit(RsLit::I64(2))],
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("add(1, 2)"), "got: {code}");
    }

    #[test]
    fn js_codegen_call_console_log() {
        let expr = RsExpr::Call(
            Box::new(RsExpr::Ident("console_log".into())),
            vec![RsExpr::Lit(RsLit::Str("hello".into()))],
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("println!"), "got: {code}");
    }

    #[test]
    fn js_codegen_call_console_error() {
        let expr = RsExpr::Call(
            Box::new(RsExpr::Ident("console_error".into())),
            vec![RsExpr::Lit(RsLit::Str("oops".into()))],
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("eprintln!"), "got: {code}");
    }

    #[test]
    fn js_codegen_call_format() {
        let expr = RsExpr::Call(
            Box::new(RsExpr::Ident("format".into())),
            vec![
                RsExpr::Lit(RsLit::Str("hello {}".into())),
                RsExpr::Lit(RsLit::Str("world".into())),
            ],
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("format!"), "got: {code}");
    }

    #[test]
    fn js_codegen_call_vec_with_null() {
        let expr = RsExpr::Call(
            Box::new(RsExpr::Ident("vec".into())),
            vec![RsExpr::Lit(RsLit::Null), RsExpr::Lit(RsLit::I64(5))],
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("vec![Default::default(); 5]"), "got: {code}");
    }

    #[test]
    fn js_codegen_new_expression() {
        let expr = RsExpr::New("MyStruct".into(), vec![RsExpr::Lit(RsLit::I64(1))]);
        let code = codegen_expr(&expr);
        assert!(code.contains("MyStruct::new(1)"), "got: {code}");
    }

    #[test]
    fn js_codegen_member_access() {
        let expr = RsExpr::Member(Box::new(RsExpr::Ident("obj".into())), "prop".into());
        let code = codegen_expr(&expr);
        assert!(code.contains("obj.prop"), "got: {code}");
    }

    #[test]
    fn js_codegen_index_access() {
        let expr = RsExpr::Index(
            Box::new(RsExpr::Ident("arr".into())),
            Box::new(RsExpr::Lit(RsLit::I64(0))),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("arr[0]"), "got: {code}");
    }

    #[test]
    fn js_codegen_arrow_function_single_expr() {
        let expr = RsExpr::ArrowFunction(
            vec![ParamDef {
                name: "x".into(),
                ty: Type::I64,
                default: None,
            }],
            Type::I64,
            vec![RsStmt::Expr(RsExpr::Binary(
                BinOp::Add,
                Box::new(RsExpr::Ident("x".into())),
                Box::new(RsExpr::Lit(RsLit::I64(1))),
            ))],
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("|"), "got: {code}");
        assert!(code.contains("x"), "got: {code}");
    }

    #[test]
    fn js_codegen_arrow_function_multi_params() {
        let expr = RsExpr::ArrowFunction(
            vec![
                ParamDef {
                    name: "a".into(),
                    ty: Type::I64,
                    default: None,
                },
                ParamDef {
                    name: "b".into(),
                    ty: Type::I64,
                    default: None,
                },
            ],
            Type::I64,
            vec![RsStmt::Expr(RsExpr::Binary(
                BinOp::Add,
                Box::new(RsExpr::Ident("a".into())),
                Box::new(RsExpr::Ident("b".into())),
            ))],
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("|a: i64, b: i64|"), "got: {code}");
    }

    #[test]
    fn js_codegen_arrow_function_no_params() {
        let expr = RsExpr::ArrowFunction(
            vec![],
            Type::I64,
            vec![RsStmt::Return(Some(RsExpr::Lit(RsLit::I64(42))))],
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("||"), "got: {code}");
    }

    #[test]
    fn js_codegen_arrow_function_with_body() {
        let expr = RsExpr::ArrowFunction(
            vec![ParamDef {
                name: "x".into(),
                ty: Type::I64,
                default: None,
            }],
            Type::Void,
            vec![
                RsStmt::Let("y".into(), Type::I64, RsExpr::Lit(RsLit::I64(0))),
                RsStmt::Return(Some(RsExpr::Ident("y".into()))),
            ],
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("{"), "got: {code}");
        assert!(code.contains("let"), "got: {code}");
    }

    #[test]
    fn js_codegen_if_expression() {
        let expr = RsExpr::If(
            Box::new(RsExpr::Ident("cond".into())),
            vec![RsStmt::Expr(RsExpr::Lit(RsLit::I64(1)))],
            Some(vec![RsStmt::Expr(RsExpr::Lit(RsLit::I64(0)))]),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("if"), "got: {code}");
        assert!(code.contains("else"), "got: {code}");
    }

    #[test]
    fn js_codegen_array_empty() {
        let expr = RsExpr::Array(vec![]);
        let code = codegen_expr(&expr);
        assert!(code.contains("Vec::new()"), "got: {code}");
    }

    #[test]
    fn js_codegen_array_non_empty() {
        let expr = RsExpr::Array(vec![
            RsExpr::Lit(RsLit::I64(1)),
            RsExpr::Lit(RsLit::I64(2)),
            RsExpr::Lit(RsLit::I64(3)),
        ]);
        let code = codegen_expr(&expr);
        assert!(code.contains("vec!"), "got: {code}");
        assert!(code.contains("1"), "got: {code}");
    }

    #[test]
    fn js_codegen_array_with_spread() {
        let expr = RsExpr::Array(vec![RsExpr::Spread(vec![RsExpr::Ident("a".into())])]);
        let code = codegen_expr(&expr);
        assert!(code.contains("concat()"), "got: {code}");
    }

    #[test]
    fn js_codegen_object_literal() {
        let expr = RsExpr::Object(vec![
            ("x".into(), RsExpr::Lit(RsLit::I64(1))),
            ("y".into(), RsExpr::Lit(RsLit::I64(2))),
        ]);
        let code = codegen_expr(&expr);
        assert!(code.contains("HashMap::from"), "got: {code}");
        assert!(code.contains("\"x\""), "got: {code}");
    }

    #[test]
    fn js_codegen_tuple_literal() {
        let expr = RsExpr::Tuple(vec![
            RsExpr::Lit(RsLit::I64(1)),
            RsExpr::Lit(RsLit::Str("two".into())),
        ]);
        let code = codegen_expr(&expr);
        assert!(code.contains("("), "got: {code}");
        assert!(code.contains(")"), "got: {code}");
    }

    #[test]
    fn js_codegen_struct_literal() {
        let expr = RsExpr::StructLiteral(
            "Point".into(),
            vec![
                ("x".into(), RsExpr::Lit(RsLit::I64(1))),
                ("y".into(), RsExpr::Lit(RsLit::I64(2))),
            ],
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("Point"), "got: {code}");
        assert!(code.contains("x: 1"), "got: {code}");
    }

    #[test]
    fn js_codegen_method_call() {
        let expr = RsExpr::MethodCall(Box::new(RsExpr::Ident("v".into())), "iter".into(), vec![]);
        let code = codegen_expr(&expr);
        assert!(code.contains("v.iter()"), "got: {code}");
    }

    #[test]
    fn js_codegen_method_call_with_args() {
        let expr = RsExpr::MethodCall(
            Box::new(RsExpr::Ident("s".into())),
            "replace".into(),
            vec![
                RsExpr::Lit(RsLit::Str("a".into())),
                RsExpr::Lit(RsLit::Str("b".into())),
            ],
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("replace"), "got: {code}");
    }

    #[test]
    fn js_codegen_method_to_uppercase() {
        let expr = RsExpr::MethodCall(
            Box::new(RsExpr::Ident("s".into())),
            "toUpperCase".into(),
            vec![],
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("to_uppercase"), "got: {code}");
    }

    #[test]
    fn js_codegen_method_to_lowercase() {
        let expr = RsExpr::MethodCall(
            Box::new(RsExpr::Ident("s".into())),
            "toLowerCase".into(),
            vec![],
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("to_lowercase"), "got: {code}");
    }

    #[test]
    fn js_codegen_method_trim() {
        let expr = RsExpr::MethodCall(Box::new(RsExpr::Ident("s".into())), "trim".into(), vec![]);
        let code = codegen_expr(&expr);
        assert!(code.contains("trim"), "got: {code}");
    }

    #[test]
    fn js_codegen_method_includes() {
        let expr = RsExpr::MethodCall(
            Box::new(RsExpr::Ident("s".into())),
            "includes".into(),
            vec![RsExpr::Lit(RsLit::Str("x".into()))],
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("contains"), "got: {code}");
    }

    #[test]
    fn js_codegen_method_split() {
        let expr = RsExpr::MethodCall(
            Box::new(RsExpr::Ident("s".into())),
            "split".into(),
            vec![RsExpr::Lit(RsLit::Str(",".into()))],
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("split"), "got: {code}");
    }

    #[test]
    fn js_codegen_optional_chain_member() {
        let expr = RsExpr::OptionalChain(Box::new(RsExpr::Member(
            Box::new(RsExpr::Ident("obj".into())),
            "prop".into(),
        )));
        let code = codegen_expr(&expr);
        assert!(code.contains("as_ref"), "got: {code}");
        assert!(code.contains("map"), "got: {code}");
    }

    #[test]
    fn js_codegen_nullish_coalesce() {
        let expr = RsExpr::NullishCoalesce(
            Box::new(RsExpr::Ident("a".into())),
            Box::new(RsExpr::Ident("b".into())),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("unwrap_or"), "got: {code}");
    }

    #[test]
    fn js_codegen_await() {
        let expr = RsExpr::Await(Box::new(RsExpr::Ident("promise".into())));
        let code = codegen_expr(&expr);
        assert!(code.contains(".await"), "got: {code}");
    }

    #[test]
    fn js_codegen_range() {
        let expr = RsExpr::Range(
            Box::new(RsExpr::Lit(RsLit::I64(0))),
            Box::new(RsExpr::Lit(RsLit::I64(10))),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains(".."), "got: {code}");
        assert!(!code.contains("..="), "got: {code}");
    }

    #[test]
    fn js_codegen_range_inclusive() {
        let expr = RsExpr::RangeInclusive(
            Box::new(RsExpr::Lit(RsLit::I64(0))),
            Box::new(RsExpr::Lit(RsLit::I64(10))),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("..="), "got: {code}");
    }

    #[test]
    fn js_codegen_reference() {
        let expr = RsExpr::Reference(Box::new(RsExpr::Ident("x".into())));
        let code = codegen_expr(&expr);
        assert!(code.contains("&"), "got: {code}");
    }

    #[test]
    fn js_codegen_deref() {
        let expr = RsExpr::Deref(Box::new(RsExpr::Ident("x".into())));
        let code = codegen_expr(&expr);
        assert!(code.contains("*"), "got: {code}");
    }

    #[test]
    fn js_codegen_literal_bool_true() {
        let expr = RsExpr::Lit(RsLit::Bool(true));
        let code = codegen_expr(&expr);
        assert_eq!(code, "true");
    }

    #[test]
    fn js_codegen_literal_bool_false() {
        let expr = RsExpr::Lit(RsLit::Bool(false));
        let code = codegen_expr(&expr);
        assert_eq!(code, "false");
    }

    #[test]
    fn js_codegen_literal_i64() {
        let expr = RsExpr::Lit(RsLit::I64(42));
        let code = codegen_expr(&expr);
        assert_eq!(code, "42");
    }

    #[test]
    fn js_codegen_literal_f64() {
        let expr = RsExpr::Lit(RsLit::F64(3.14));
        let code = codegen_expr(&expr);
        assert!(code.contains("3.14"), "got: {code}");
    }

    #[test]
    fn js_codegen_literal_f64_integer() {
        let expr = RsExpr::Lit(RsLit::F64(5.0));
        let code = codegen_expr(&expr);
        assert!(code.contains(".0"), "got: {code}");
    }

    #[test]
    fn js_codegen_literal_string() {
        let expr = RsExpr::Lit(RsLit::Str("hello".into()));
        let code = codegen_expr(&expr);
        assert!(code.contains("\"hello\".to_string()"), "got: {code}");
    }

    #[test]
    fn js_codegen_literal_null() {
        let expr = RsExpr::Lit(RsLit::Null);
        let code = codegen_expr(&expr);
        assert_eq!(code, "None");
    }

    #[test]
    fn js_codegen_path() {
        let expr = RsExpr::Path(vec!["std".into(), "collections".into(), "HashMap".into()]);
        let code = codegen_expr(&expr);
        assert_eq!(code, "std::collections::HashMap");
    }

    #[test]
    fn js_codegen_ident_this() {
        let expr = RsExpr::Ident("this".into());
        let code = codegen_expr(&expr);
        assert_eq!(code, "self");
    }

    #[test]
    fn js_codegen_ident_null() {
        let expr = RsExpr::Ident("null".into());
        let code = codegen_expr(&expr);
        assert_eq!(code, "None");
    }

    #[test]
    fn js_codegen_ident_underscore() {
        let expr = RsExpr::Ident("_private".into());
        let code = codegen_expr(&expr);
        assert_eq!(code, "_private");
    }

    #[test]
    fn js_codegen_method_char_at() {
        let expr = RsExpr::MethodCall(
            Box::new(RsExpr::Ident("s".into())),
            "charAt".into(),
            vec![RsExpr::Lit(RsLit::I64(0))],
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("chars().nth(0)"), "got: {code}");
    }

    #[test]
    fn js_codegen_method_index_of() {
        let expr = RsExpr::MethodCall(
            Box::new(RsExpr::Ident("s".into())),
            "indexOf".into(),
            vec![RsExpr::Lit(RsLit::Str("x".into()))],
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("position"), "got: {code}");
    }

    #[test]
    fn js_codegen_method_length() {
        let expr = RsExpr::MethodCall(Box::new(RsExpr::Ident("v".into())), "length".into(), vec![]);
        let code = codegen_expr(&expr);
        assert!(code.contains(".len()"), "got: {code}");
    }

    #[test]
    fn js_codegen_method_repeat() {
        let expr = RsExpr::MethodCall(
            Box::new(RsExpr::Ident("s".into())),
            "repeat".into(),
            vec![RsExpr::Lit(RsLit::I64(3))],
        );
        let code = codegen_expr(&expr);
        assert!(code.contains(".repeat(3)"), "got: {code}");
    }

    #[test]
    fn js_codegen_method_substring() {
        let expr = RsExpr::MethodCall(
            Box::new(RsExpr::Ident("s".into())),
            "slice".into(),
            vec![RsExpr::Lit(RsLit::I64(0)), RsExpr::Lit(RsLit::I64(5))],
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("[0..5]"), "got: {code}");
    }

    #[test]
    fn js_codegen_closure_with_ref_param() {
        let expr = RsExpr::Closure(
            vec![ClosureParam {
                name: "x".into(),
                ty: Type::I64,
                by_ref: true,
                is_mutable: false,
            }],
            Box::new(RsExpr::Ident("x".into())),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("&x"), "got: {code}");
    }

    #[test]
    fn js_codegen_closure_with_mut_param() {
        let expr = RsExpr::Closure(
            vec![ClosureParam {
                name: "x".into(),
                ty: Type::I64,
                by_ref: false,
                is_mutable: true,
            }],
            Box::new(RsExpr::Ident("x".into())),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("mut x"), "got: {code}");
    }

    #[test]
    fn js_codegen_block_stmts() {
        let expr = RsExpr::Block(vec![
            RsStmt::Let("x".into(), Type::I64, RsExpr::Lit(RsLit::I64(1))),
            RsStmt::Return(Some(RsExpr::Ident("x".into()))),
        ]);
        let code = codegen_expr(&expr);
        assert!(code.contains("{"), "got: {code}");
        assert!(code.contains("let"), "got: {code}");
        assert!(code.contains("return"), "got: {code}");
    }

    #[test]
    fn js_codegen_spread_single() {
        let expr = RsExpr::Spread(vec![RsExpr::Ident("a".into())]);
        let code = codegen_expr(&expr);
        assert_eq!(code, "a");
    }

    #[test]
    fn js_codegen_spread_multiple() {
        let expr = RsExpr::Spread(vec![RsExpr::Ident("a".into()), RsExpr::Ident("b".into())]);
        let code = codegen_expr(&expr);
        assert!(code.contains("a"), "got: {code}");
        assert!(code.contains("b"), "got: {code}");
    }

    #[test]
    fn js_codegen_console_log_multiple_args() {
        let expr = RsExpr::Call(
            Box::new(RsExpr::Ident("console_log".into())),
            vec![
                RsExpr::Lit(RsLit::Str("a: {}".into())),
                RsExpr::Ident("a".into()),
            ],
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("println!"), "got: {code}");
    }

    #[test]
    fn js_codegen_console_log_no_args() {
        let expr = RsExpr::Call(Box::new(RsExpr::Ident("console_log".into())), vec![]);
        let code = codegen_expr(&expr);
        assert!(code.contains("println!()"), "got: {code}");
    }

    #[test]
    fn js_codegen_console_warn() {
        let expr = RsExpr::Call(
            Box::new(RsExpr::Ident("console_warn".into())),
            vec![RsExpr::Lit(RsLit::Str("warning".into()))],
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("eprintln!"), "got: {code}");
        assert!(code.contains("WARN"), "got: {code}");
    }

    #[test]
    fn js_codegen_call_no_args() {
        let expr = RsExpr::Call(Box::new(RsExpr::Ident("do_something".into())), vec![]);
        let code = codegen_expr(&expr);
        assert!(code.contains("doSomething()"), "got: {code}");
    }

    #[test]
    fn js_codegen_type_ascription() {
        let expr = RsExpr::TypeAscription(Box::new(RsExpr::Lit(RsLit::I64(42))), Type::F64);
        let code = codegen_expr(&expr);
        assert!(code.contains("<f64>"), "got: {code}");
    }

    #[test]
    fn js_codegen_field_access() {
        let expr = RsExpr::FieldAccess(Box::new(RsExpr::Ident("obj".into())), "field".into());
        let code = codegen_expr(&expr);
        assert!(code.contains("obj.field"), "got: {code}");
    }

    #[test]
    fn js_codegen_binary_bitand() {
        let expr = RsExpr::Binary(
            BinOp::BitAnd,
            Box::new(RsExpr::Lit(RsLit::I64(0xFF))),
            Box::new(RsExpr::Lit(RsLit::I64(0x0F))),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("&"), "got: {code}");
    }

    #[test]
    fn js_codegen_binary_bitor() {
        let expr = RsExpr::Binary(
            BinOp::BitOr,
            Box::new(RsExpr::Lit(RsLit::I64(0xF0))),
            Box::new(RsExpr::Lit(RsLit::I64(0x0F))),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("|"), "got: {code}");
    }

    #[test]
    fn js_codegen_binary_bitxor() {
        let expr = RsExpr::Binary(
            BinOp::BitXor,
            Box::new(RsExpr::Lit(RsLit::I64(0xFF))),
            Box::new(RsExpr::Lit(RsLit::I64(0x0F))),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("^"), "got: {code}");
    }

    #[test]
    fn js_codegen_binary_shl() {
        let expr = RsExpr::Binary(
            BinOp::Shl,
            Box::new(RsExpr::Lit(RsLit::I64(1))),
            Box::new(RsExpr::Lit(RsLit::I64(4))),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("<<"), "got: {code}");
    }

    #[test]
    fn js_codegen_binary_shr() {
        let expr = RsExpr::Binary(
            BinOp::Shr,
            Box::new(RsExpr::Lit(RsLit::I64(16))),
            Box::new(RsExpr::Lit(RsLit::I64(2))),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains(">>"), "got: {code}");
    }

    #[test]
    fn js_codegen_binary_lte() {
        let expr = RsExpr::Binary(
            BinOp::Lte,
            Box::new(RsExpr::Ident("a".into())),
            Box::new(RsExpr::Ident("b".into())),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("<="), "got: {code}");
    }

    #[test]
    fn js_codegen_binary_gte() {
        let expr = RsExpr::Binary(
            BinOp::Gte,
            Box::new(RsExpr::Ident("a".into())),
            Box::new(RsExpr::Ident("b".into())),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains(">="), "got: {code}");
    }

    #[test]
    fn js_codegen_assign_mul() {
        let expr = RsExpr::Assign(
            AssignOp::MulAssign,
            Box::new(RsExpr::Ident("x".into())),
            Box::new(RsExpr::Lit(RsLit::I64(2))),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("*="), "got: {code}");
    }

    #[test]
    fn js_codegen_assign_div() {
        let expr = RsExpr::Assign(
            AssignOp::DivAssign,
            Box::new(RsExpr::Ident("x".into())),
            Box::new(RsExpr::Lit(RsLit::I64(2))),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("/="), "got: {code}");
    }

    #[test]
    fn js_codegen_assign_mod() {
        let expr = RsExpr::Assign(
            AssignOp::ModAssign,
            Box::new(RsExpr::Ident("x".into())),
            Box::new(RsExpr::Lit(RsLit::I64(3))),
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("%="), "got: {code}");
    }

    #[test]
    fn js_codegen_method_slice_alias() {
        let expr = RsExpr::MethodCall(
            Box::new(RsExpr::Ident("s".into())),
            "substring".into(),
            vec![RsExpr::Lit(RsLit::I64(1)), RsExpr::Lit(RsLit::I64(4))],
        );
        let code = codegen_expr(&expr);
        assert!(code.contains("[1..4]"), "got: {code}");
    }

    #[test]
    fn js_codegen_call_console_log_empty() {
        let expr = RsExpr::Call(Box::new(RsExpr::Ident("console_log".into())), vec![]);
        let code = codegen_expr(&expr);
        assert!(code.contains("println!()"), "got: {code}");
    }
}
