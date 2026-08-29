use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::abi;

/// Compile profili.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileProfile {
    Debug,
    Release,
}

/// Shared library compile seçenekleri.
#[derive(Debug, Clone)]
pub struct CompileOptions {
    /// Proje root'u (Cargo.toml'un bulunduğu dizin).
    pub root: PathBuf,
    /// .dll/.so çıktısının yazılacağı dizin.
    pub target_dir: PathBuf,
    /// Compile profili.
    pub profile: CompileProfile,
    /// Kalıcı proje dizini (temp yerine reuse). None ise geçici dizin oluşturulur.
    pub project_dir: Option<PathBuf>,
}

/// Compile sonucu.
#[derive(Debug)]
pub struct CompileResult {
    /// Üretilen .dll/.so dosya yolu.
    pub library_path: PathBuf,
    /// Compile süresi (milisaniye).
    pub compile_time_ms: u64,
    /// CSS içeriği (varsa, hot-swap için).
    pub css: Option<String>,
}

/// `.uwebr` content'ini shared library'ye compile eder.
///
/// Geçici bir Cargo projesi oluşturur, transpile edilmiş kodu yazar,
/// `cargo build --lib` ile derler ve çıktıyı `target_dir`'e kopyalar.
///
/// `CompileOptions.project_dir` ayarlıysa mevcut projeyi yeniden kullanır
/// (hızlı incremental build). Aksi halde her seferinde temp dizin oluşturur.
pub fn compile_shared_library(
    uwebr_content: &str,
    component_name: &str,
    options: &CompileOptions,
) -> Result<CompileResult> {
    let start = Instant::now();

    // CSS extraction (raw content'den)
    let css = extract_css(uwebr_content);

    // Proje dizini: reuse veya temp
    let tmp_path;
    let _tmp_dir;
    if let Some(ref proj_dir) = options.project_dir {
        fs::create_dir_all(proj_dir.join("src")).context("failed to create project dir")?;
        tmp_path = proj_dir.clone();
        // İlk seferde Cargo.toml oluştur (yoksa)
        if !tmp_path.join("Cargo.toml").exists() {
            init_lib_project(&tmp_path, component_name, &options.root)?;
        }
        _tmp_dir = None;
    } else {
        let td = tempfile::tempdir().context("failed to create temp dir")?;
        let p = td.path().to_path_buf();
        init_lib_project(&p, component_name, &options.root)?;
        tmp_path = p;
        _tmp_dir = Some(td);
    }

    // Transpile edilmiş kodu yaz
    let lib_rs = generate_lib_rs(uwebr_content, component_name, &options.root)?;
    fs::write(tmp_path.join("src/lib.rs"), &lib_rs).context("failed to write lib.rs")?;

    // Compile et (sccache ile)
    let success = cargo_build_with_sccache(&tmp_path, options)?;

    if !success {
        anyhow::bail!("cargo build failed for component '{component_name}'");
    }

    // Çıktı dosyasını bul
    let lib_name = abi::library_filename(component_name);
    let lib_ext = abi::library_extension();
    let profile_dir = match options.profile {
        CompileProfile::Release => "release",
        CompileProfile::Debug => "debug",
    };
    let built_lib = options
        .target_dir
        .join(profile_dir)
        .join(format!("{lib_name}.{lib_ext}"));

    if !built_lib.exists() {
        // Bazı durumlarda cargo target triple altına yazar
        let alt_path = find_built_lib(&options.target_dir, profile_dir, &lib_name, lib_ext);
        if let Some(alt) = alt_path {
            let dest = abi::library_path(&options.target_dir, component_name);
            fs::copy(&alt, &dest).with_context(|| {
                format!("failed to copy {} → {}", alt.display(), dest.display())
            })?;
        } else {
            anyhow::bail!("compiled library not found at {}", built_lib.display());
        }
    }

    let dest = abi::library_path(&options.target_dir, component_name);
    if built_lib.exists() && built_lib != dest {
        fs::copy(&built_lib, &dest)
            .with_context(|| format!("failed to copy library to {}", dest.display()))?;
    }

    let compile_time_ms = start.elapsed().as_millis() as u64;

    Ok(CompileResult {
        library_path: dest,
        compile_time_ms,
        css,
    })
}

