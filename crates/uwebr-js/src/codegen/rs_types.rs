use crate::codegen::CodeGen;
use crate::types::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TranspileOptions;

    fn codegen_struct(def: &StructDef) -> String {
        let options = TranspileOptions::default();
        let mut codegen = crate::codegen::CodeGen::new(&options);
        generate_struct(&mut codegen, def);
        codegen.output.clone()
    }

    fn codegen_enum(def: &EnumDef) -> String {
        let options = TranspileOptions::default();
        let mut codegen = crate::codegen::CodeGen::new(&options);
        generate_enum(&mut codegen, def);
        codegen.output.clone()
    }

    fn codegen_impl(impl_def: &ImplDef) -> String {
        let options = TranspileOptions::default();
        let mut codegen = crate::codegen::CodeGen::new(&options);
        generate_impl(&mut codegen, impl_def);
        codegen.output.clone()
    }

    fn codegen_trait(def: &TraitDef) -> String {
        let options = TranspileOptions::default();
        let mut codegen = crate::codegen::CodeGen::new(&options);
        generate_trait(&mut codegen, def);
        codegen.output.clone()
    }

    #[test]
    fn js_codegen_struct_empty() {
        let def = StructDef {
            name: "Empty".into(),
            fields: vec![],
            impls: vec![],
        };
        let code = codegen_struct(&def);
        assert!(code.contains("struct Empty;"), "got: {code}");
    }

    #[test]
    fn js_codegen_struct_with_fields() {
        let def = StructDef {
            name: "Point".into(),
            fields: vec![
                FieldDef {
                    name: "x".into(),
                    ty: Type::F64,
                    is_pub: true,
                },
                FieldDef {
                    name: "y".into(),
                    ty: Type::F64,
                    is_pub: true,
                },
            ],
            impls: vec![],
        };
        let code = codegen_struct(&def);
        assert!(code.contains("struct Point"), "got: {code}");
        assert!(code.contains("pub x: f64"), "got: {code}");
        assert!(code.contains("pub y: f64"), "got: {code}");
    }

    #[test]
    fn js_codegen_struct_string_field() {
        let def = StructDef {
            name: "Person".into(),
            fields: vec![FieldDef {
                name: "name".into(),
                ty: Type::String,
                is_pub: true,
            }],
            impls: vec![],
        };
        let code = codegen_struct(&def);
        assert!(code.contains("pub name: String"), "got: {code}");
    }

    #[test]
    fn js_codegen_struct_bool_field() {
        let def = StructDef {
            name: "Config".into(),
            fields: vec![FieldDef {
                name: "enabled".into(),
                ty: Type::Bool,
                is_pub: true,
            }],
            impls: vec![],
        };
        let code = codegen_struct(&def);
        assert!(code.contains("pub enabled: bool"), "got: {code}");
    }

    #[test]
    fn js_codegen_struct_vec_field() {
        let def = StructDef {
            name: "List".into(),
            fields: vec![FieldDef {
                name: "items".into(),
                ty: Type::Vec(Box::new(Type::String)),
                is_pub: true,
            }],
            impls: vec![],
        };
        let code = codegen_struct(&def);
        assert!(code.contains("pub items: Vec<String>"), "got: {code}");
    }

    #[test]
    fn js_codegen_enum_unit_variants() {
        let def = EnumDef {
            name: "Color".into(),
            variants: vec![
                EnumVariant {
                    name: "Red".into(),
                    fields: vec![],
                },
                EnumVariant {
                    name: "Green".into(),
                    fields: vec![],
                },
                EnumVariant {
                    name: "Blue".into(),
                    fields: vec![],
                },
            ],
        };
        let code = codegen_enum(&def);
        assert!(code.contains("enum Color"), "got: {code}");
        assert!(code.contains("Red,"), "got: {code}");
        assert!(code.contains("Green,"), "got: {code}");
        assert!(code.contains("Blue,"), "got: {code}");
    }

    #[test]
    fn js_codegen_enum_tuple_variants() {
        let def = EnumDef {
            name: "Shape".into(),
            variants: vec![
                EnumVariant {
                    name: "Circle".into(),
                    fields: vec![Type::F64],
                },
                EnumVariant {
                    name: "Rect".into(),
                    fields: vec![Type::F64, Type::F64],
                },
            ],
        };
        let code = codegen_enum(&def);
        assert!(code.contains("Circle(f64),"), "got: {code}");
        assert!(code.contains("Rect(f64, f64),"), "got: {code}");
    }

    #[test]
    fn js_codegen_enum_mixed_variants() {
        let def = EnumDef {
            name: "Token".into(),
            variants: vec![
                EnumVariant {
                    name: "Int".into(),
                    fields: vec![Type::I64],
                },
                EnumVariant {
                    name: "Str".into(),
                    fields: vec![Type::String],
                },
                EnumVariant {
                    name: "None".into(),
                    fields: vec![],
                },
            ],
        };
        let code = codegen_enum(&def);
        assert!(code.contains("Int(i64),"), "got: {code}");
        assert!(code.contains("Str(String),"), "got: {code}");
        assert!(code.contains("None,"), "got: {code}");
    }

    #[test]
    fn js_codegen_impl_basic() {
        let impl_def = ImplDef {
            self_type: Type::Struct(StructDef {
                name: "Point".into(),
                fields: vec![],
                impls: vec![],
            }),
            trait_name: None,
            methods: vec![MethodDef {
                name: "new".into(),
                params: vec![],
                return_type: Type::Struct(StructDef {
                    name: "Point".into(),
                    fields: vec![],
                    impls: vec![],
                }),
                body: vec![],
                is_pub: true,
                is_async: false,
                self_param: Some(SelfParam::None),
            }],
            generics: vec![],
        };
        let code = codegen_impl(&impl_def);
        assert!(code.contains("impl Point"), "got: {code}");
        assert!(code.contains("pub fn new"), "got: {code}");
    }

    #[test]
    fn js_codegen_impl_with_trait() {
        let impl_def = ImplDef {
            self_type: Type::Struct(StructDef {
                name: "MyType".into(),
                fields: vec![],
                impls: vec![],
            }),
            trait_name: Some("Display".into()),
            methods: vec![MethodDef {
                name: "fmt".into(),
                params: vec![],
                return_type: Type::Void,
                body: vec![],
                is_pub: true,
                is_async: false,
                self_param: Some(SelfParam::SelfRef),
            }],
            generics: vec![],
        };
        let code = codegen_impl(&impl_def);
        assert!(code.contains("impl Display for MyType"), "got: {code}");
    }

    #[test]
    fn js_codegen_impl_multiple_methods() {
        let impl_def = ImplDef {
            self_type: Type::Struct(StructDef {
                name: "Calc".into(),
                fields: vec![],
                impls: vec![],
            }),
            trait_name: None,
            methods: vec![
                MethodDef {
                    name: "add".into(),
                    params: vec![],
                    return_type: Type::I64,
                    body: vec![],
                    is_pub: true,
                    is_async: false,
                    self_param: Some(SelfParam::SelfRef),
                },
                MethodDef {
                    name: "sub".into(),
                    params: vec![],
                    return_type: Type::I64,
                    body: vec![],
                    is_pub: true,
                    is_async: false,
                    self_param: Some(SelfParam::SelfRef),
                },
            ],
            generics: vec![],
        };
        let code = codegen_impl(&impl_def);
        assert!(code.contains("pub fn add"), "got: {code}");
        assert!(code.contains("pub fn sub"), "got: {code}");
    }

    #[test]
    fn js_codegen_trait_basic() {
        let def = TraitDef {
            name: "Drawable".into(),
            methods: vec![TraitMethod {
                name: "draw".into(),
                params: vec![],
                return_type: Type::Void,
            }],
        };
        let code = codegen_trait(&def);
        assert!(code.contains("trait Drawable"), "got: {code}");
        assert!(code.contains("fn draw() -> ();"), "got: {code}");
    }

    #[test]
    fn js_codegen_trait_multiple_methods() {
        let def = TraitDef {
            name: "MathOps".into(),
            methods: vec![
                TraitMethod {
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
                },
                TraitMethod {
                    name: "multiply".into(),
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
                },
            ],
        };
        let code = codegen_trait(&def);
        assert!(
            code.contains("fn add(a: i64, b: i64) -> i64;"),
            "got: {code}"
        );
        assert!(
            code.contains("fn multiply(a: i64, b: i64) -> i64;"),
            "got: {code}"
        );
    }

    #[test]
    fn js_codegen_trait_bool_return() {
        let def = TraitDef {
            name: "Validator".into(),
            methods: vec![TraitMethod {
                name: "is_valid".into(),
                params: vec![],
                return_type: Type::Bool,
            }],
        };
        let code = codegen_trait(&def);
        assert!(code.contains("fn is_valid() -> bool;"), "got: {code}");
    }

    #[test]
    fn js_codegen_trait_string_return() {
        let def = TraitDef {
            name: "Describable".into(),
            methods: vec![TraitMethod {
                name: "describe".into(),
                params: vec![],
                return_type: Type::String,
            }],
        };
        let code = codegen_trait(&def);
        assert!(code.contains("fn describe() -> String;"), "got: {code}");
    }
}

