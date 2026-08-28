use crate::codegen::CodeGen;
use crate::types::*;

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
