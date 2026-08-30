use crate::codegen::CodeGen;
use crate::types::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TranspileOptions;

    fn codegen_function(func: &FunctionDef) -> String {
        let options = TranspileOptions::default();
        let mut codegen = crate::codegen::CodeGen::new(&options);
        generate_function_def(&mut codegen, func);
        codegen.output.clone()
    }

    fn codegen_method(method: &MethodDef) -> String {
        let options = TranspileOptions::default();
        let mut codegen = crate::codegen::CodeGen::new(&options);
        generate_method(&mut codegen, method);
        codegen.output.clone()
    }

    #[test]
    fn js_codegen_function_no_params() {
        let func = FunctionDef {
            name: "greet".into(),
            params: vec![],
            return_type: Type::Void,
            body: vec![RsStmt::Expr(RsExpr::Call(
                Box::new(RsExpr::Ident("console_log".into())),
                vec![RsExpr::Lit(RsLit::Str("hi".into()))],
            ))],
            is_async: false,
            generics: vec![],
        };
        let code = codegen_function(&func);
        assert!(code.contains("fn greet()"), "got: {code}");
    }

    #[test]
    fn js_codegen_function_with_params() {
        let func = FunctionDef {
            name: "add".into(),
            params: vec![
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
            return_type: Type::I64,
            body: vec![RsStmt::Return(Some(RsExpr::Binary(
                BinOp::Add,
                Box::new(RsExpr::Ident("a".into())),
                Box::new(RsExpr::Ident("b".into())),
            )))],
            is_async: false,
            generics: vec![],
        };
        let code = codegen_function(&func);
        assert!(
            code.contains("fn add(a: i64, b: i64) -> i64"),
            "got: {code}"
        );
        assert!(code.contains("return (a + b)"), "got: {code}");
    }

    #[test]
    fn js_codegen_function_async() {
        let func = FunctionDef {
            name: "fetch_data".into(),
            params: vec![],
            return_type: Type::Void,
            body: vec![],
            is_async: true,
            generics: vec![],
        };
        let code = codegen_function(&func);
        assert!(code.contains("async fn fetch_data()"), "got: {code}");
    }

    #[test]
    fn js_codegen_function_return_type() {
        let func = FunctionDef {
            name: "get_value".into(),
            params: vec![],
            return_type: Type::String,
            body: vec![RsStmt::Return(Some(RsExpr::Lit(RsLit::Str("hi".into()))))],
            is_async: false,
            generics: vec![],
        };
        let code = codegen_function(&func);
        assert!(code.contains("-> String"), "got: {code}");
    }

    #[test]
    fn js_codegen_function_void_return() {
        let func = FunctionDef {
            name: "do_nothing".into(),
            params: vec![],
            return_type: Type::Void,
            body: vec![],
            is_async: false,
            generics: vec![],
        };
        let code = codegen_function(&func);
        assert!(
            !code.contains("->"),
            "void should not have return type: {code}"
        );
    }

    #[test]
    fn js_codegen_function_empty_body() {
        let func = FunctionDef {
            name: "empty".into(),
            params: vec![],
            return_type: Type::Void,
            body: vec![],
            is_async: false,
            generics: vec![],
        };
        let code = codegen_function(&func);
        assert!(code.contains("fn empty()"), "got: {code}");
    }

    #[test]
    fn js_codegen_function_multiple_stmts_body() {
        let func = FunctionDef {
            name: "multi".into(),
            params: vec![],
            return_type: Type::I64,
            body: vec![
                RsStmt::Let("a".into(), Type::I64, RsExpr::Lit(RsLit::I64(1))),
                RsStmt::Let("b".into(), Type::I64, RsExpr::Lit(RsLit::I64(2))),
                RsStmt::Return(Some(RsExpr::Binary(
                    BinOp::Add,
                    Box::new(RsExpr::Ident("a".into())),
                    Box::new(RsExpr::Ident("b".into())),
                ))),
            ],
            is_async: false,
            generics: vec![],
        };
        let code = codegen_function(&func);
        assert!(code.contains("let a"), "got: {code}");
        assert!(code.contains("let b"), "got: {code}");
        assert!(code.contains("return"), "got: {code}");
    }

    #[test]
    fn js_codegen_function_bool_param() {
        let func = FunctionDef {
            name: "check".into(),
            params: vec![ParamDef {
                name: "flag".into(),
                ty: Type::Bool,
                default: None,
            }],
            return_type: Type::Bool,
            body: vec![RsStmt::Return(Some(RsExpr::Ident("flag".into())))],
            is_async: false,
            generics: vec![],
        };
        let code = codegen_function(&func);
        assert!(code.contains("flag: bool"), "got: {code}");
    }

    #[test]
    fn js_codegen_function_string_param() {
        let func = FunctionDef {
            name: "greet".into(),
            params: vec![ParamDef {
                name: "name".into(),
                ty: Type::String,
                default: None,
            }],
            return_type: Type::String,
            body: vec![RsStmt::Return(Some(RsExpr::Ident("name".into())))],
            is_async: false,
            generics: vec![],
        };
        let code = codegen_function(&func);
        assert!(code.contains("name: String"), "got: {code}");
    }

    #[test]
    fn js_codegen_method_self_ref() {
        let method = MethodDef {
            name: "get_name".into(),
            params: vec![],
            return_type: Type::String,
            body: vec![RsStmt::Return(Some(RsExpr::Lit(RsLit::Str("test".into()))))],
            is_pub: true,
            is_async: false,
            self_param: Some(SelfParam::SelfRef),
        };
        let code = codegen_method(&method);
        assert!(code.contains("&self"), "got: {code}");
        assert!(code.contains("pub fn get_name"), "got: {code}");
    }

    #[test]
    fn js_codegen_method_self_mut() {
        let method = MethodDef {
            name: "set_name".into(),
            params: vec![ParamDef {
                name: "n".into(),
                ty: Type::String,
                default: None,
            }],
            return_type: Type::Void,
            body: vec![],
            is_pub: true,
            is_async: false,
            self_param: Some(SelfParam::SelfMut),
        };
        let code = codegen_method(&method);
        assert!(code.contains("&mut self"), "got: {code}");
    }

    #[test]
    fn js_codegen_method_no_self() {
        let method = MethodDef {
            name: "new".into(),
            params: vec![],
            return_type: Type::Struct(StructDef {
                name: "Foo".into(),
                fields: vec![],
                impls: vec![],
            }),
            body: vec![],
            is_pub: true,
            is_async: false,
            self_param: Some(SelfParam::None),
        };
        let code = codegen_method(&method);
        assert!(!code.contains("self"), "should not have self: {code}");
    }

    #[test]
    fn js_codegen_method_async() {
        let method = MethodDef {
            name: "fetch".into(),
            params: vec![],
            return_type: Type::Void,
            body: vec![],
            is_pub: true,
            is_async: true,
            self_param: Some(SelfParam::SelfRef),
        };
        let code = codegen_method(&method);
        assert!(code.contains("pub async fn fetch"), "got: {code}");
    }

    #[test]
    fn js_codegen_method_with_params() {
        let method = MethodDef {
            name: "add".into(),
            params: vec![
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
            return_type: Type::I64,
            body: vec![RsStmt::Return(Some(RsExpr::Binary(
                BinOp::Add,
                Box::new(RsExpr::Ident("a".into())),
                Box::new(RsExpr::Ident("b".into())),
            )))],
            is_pub: true,
            is_async: false,
            self_param: Some(SelfParam::SelfRef),
        };
        let code = codegen_method(&method);
        assert!(
            code.contains("pub fn add(&self, a: i64, b: i64) -> i64"),
            "got: {code}"
        );
    }

    #[test]
    fn js_codegen_function_vec_param() {
        let func = FunctionDef {
            name: "sum".into(),
            params: vec![ParamDef {
                name: "nums".into(),
                ty: Type::Vec(Box::new(Type::I64)),
                default: None,
            }],
            return_type: Type::I64,
            body: vec![RsStmt::Return(Some(RsExpr::Lit(RsLit::I64(0))))],
            is_async: false,
            generics: vec![],
        };
        let code = codegen_function(&func);
        assert!(code.contains("Vec<i64>"), "got: {code}");
    }

    #[test]
    fn js_codegen_function_option_param() {
        let func = FunctionDef {
            name: "maybe".into(),
            params: vec![ParamDef {
                name: "x".into(),
                ty: Type::Option(Box::new(Type::I64)),
                default: None,
            }],
            return_type: Type::Void,
            body: vec![],
            is_async: false,
            generics: vec![],
        };
        let code = codegen_function(&func);
        assert!(code.contains("Option<i64>"), "got: {code}");
    }
}

pub fn generate_function_def(codegen: &mut CodeGen, func: &FunctionDef) {
    if func.is_async {
        codegen.write_indent();
        codegen.write("async fn ");
    } else {
        codegen.write_indent();
        codegen.write("fn ");
    }
    codegen.write(&func.name);
    codegen.write("(");
    for (i, param) in func.params.iter().enumerate() {
        if i > 0 {
            codegen.write(", ");
        }
        codegen.write(&format!("{}: {}", param.name, param.ty.to_rust_string()));
    }
    codegen.write(")");
    if !func.return_type.is_void() {
        codegen.write(&format!(" -> {}", func.return_type.to_rust_string()));
    }
    codegen.writeln(" {");
    codegen.indent();
    for stmt in &func.body {
        codegen.generate_stmt(stmt);
    }
    codegen.dedent();
    codegen.writeln("}");
}

pub fn generate_method(codegen: &mut CodeGen, method: &MethodDef) {
    if method.is_async {
        codegen.write_indent();
        codegen.write("pub async fn ");
    } else {
        codegen.write_indent();
        codegen.write("pub fn ");
    }
    codegen.write(&method.name);
    codegen.write("(");
    let mut first_param = true;
    if let Some(self_param) = &method.self_param {
        match self_param {
            SelfParam::SelfRef => {
                codegen.write("&self");
                first_param = false;
            }
            SelfParam::SelfMut => {
                codegen.write("&mut self");
                first_param = false;
            }
            SelfParam::SelfOwned => {
                codegen.write("self");
                first_param = false;
            }
            SelfParam::None => {}
        }
    }
    for param in &method.params {
        if !first_param {
            codegen.write(", ");
        }
        first_param = false;
        codegen.write(&format!("{}: {}", param.name, param.ty.to_rust_string()));
    }
    codegen.write(")");
    if !method.return_type.is_void() {
        codegen.write(&format!(" -> {}", method.return_type.to_rust_string()));
    }
    codegen.writeln(" {");
    codegen.indent();
    for stmt in &method.body {
        codegen.generate_stmt(stmt);
    }
    codegen.dedent();
    codegen.writeln("}");
}