/// `cargo build --lib` çalıştırır. sccache mevcutsa RUSTC_WRAPPER olarak kullanır.
fn cargo_build_with_sccache(project_dir: &Path, options: &CompileOptions) -> Result<bool> {
    let mut cmd = Command::new("cargo");
    cmd.args(["build", "--lib"])
        .arg("--manifest-path")
        .arg(project_dir.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(&options.target_dir);

    match options.profile {
        CompileProfile::Release => {
            cmd.arg("--release");
        }
        CompileProfile::Debug => {}
    }

    // sccache varsa kullan
    if let Ok(output) = Command::new("sccache").arg("--version").output() {
        if output.status.success() {
            cmd.env("RUSTC_WRAPPER", "sccache");
        }
    }

    let status = cmd.status().context("failed to run cargo build")?;
    Ok(status.success())
}

/// Geçici Cargo lib projesi oluşturur.
fn init_lib_project(tmp_path: &Path, component_name: &str, project_root: &Path) -> Result<()> {
    let workspace_root = project_root.parent().unwrap_or(project_root);

    // src/ dizinini oluştur
    fs::create_dir_all(tmp_path.join("src"))?;

    // Cargo.toml'u sıfırdan yaz (cargo init kullanma — duplicate section önlemi)
    // Windows backslash'ları forward slash'a çevir (Cargo.toml escape eder)
    let core_path = workspace_root
        .join("crates/uwebr-core")
        .to_string_lossy()
        .replace('\\', "/");
    let render_path = workspace_root
        .join("crates/uwebr-render")
        .to_string_lossy()
        .replace('\\', "/");

    let manifest = format!(
        r#"[package]
name = "uwebr_dynlib_{component_name}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
uwebr-core = {{ path = "{core_path}" }}
uwebr-render = {{ path = "{render_path}" }}
"#,
    );
    fs::write(tmp_path.join("Cargo.toml"), manifest)?;

    // Boş lib.rs
    fs::write(tmp_path.join("src/lib.rs"), "// placeholder\n")?;

    Ok(())
}

/// `.uwebr` content'inden `<style>` bloğunu extract eder.
fn extract_css(content: &str) -> Option<String> {
    let start_tag = "<style>";
    let end_tag = "</style>";
    let start = content.find(start_tag)?;
    let end = content.find(end_tag)?;
    let css_start = start + start_tag.len();
    if css_start >= end {
        return None;
    }
    let css = &content[css_start..end];
    let trimmed = css.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Shared library'nin `src/lib.rs` içeriğini üretir.
///
/// Transpile edilmiş .uwebr kodunu alır ve `#[no_mangle] pub extern "C"`
/// fonksiyonlarla sarar.
fn generate_lib_rs(
    uwebr_content: &str,
    component_name: &str,
    project_root: &Path,
) -> Result<String> {
    // Transpile
    let transpiled = transpile_uwebr(uwebr_content, component_name, project_root)?;

    // CSS extract
    let css = extract_css(uwebr_content);
    let css_const = if let Some(ref css_text) = css {
        let const_name = format!("CSS_{}", component_name.to_uppercase());
        format!("const {const_name}: &str = r#\"{css_text}\"#;\n\n")
    } else {
        String::new()
    };

    let css_export = if css.is_some() {
        let const_name = format!("CSS_{}", component_name.to_uppercase());
        format!(
            r#"
#[no_mangle]
pub extern "C" fn css() -> *const std::ffi::c_char {{
    {const_name}.as_ptr() as *const std::ffi::c_char
}}
"#
        )
    } else {
        r#"
#[no_mangle]
pub extern "C" fn css() -> *const std::ffi::c_char {
    std::ptr::null()
}
"#
        .to_string()
    };

    let component_fn = format!("{}_component", to_snake(component_name));

    Ok(format!(
        r#"#![allow(unused, non_snake_case)]

use uwebr_core::component::{{Element, NodeType, PropValue}};

{css_const}
{transpiled}

#[no_mangle]
pub extern "C" fn render() -> *mut Element {{
    let elem = {component_fn}(&[]);
    Box::into_raw(Box::new(elem))
}}

#[no_mangle]
pub extern "C" fn cleanup() {{
}}

{css_export}
"#
    ))
}

/// `.uwebr` content'ini Rust koduna transpile eder.
///
/// `uwebr-cli::transpiler::transpile` kullanır — kod tekrarı yok.
fn transpile_uwebr(content: &str, component_name: &str, project_root: &Path) -> Result<String> {
    // Workspace root'unu bul (crates/'in bir üst dizini)
    let _workspace_root = project_root.parent().unwrap_or(project_root);

    // uwebr-cli'yi dynamic olarak load etmeye gerek yok —
    // transpile fonksiyonunu burada yeniden uyguluyoruz.
    // Alternatif: uwebr-cli'yi dependency olarak ekle.
    //
    // Basitlik adına, transpile mantığını burada uyguluyoruz:
    // 1. <style> bloğunu çıkar
    // 2. <script> bloğunu çıkar
    // 3. HTML'i parse et
    // 4. Component fonksiyonu üret

    let _css = extract_css(content);
    let script = extract_tag(content, "script");
    let html = extract_html(content);

    // Basit codegen
    let snake_name = to_snake(component_name);
    let mut output = String::new();

    // Script bindings (basit: let → state accessor)
    if !script.is_empty() {
        output.push_str("// Transpiled from <script> block:\n");
        output.push_str(&transpile_script_simple(&script));
        output.push('\n');
    }

    // Event handler registration
    let handlers = extract_event_handlers(&html);
    output.push_str("use uwebr_core::events::register_action;\n\n");

    // Component function
    let fn_name = format!("{snake_name}_component");
    output.push_str(&format!(
        "pub fn {fn_name}(__props: &[(String, PropValue)]) -> Element {{\n"
    ));

    for handler in &handlers {
        output.push_str(&format!("    register_action(\"{handler}\", {handler});\n"));
    }

    // HTML codegen
    output.push_str(&generate_element_simple(&html, 2));
    output.push_str("\n}\n");

    Ok(output)
}

/// Basit script transpilation — `let x = 0` → `static X: std::sync::atomic::AtomicI32 = ...`
fn transpile_script_simple(script: &str) -> String {
    let mut output = String::new();
    for line in script.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("let ") {
            // let count = 0; → pub static COUNT: AtomicI32 = AtomicI32::new(0);
            if let Some(rest) = trimmed.strip_prefix("let ") {
                let parts: Vec<&str> = rest.splitn(2, '=').collect();
                if parts.len() == 2 {
                    let name = parts[0].trim().to_uppercase();
                    let val = parts[1].trim().trim_end_matches(';').trim();
                    output.push_str(&format!(
                        "use std::sync::atomic::{{AtomicI32, Ordering}};\n\
                         static {name}: AtomicI32 = AtomicI32::new({val});\n\
                         fn get_{name}() -> i32 {{ {name}.load(Ordering::Relaxed) }}\n\
                         fn set_{name}(v: i32) {{ {name}.store(v, Ordering::Relaxed); }}\n\n"
                    ));
                }
            }
        } else if trimmed.starts_with("function ") {
            // function increment() { count++; }
            if let Some(rest) = trimmed.strip_prefix("function ") {
                let fname = rest.split('(').next().unwrap_or("").trim().to_string();
                output.push_str(&format!("fn {fname}() {{ /* handler */ }}\n"));
            }
        }
    }
    output
}

/// HTML content'den event handler isimlerini extract eder.
fn extract_event_handlers(html: &str) -> Vec<String> {
    let mut handlers = Vec::new();
    let mut chars = html.chars().peekable();
    while let Some(c) = chars.next() {
        if c == 'o' {
            // on:click={handler} pattern
            let rest: String = chars.clone().take(10).collect();
            if rest.starts_with("n:click={") {
                // Skip "n:click={"
                for _ in 0..9 {
                    chars.next();
                }
                let mut name = String::new();
                while let Some(&next) = chars.peek() {
                    if next == '}' {
                        chars.next();
                        break;
                    }
                    name.push(next);
                    chars.next();
                }
                if !name.is_empty() {
                    handlers.push(name);
                }
            }
        }
    }
    handlers
}

/// HTML'den basit Element codegen'i üretir.
///
/// `Element { node_type: NodeType::Element("div".into()), props: vec![...], children: vec![...] }`
fn generate_element_simple(html: &str, indent: usize) -> String {
    let pad = " ".repeat(indent);
    let child_pad = " ".repeat(indent + 4);
    let trimmed = html.trim();

    if trimmed.is_empty() {
        return format!("{pad}Element::text(\"\")");
    }

    // Basit HTML tag parse: <div class="app">...</div>
    if let Some(tag_start) = trimmed.find('<') {
        if let Some(tag_end) = trimmed.find('>') {
            let tag_content = &trimmed[tag_start + 1..tag_end];
            let tag_name = tag_content.split_whitespace().next().unwrap_or("div");

            // Props extract
            let mut props_entries = Vec::new();
            if let Some(class_pos) = tag_content.find("class=\"") {
                let class_start = class_pos + 7;
                if let Some(class_end) = tag_content[class_start..].find('"') {
                    let class_val = &tag_content[class_start..class_start + class_end];
                    props_entries.push(format!(
                        "{child_pad}(\"class\".into(), PropValue::String(\"{class_val}\".into()))"
                    ));
                }
            }

            let props_code = if props_entries.is_empty() {
                "vec![]".to_string()
            } else {
                format!("vec![\n{}\n{pad}]", props_entries.join(",\n"))
            };

            // Children
            let after_tag = &trimmed[tag_end + 1..];
            let close_tag = format!("</{tag_name}>");
            let children_code = if let Some(close_pos) = after_tag.find(&close_tag) {
                let children_html = &after_tag[..close_pos].trim();
                if children_html.is_empty() {
                    "vec![]".to_string()
                } else if children_html.contains('<') {
                    let child = generate_element_simple(children_html, indent + 8);
                    format!("vec![\n{child}\n{pad}]")
                } else {
                    format!("vec![Element::text(\"{children_html}\")]")
                }
            } else {
                "vec![]".to_string()
            };

            format!(
                "{pad}Element {{\n\
                 {child_pad}node_type: NodeType::Element(\"{tag_name}\".into()),\n\
                 {child_pad}props: {props_code},\n\
                 {child_pad}children: {children_code},\n\
                 {pad}}}"
            )
        } else {
            format!("{pad}Element::text(\"\")")
        }
    } else {
        format!("{pad}Element::text(\"{trimmed}\")")
    }
}

/// `<tag>` ... `</tag>` içeriğini extract eder.
fn extract_tag(content: &str, tag: &str) -> String {
    let start_tag = format!("<{tag}>");
    let end_tag = format!("</{tag}>");
    let start = match content.find(&start_tag) {
        Some(p) => p + start_tag.len(),
        None => return String::new(),
    };
    let end = match content.find(&end_tag) {
        Some(p) => p,
        None => return String::new(),
    };
    if start >= end {
        return String::new();
    }
    content[start..end].trim().to_string()
}

/// `<style>` ve `<script>` bloklarını çıkarıp saf HTML döndürür.
fn extract_html(content: &str) -> String {
    let mut result = content.to_string();
    // Style bloklarını çıkar
    while let Some(start) = result.find("<style>") {
        let end = match result[start..].find("</style>") {
            Some(p) => start + p + "</style>".len(),
            None => break,
        };
        result.replace_range(start..end, "");
    }
    // Script bloklarını çıkar
    while let Some(start) = result.find("<script>") {
        let end = match result[start..].find("</script>") {
            Some(p) => start + p + "</script>".len(),
            None => break,
        };
        result.replace_range(start..end, "");
    }
    result.trim().to_string()
}

/// snake_case conversion.
pub fn to_snake(s: &str) -> String {
    let mut result = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(ch.to_ascii_lowercase());
    }
    result
}