pub fn generate_struct(codegen: &mut CodeGen, def: &StructDef) {
    if def.fields.is_empty() {
        codegen.writeln(&format!("struct {};", def.name));
    } else {
        codegen.writeln(&format!("struct {} {{", def.name));
        codegen.indent();
        for field in &def.fields {
            codegen.writeln(&format!(
                "pub {}: {},",
                field.name,
                field.ty.to_rust_string()
            ));
        }
        codegen.dedent();
        codegen.writeln("}");
    }
}

pub fn generate_enum(codegen: &mut CodeGen, def: &EnumDef) {
    codegen.writeln(&format!("enum {} {{", def.name));
    codegen.indent();
    for variant in &def.variants {
        if variant.fields.is_empty() {
            codegen.writeln(&format!("{},", variant.name));
        } else {
            let fields: Vec<String> = variant.fields.iter().map(|f| f.to_rust_string()).collect();
            codegen.writeln(&format!("{}({}),", variant.name, fields.join(", ")));
        }
    }
    codegen.dedent();
    codegen.writeln("}");
}

pub fn generate_impl(codegen: &mut CodeGen, impl_def: &ImplDef) {
    let type_str = impl_def.self_type.to_rust_string();
    if let Some(trait_name) = &impl_def.trait_name {
        codegen.writeln(&format!("impl {} for {} {{", trait_name, type_str));
    } else {
        codegen.writeln(&format!("impl {} {{", type_str));
    }
    codegen.indent();
    for method in &impl_def.methods {
        super::functions::generate_method(codegen, method);
        codegen.output.push('\n');
    }
    codegen.dedent();
    codegen.writeln("}");
}

pub fn generate_trait(codegen: &mut CodeGen, trait_def: &TraitDef) {
    codegen.writeln(&format!("trait {} {{", trait_def.name));
    codegen.indent();
    for method in &trait_def.methods {
        let params: Vec<String> = method
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, p.ty.to_rust_string()))
            .collect();
        codegen.writeln(&format!(
            "fn {}({}) -> {};",
            method.name,
            params.join(", "),
            method.return_type.to_rust_string()
        ));
    }
    codegen.dedent();
    codegen.writeln("}");
}
