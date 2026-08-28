use crate::transpiler;
use anyhow::{Context, Result};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Scaffold a new uwebr project
pub fn init_project(name: &str) -> Result<()> {
    let root = Path::new(name);
    let crate_name = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("my-app");

    // Create directory structure
    fs::create_dir_all(root.join("src/app"))?;
    fs::create_dir_all(root.join("src/components"))?;
    fs::create_dir_all(root.join("public"))?;

    // Cargo.toml
    fs::write(
        root.join("Cargo.toml"),
        format!(
            r#"[package]
name = "{crate_name}"
version = "0.1.0"
edition = "2021"

[dependencies]
uwebr-app = {{ git = "https://github.com/uwebr/uwebr" }}
uwebr-core = {{ git = "https://github.com/uwebr/uwebr" }}
uwebr-macro = {{ git = "https://github.com/uwebr/uwebr" }}
anyhow = "1"
"#
        ),
    )?;

    // src/main.rs — placeholder, overwritten by build/dev
    fs::write(
        root.join("src/main.rs"),
        r#"mod generated;

use uwebr_app::App;
use uwebr_app::FnComponent;
use generated::app::app_component;

pub fn main() -> anyhow::Result<()> {
    App::new("App")
        .with_component(FnComponent::new(|| app_component()))
        .run()
}
"#,
    )?;

    // src/app/App.uwebr (template)
    fs::write(
        root.join("src/app/App.uwebr"),
        r#"<div class="app">
  <h1>Hello from uwebr!</h1>
</div>

<script>
  let count = 0;

  function increment() {
    count++;
  }
</script>

<style>
  .app {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100vh;
    background-color: #1a1a2e;
    color: #e0e0e0;
    font-family: system-ui, sans-serif;
  }

  h1 {
    font-size: 2rem;
    margin-bottom: 1rem;
  }
</style>
"#,
    )?;

    println!("Created uwebr project: {name}");
    println!();
    println!("  cd {name}");
    println!("  cargo run");

    Ok(())
}

/// Result of parsing a single .uwebr file
#[derive(Debug, Clone)]
pub struct ParseResult {
    pub path: PathBuf,
    pub html: String,
    pub has_script: bool,
    pub has_style: bool,
    pub parse_time_us: u128,
    pub error: Option<String>,
}

/// Incremental build cache — only re-parses changed files
pub struct BuildCache {
    results: HashMap<PathBuf, ParseResult>,
    root: PathBuf,
}

impl BuildCache {
    pub fn new(root: PathBuf) -> Self {
        Self {
            results: HashMap::new(),
            root,
        }
    }

    /// Full build: parse all .uwebr files
    pub fn build_all(&mut self) -> Result<Vec<ParseResult>> {
        let files = find_uwebr_files(&self.root)?;
        let mut results = vec![];

        for file in &files {
            let result = self.parse_file(file)?;
            self.results.insert(file.clone(), result.clone());
            results.push(result);
        }

        Ok(results)
    }

    /// Incremental build: only re-parse changed files
    pub fn build_incremental(&mut self, changed_files: &[PathBuf]) -> Result<Vec<ParseResult>> {
        let mut results = vec![];

        for file in changed_files {
            // Only process .uwebr files that exist
            if file.extension().is_some_and(|ext| ext == "uwebr") && file.exists() {
                let result = self.parse_file(file)?;
                self.results.insert(file.clone(), result.clone());
                results.push(result);
            }
        }

        Ok(results)
    }

    /// Parse a single .uwebr file (public for testing)
    pub fn parse_file(&self, file: &Path) -> Result<ParseResult> {
        let start = Instant::now();
        let content = fs::read_to_string(file)
            .with_context(|| format!("Failed to read {}", file.display()))?;

        let has_script = content.contains("<script>");
        let has_style = content.contains("<style>");

        let error = match uwebr_html::parse_html(&content) {
            Ok(_node) => None,
            Err(e) => Some(e.to_string()),
        };

        let parse_time_us = start.elapsed().as_micros();

        Ok(ParseResult {
            path: file.to_path_buf(),
            html: content,
            has_script,
            has_style,
            parse_time_us,
            error,
        })
    }

    /// Get cached result for a file
    pub fn get_cached(&self, path: &Path) -> Option<&ParseResult> {
        self.results.get(path)
    }

    /// Number of cached files
    pub fn cached_count(&self) -> usize {
        self.results.len()
    }
}

