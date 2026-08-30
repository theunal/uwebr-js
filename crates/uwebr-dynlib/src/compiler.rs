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
/// `uwebr-cli::transpiler::transpile` kullanarak gerçek transpile pipeline'ını çalıştırır.
/// Üretilen kodu `#[no_mangle] extern "C"` wrapper ile sarar.
///
/// `CompileOptions.project_dir` ayarlıysa mevcut projeyi yeniden kullanır
/// (hızlı incremental build). Aksi halde her seferinde temp dizin oluşturur.
///
/// İlk build'de `cargo build --lib` kullanır (dependency'leri derlemek için).
/// Sonraki build'lerde doğrudan `rustc` çağırır (skeleton pre-compiled).
pub fn compile_shared_library(
    uwebr_content: &str,
    component_name: &str,
    options: &CompileOptions,
) -> Result<CompileResult> {
    let start = Instant::now();

    // CSS extraction (raw content'den — CSS_CONST_NAME ile static üretmek için)
    let css = extract_css(uwebr_content);
    let css_const_name = format!("CSS_{}", component_name.to_uppercase());

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

    // Transpile: gerçek pipeline'ı kullan (shared_lib modu)
    let transpiled = uwebr_transpiler::transpile_with_options(
        uwebr_content,
        component_name,
        &uwebr_transpiler::TranspileOptions { shared_lib: true },
    )
    .context("transpile failed")?;

    // Transpile çıktısındaki `pub const CSS_*:` satırlarını kaldır
    // (shared library'de CSS'i static olarak ayrı tanımlıyoruz)
    let cleaned = remove_css_const(&transpiled, &css_const_name);

    // Shared library lib.rs üret
    let lib_rs = generate_lib_rs(&cleaned, &css, &css_const_name, component_name);
    fs::write(tmp_path.join("src/lib.rs"), &lib_rs).context("failed to write lib.rs")?;

    // Skeleton detection: ilk build cargo, sonraki rustc
    let deps_dir = options.target_dir.join("debug").join("deps");
    let skeleton_built = deps_dir.exists();

    let success = if skeleton_built {
        // Hızlı yol: doğrudan rustc (FAZ 20)
        compile_with_rustc(&tmp_path, component_name, options)?
    } else {
        // İlk sefer: cargo build (skeleton)
        cargo_build_with_sccache(&tmp_path, options)?
    };

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

/// Transpile çıktısındaki `pub const CSS_*:` satırlarını kaldırır.
///
/// Gerçek transpiler `pub const CSS_APP: &str = ...;` üretir, ama shared library'de
/// CSS'i static olarak ayrı tanımlayıp `css()` fonksiyonuyla export ediyoruz.
fn remove_css_const(code: &str, const_name: &str) -> String {
    let mut result = String::new();
    for line in code.lines() {
        let trimmed = line.trim();
        // `pub const CSS_APP:` veya `const CSS_APP:` satırlarını atla
        if trimmed.starts_with(&format!("pub const {const_name}:"))
            || trimmed.starts_with(&format!("const {const_name}:"))
        {
            continue;
        }
        result.push_str(line);
        result.push('\n');
    }
    result
}

/// Shared library'nin `src/lib.rs` içeriğini üretir.
///
/// Gerçek transpiler çıktısını alır, `#[no_mangle] extern "C"` wrapper ile sarar.
fn generate_lib_rs(
    transpiled: &str,
    css: &Option<String>,
    css_const_name: &str,
    component_name: &str,
) -> String {
    let snake_name = to_snake(component_name);
    let component_fn = format!("{snake_name}_component");

    // CSS static tanımı
    let css_static = if let Some(ref css_text) = css {
        format!("static {css_const_name}: &str = r#\"{css_text}\"#;\n\n")
    } else {
        String::new()
    };

    // CSS export fonksiyonu
    let css_export = if css.is_some() {
        format!(
            r#"
#[no_mangle]
pub extern "C" fn css() -> *const std::ffi::c_char {{
    {css_const_name}.as_ptr() as *const std::ffi::c_char
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

    format!(
        r#"#![allow(unused, non_snake_case)]

use uwebr_core::component::{{Element, NodeType, PropValue}};

{css_static}{transpiled}

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
    )
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

/// `target/debug/deps/` altındaki `.rlib` dosyalarını toplar.
///
/// Her rlib dosyasından crate ismini çıkarır:
/// `libuwebr_core-abc123.rlib` → `("uwebr_core", path)`
///
/// `--extern` parametreleri için kullanılır.
fn collect_rlibs(target_dir: &Path) -> Result<Vec<(String, PathBuf)>> {
    let deps_dir = target_dir.join("debug").join("deps");
    let mut rlibs = Vec::new();

    if !deps_dir.exists() {
        anyhow::bail!("deps dir not found: {}", deps_dir.display());
    }

    for entry in fs::read_dir(&deps_dir).context("failed to read deps dir")? {
        let entry = entry.context("failed to read deps entry")?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("rlib") {
            // lib<name>-<hash>.rlib → <name>
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            // Skip "lib" prefix
            let without_lib = stem.strip_prefix("lib").unwrap_or(stem);
            // Remove hash suffix (after last '-')
            if let Some(pos) = without_lib.rfind('-') {
                let crate_name = &without_lib[..pos];
                if !crate_name.is_empty() {
                    rlibs.push((crate_name.to_string(), path));
                }
            }
        }
    }

    // Sort by crate name for deterministic output
    rlibs.sort_by(|a, b| a.0.cmp(&b.0));

    Ok(rlibs)
}

/// Doğrudan `rustc` kullanarak shared library compile eder.
///
/// `cargo build` overhead'ini atlar: sadece `rustc --edition 2021 --crate-type cdylib`
/// ile tek dosya compile. Dependency'ler önceden compile edilmiş `.rlib` dosyalarından link edilir.
/// `codegen-units=1`, `strip=symbols`, `debug-assertions=no` ile optimize edilir.
fn compile_with_rustc(
    project_dir: &Path,
    component_name: &str,
    options: &CompileOptions,
) -> Result<bool> {
    let deps_dir = options.target_dir.join("debug").join("deps");
    let lib_rs = project_dir.join("src").join("lib.rs");

    if !lib_rs.exists() {
        anyhow::bail!("lib.rs not found at {}", lib_rs.display());
    }
    if !deps_dir.exists() {
        anyhow::bail!(
            "deps dir not found (run cargo build first): {}",
            deps_dir.display()
        );
    }

    // RLib'leri topla
    let rlibs = collect_rlibs(&options.target_dir)?;

    // Output path
    let lib_name = abi::library_filename(component_name);
    let lib_ext = abi::library_extension();
    let profile_dir = match options.profile {
        CompileProfile::Release => "release",
        CompileProfile::Debug => "debug",
    };
    let output_path = options
        .target_dir
        .join(profile_dir)
        .join(format!("{lib_name}.{lib_ext}"));

    // rustc komutu oluştur — optimize flags
    let mut cmd = Command::new("rustc");
    cmd.arg("--edition")
        .arg("2021")
        .arg("--crate-type")
        .arg("cdylib")
        .arg("--crate-name")
        .arg(format!("uwebr_dynlib_{component_name}"))
        // Hız optimizasyonları
        .arg("-C")
        .arg("codegen-units=1") // Tek codegen unit — daha hızlı link
        .arg("-C")
        .arg("strip=symbols") // Sembol strip — daha küçük çıktı
        .arg("-C")
        .arg("debug-assertions=no")
        .arg("-C")
        .arg("overflow-checks=no")
        .arg("-L")
        .arg(&deps_dir)
        .arg("-o")
        .arg(&output_path)
        .arg(&lib_rs);

    // Her rlib için --extern ekle
    for (name, path) in &rlibs {
        cmd.arg("--extern")
            .arg(format!("{name}={}", path.display()));
    }

    log::debug!(
        "rustc compile: {} rlabs, output={}",
        rlibs.len(),
        output_path.display()
    );

    let status = cmd.status().context("failed to run rustc")?;
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

    let manifest = format!(
        r#"[package]
name = "uwebr_dynlib_{component_name}"
version = "0.1.0"
edition = "2021"

[workspace]

[lib]
crate-type = ["cdylib"]

[dependencies]
uwebr-core = {{ path = "{core_path}" }}
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
    fn test_to_snake() {
        assert_eq!(to_snake("App"), "app");
        assert_eq!(to_snake("MyComponent"), "my_component");
        assert_eq!(to_snake("HTML"), "h_t_m_l");
    }

    #[test]
    fn test_library_filename() {
        assert_eq!(abi::library_filename("App"), "uwebr_dynlib_App");
    }

    #[test]
    fn test_remove_css_const_pub() {
        let code = "pub const CSS_APP: &str = r#\"body { }\"#;\nlet x = 1;\n";
        let result = remove_css_const(code, "CSS_APP");
        assert!(!result.contains("CSS_APP"));
        assert!(result.contains("let x = 1;"));
    }

    #[test]
    fn test_remove_css_const_private() {
        let code = "const CSS_APP: &str = r#\"body { }\"#;\nlet x = 1;\n";
        let result = remove_css_const(code, "CSS_APP");
        assert!(!result.contains("CSS_APP"));
        assert!(result.contains("let x = 1;"));
    }

    #[test]
    fn test_remove_css_const_preserves_other_code() {
        let code = "#![allow(unused)]\nuse uwebr_core::component::Element;\npub const CSS_APP: &str = r#\".app{}\"#;\npub fn app_component() -> Element { todo!() }\n";
        let result = remove_css_const(code, "CSS_APP");
        assert!(result.contains("#![allow(unused)]"));
        assert!(result.contains("use uwebr_core"));
        assert!(result.contains("pub fn app_component"));
        assert!(!result.contains("CSS_APP"));
    }

    #[test]
    fn test_remove_css_const_no_match() {
        let code = "let x = 1;\nlet y = 2;\n";
        let result = remove_css_const(code, "CSS_APP");
        assert_eq!(result, code);
    }
}
