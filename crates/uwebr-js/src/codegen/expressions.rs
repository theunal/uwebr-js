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
                if name == "vec" && args.len() == 2 {
                    if matches!(&args[0], RsExpr::Lit(RsLit::Null)) {
                        codegen.write("vec![Default::default(); ");
                        generate_expression(codegen, &args[1]);
                        codegen.write("]");
                        return;
                    }
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
                            if let Some(key_expr) = entry.get(0) {
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
                    if let Some(start) = args.get(0) {
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