/// Validate all .uwebr files (parse-only, no transpile/compile)
pub fn validate_project(path: &str) -> Result<()> {
    let root = Path::new(path);

    let uwebr_files = find_uwebr_files(root)?;

    if uwebr_files.is_empty() {
        println!("No .uwebr files found in {path}");
        return Ok(());
    }

    println!("Validating {} .uwebr file(s)...", uwebr_files.len());

    for file in &uwebr_files {
        let rel = file.strip_prefix(root).unwrap_or(file);
        println!("  Validating: {}", rel.display());

        let content = fs::read_to_string(file)
            .with_context(|| format!("Failed to read {}", file.display()))?;

        match uwebr_html::parse_html(&content) {
            Ok(_node) => {
                println!("    OK");
            }
            Err(e) => {
                println!("    ERROR: {e}");
            }
        }
    }

    println!("Validation complete.");
    Ok(())
}

/// Transpile .uwebr files → .rs and compile with cargo
pub fn build_project(path: &str, release: bool) -> Result<()> {
    let root = Path::new(path);
    let out_dir = root.join("src/generated");

    // Find all .uwebr files
    let uwebr_files = find_uwebr_files(root)?;

    if uwebr_files.is_empty() {
        println!("No .uwebr files found in {path}");
        return Ok(());
    }

    fs::create_dir_all(&out_dir)?;

    println!("Transpiling {} .uwebr file(s)...", uwebr_files.len());

    let mut generated_files = vec![];
    let mut errors = 0;

    for file in &uwebr_files {
        let rel = file.strip_prefix(root).unwrap_or(file);
        let file_name = file
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("Component");

        println!("  Compiling: {}", rel.display());

        let content = fs::read_to_string(file)
            .with_context(|| format!("Failed to read {}", file.display()))?;

        // Transpile .uwebr → Rust
        match transpiler::transpile(&content, file_name) {
            Ok(rs_code) => {
                let out_file = out_dir.join(format!("{}.rs", file_name));
                fs::write(&out_file, &rs_code)?;
                println!(
                    "    → {}",
                    out_file.strip_prefix(root).unwrap_or(&out_file).display()
                );
                generated_files.push((file_name.to_string(), out_file));
            }
            Err(e) => {
                println!("    ERROR: {e}");
                errors += 1;
            }
        }
    }

    if errors > 0 {
        println!("\nBuild failed with {errors} error(s).");
        return Ok(());
    }

    // Write mod.rs + main.rs
    let names: Vec<String> = generated_files.iter().map(|(n, _)| n.clone()).collect();
    write_mod_and_main(root, &names)?;
    println!("  Generated src/main.rs + mod.rs");

    println!(
        "Transpilation complete. {} file(s) generated.",
        generated_files.len()
    );

    // Run cargo build if not check-only mode
    println!("\nCompiling with cargo...");
    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("build");
    if release {
        cmd.arg("--release");
    }
    cmd.current_dir(root);

    let status = cmd.status()?;
    if status.success() {
        println!("Build succeeded.");
    } else {
        println!("Cargo build failed.");
    }

    Ok(())
}

