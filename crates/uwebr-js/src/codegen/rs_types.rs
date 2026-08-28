use crate::codegen::CodeGen;
use crate::types::*;

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