/// Build edilmiş library dosyasını bulmaya çalışır.
fn find_built_lib(
    target_dir: &Path,
    profile_dir: &str,
    lib_name: &str,
    lib_ext: &str,
) -> Option<PathBuf> {
    // target/<triple>/<profile>/ altına bak
    let profile_path = target_dir.join(profile_dir);
    if profile_path.exists() {
        for entry in fs::read_dir(&profile_path).ok()? {
            let entry = entry.ok()?;
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let candidate = entry.path().join(format!("{lib_name}.{lib_ext}"));
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_css() {
        let content = r#"<div>Hello</div>
<style>.app { color: red; }</style>"#;
        assert_eq!(extract_css(content), Some(".app { color: red; }".into()));
    }

    #[test]
    fn test_extract_css_empty() {
        assert_eq!(extract_css("<div>Hello</div>"), None);
    }

    #[test]
    fn test_extract_tag() {
        let content = "text before <script>let x = 1;</script> text after";
        assert_eq!(extract_tag(content, "script"), "let x = 1;");
    }

    #[test]
    fn test_extract_tag_missing() {
        assert_eq!(extract_tag("no script here", "script"), "");
    }

    #[test]
    fn test_extract_html_removes_style_and_script() {
        let content = r#"<div class="app">
  <style>.app { color: red; }</style>
  <script>let x = 1;</script>
  <p>Hello</p>
</div>"#;
        let html = extract_html(content);
        assert!(!html.contains("<style>"));
        assert!(!html.contains("<script>"));
        assert!(html.contains("<p>Hello</p>"));
    }

    #[test]
    fn test_to_snake() {
        assert_eq!(to_snake("App"), "app");
        assert_eq!(to_snake("MyComponent"), "my_component");
        assert_eq!(to_snake("HTML"), "h_t_m_l");
    }

    #[test]
    fn test_extract_event_handlers() {
        let html = r#"<button on:click={increment}>Click</button>"#;
        let handlers = extract_event_handlers(html);
        assert_eq!(handlers, vec!["increment"]);
    }

    #[test]
    fn test_library_filename() {
        assert_eq!(abi::library_filename("App"), "uwebr_dynlib_App");
    }
}