/// Start dev server with incremental hot reload (transpile + cargo build)
pub fn dev_server(path: &str) -> Result<()> {
    let root = PathBuf::from(path);
    let (tx, rx) = mpsc::channel();

    let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| {
        if let Ok(event) = res {
            if matches!(
                event.kind,
                EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
            ) {
                let _ = tx.send(event);
            }
        }
    })?;

    // Watch src/ directory
    watcher.watch(root.join("src").as_path(), RecursiveMode::Recursive)?;

    // Initial full transpile + build
    println!("uwebr dev — transpiling + building...");
    let start = Instant::now();
    match transpile_all(&root) {
        Ok(count) => {
            println!("  Transpiled {count} file(s) in {:?}", start.elapsed());
        }
        Err(e) => {
            eprintln!("  Transpile error: {e}");
        }
    }

    // Run cargo build
    println!("Building...");
    let status = std::process::Command::new("cargo")
        .args(["build"])
        .current_dir(&root)
        .status()?;
    if status.success() {
        println!("  Build succeeded in {:?}", start.elapsed());
    } else {
        println!("  Build failed");
    }

    println!("Watching for changes in src/...");
    println!("Press Ctrl+C to stop.");

    // Event loop with incremental rebuild
    loop {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(event) => {
                // Collect all changed files (debounce: wait a bit for more events)
                let mut changed = event.paths.clone();

                // Drain any additional events within 100ms (debounce)
                while let Ok(more) = rx.recv_timeout(Duration::from_millis(100)) {
                    for p in more.paths {
                        if !changed.contains(&p) {
                            changed.push(p);
                        }
                    }
                }

                // Filter to only .uwebr files
                let uwebr_changed: Vec<_> = changed
                    .iter()
                    .filter(|p| p.extension().is_some_and(|ext| ext == "uwebr"))
                    .cloned()
                    .collect();

                if uwebr_changed.is_empty() {
                    continue;
                }

                let paths_display: Vec<_> = uwebr_changed
                    .iter()
                    .filter_map(|p| p.strip_prefix(&root).ok())
                    .map(|p| p.display().to_string())
                    .collect();

                println!(
                    "[rebuild] {} file(s): {}",
                    uwebr_changed.len(),
                    paths_display.join(", ")
                );

                let start = Instant::now();

                // Transpile changed files
                match transpile_incremental(&root, &uwebr_changed) {
                    Ok(count) => {
                        // Cargo build
                        let build_status = std::process::Command::new("cargo")
                            .args(["build"])
                            .current_dir(&root)
                            .status();

                        match build_status {
                            Ok(s) if s.success() => {
                                println!("  ✓ Transpiled {count} + built in {:?}", start.elapsed());
                            }
                            Ok(_) => {
                                println!(
                                    "  ⚠ Transpiled {count} file(s) but cargo build failed ({:?})",
                                    start.elapsed()
                                );
                            }
                            Err(e) => {
                                eprintln!("  ✗ cargo build error: {e}");
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("  ✗ Transpile error: {e}");
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // No event — keep watching
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                println!("File watcher disconnected.");
                break;
            }
        }
    }

    Ok(())
}

/// Transpile all .uwebr files to .rs
fn transpile_all(root: &Path) -> Result<usize> {
    let files = find_uwebr_files(root)?;
    let out_dir = root.join("src/generated");
    fs::create_dir_all(&out_dir)?;

    let mut count = 0;
    let mut generated = vec![];

    for file in &files {
        let file_name = file
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("Component");
        let content = fs::read_to_string(file)?;

        match transpiler::transpile(&content, file_name) {
            Ok(rs_code) => {
                let out_file = out_dir.join(format!("{}.rs", file_name));
                fs::write(&out_file, &rs_code)?;
                generated.push(file_name.to_string());
                count += 1;
            }
            Err(e) => {
                let rel = file.strip_prefix(root).unwrap_or(file);
                eprintln!("  ERROR in {}: {e}", rel.display());
            }
        }
    }

    // Write mod.rs + main.rs
    write_mod_and_main(root, &generated)?;

    Ok(count)
}

/// Transpile only changed .uwebr files (incremental)
fn transpile_incremental(root: &Path, changed: &[PathBuf]) -> Result<usize> {
    let out_dir = root.join("src/generated");
    fs::create_dir_all(&out_dir)?;

    let mut count = 0;
    let mut generated = vec![];

    // Re-transpile all to keep mod.rs consistent
    let all_files = find_uwebr_files(root)?;

    for file in &all_files {
        let file_name = file
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("Component");

        // Only re-transpile if this file was changed
        if changed.contains(file) {
            let content = fs::read_to_string(file)?;
            match transpiler::transpile(&content, file_name) {
                Ok(rs_code) => {
                    let out_file = out_dir.join(format!("{}.rs", file_name));
                    fs::write(&out_file, &rs_code)?;
                    count += 1;
                }
                Err(e) => {
                    let rel = file.strip_prefix(root).unwrap_or(file);
                    eprintln!("  ERROR in {}: {e}", rel.display());
                }
            }
        }

        generated.push(file_name.to_string());
    }

    // Rewrite mod.rs + main.rs
    write_mod_and_main(root, &generated)?;

    Ok(count)
}

/// Recursively find all .uwebr files
fn find_uwebr_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = vec![];

    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        if entry.path().extension().is_some_and(|ext| ext == "uwebr") {
            files.push(entry.path().to_path_buf());
        }
    }

    files.sort();
    Ok(files)
}

/// Write mod.rs and main.rs to connect generated components
fn write_mod_and_main(root: &Path, generated: &[String]) -> Result<()> {
    let out_dir = root.join("src/generated");

    // Write mod.rs
    let mod_content: String = generated
        .iter()
        .map(|name| format!("pub mod {};", transpiler::to_snake(name)))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(out_dir.join("mod.rs"), mod_content)?;

    // Determine root component: prefer "App", else first file
    let root_name = generated
        .iter()
        .find(|n| n.eq_ignore_ascii_case("App"))
        .or(generated.first())
        .cloned()
        .unwrap_or_default();
    if root_name.is_empty() {
        return Ok(());
    }
    let root_snake = transpiler::to_snake(&root_name);
    let root_upper = root_name.to_uppercase();

    // Check if root component has CSS
    let root_rs = out_dir.join(format!("{}.rs", root_name));
    let root_has_css = fs::read_to_string(&root_rs)
        .map(|c| c.contains("const CSS_"))
        .unwrap_or(false);

    // Generate main.rs
    let main_rs = root.join("src/main.rs");
    let mut main_content = String::new();
    main_content.push_str("mod generated;\n\n");
    main_content.push_str("use uwebr_app::App;\n");
    main_content.push_str("use uwebr_app::FnComponent;\n\n");
    main_content.push_str(&format!(
        "use generated::{root_snake}::{root_snake}_component;\n"
    ));
    if root_has_css {
        main_content.push_str(&format!(
            "use generated::{root_snake}::CSS_{root_upper};\n"
        ));
    }
    main_content.push_str("\npub fn main() -> anyhow::Result<()> {\n");
    main_content.push_str(&format!(
        "    let mut app = App::new(\"{}\");\n",
        root_name
    ));
    if root_has_css {
        main_content.push_str(&format!("    app = app.with_css(CSS_{root_upper});\n"));
    }
    main_content.push_str("    app.with_component(FnComponent::new(|| {\n");
    main_content.push_str(&format!("        {root_snake}_component()\n"));
    main_content.push_str("    }))\n");
    main_content.push_str("    .run()\n");
    main_content.push_str("}\n");
    fs::write(&main_rs, main_content)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_build_cache_new() {
        let tmp = TempDir::new().unwrap();
        let cache = BuildCache::new(tmp.path().to_path_buf());
        assert_eq!(cache.cached_count(), 0);
    }

    #[test]
    fn test_build_cache_full() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src/app");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("Page.uwebr"), r#"<div>Hello</div>"#).unwrap();
        fs::write(src.join("Button.uwebr"), r#"<button>Click</button>"#).unwrap();
        let mut cache = BuildCache::new(tmp.path().to_path_buf());
        let results = cache.build_all().unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(cache.cached_count(), 2);
    }

    #[test]
    fn test_build_cache_incremental() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src/app");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("Page.uwebr"), r#"<div>Hello</div>"#).unwrap();
        fs::write(src.join("Button.uwebr"), r#"<button>Click</button>"#).unwrap();

        let mut cache = BuildCache::new(tmp.path().to_path_buf());
        cache.build_all().unwrap();

        let changed = vec![tmp.path().join("src/app/Page.uwebr")];
        let results = cache.build_incremental(&changed).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(cache.cached_count(), 2);
    }

    #[test]
    fn test_build_cache_parse_result() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src/app");
        fs::create_dir_all(&src).unwrap();
        fs::write(
            src.join("Page.uwebr"),
            r#"<div class="page"><h1>Hello</h1></div>"#,
        )
        .unwrap();
        let cache = BuildCache::new(tmp.path().to_path_buf());
        let result = cache
            .parse_file(&tmp.path().join("src/app/Page.uwebr"))
            .unwrap();
        assert!(result.error.is_none());
        assert!(result.html.contains("Hello"));
        assert!(result.parse_time_us > 0);
    }

    #[test]
    fn test_build_cache_parse_error() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("bad.uwebr"), "<div><unclosed>").unwrap();
        let cache = BuildCache::new(tmp.path().to_path_buf());
        let result = cache.parse_file(&tmp.path().join("bad.uwebr")).unwrap();
        assert!(result.path.ends_with("bad.uwebr"));
    }

    #[test]
    fn test_build_cache_get_cached() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src/app");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("Page.uwebr"), r#"<div>Hello</div>"#).unwrap();
        let mut cache = BuildCache::new(tmp.path().to_path_buf());
        cache.build_all().unwrap();
        let cached = cache.get_cached(&tmp.path().join("src/app/Page.uwebr"));
        assert!(cached.is_some());
        assert!(cached.unwrap().html.contains("Hello"));
    }

    #[test]
    fn test_find_uwebr_files() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src/app");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("App.uwebr"), "<div>test</div>").unwrap();
        fs::write(src.join("Home.uwebr"), "<div>home</div>").unwrap();
        fs::write(src.join("other.rs"), "fn main() {}").unwrap();
        let files = find_uwebr_files(tmp.path()).unwrap();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_find_uwebr_files_empty() {
        let tmp = TempDir::new().unwrap();
        let files = find_uwebr_files(tmp.path()).unwrap();
        assert!(files.is_empty());
    }
}
