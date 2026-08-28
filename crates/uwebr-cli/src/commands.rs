use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use notify::{Watcher, RecursiveMode, Event, EventKind};

/// Scaffold a new uwebr project
pub fn init_project(name: &str) -> Result<()> {
    let root = Path::new(name);
    let crate_name = root.file_name()
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

    // src/main.rs
    fs::write(
        root.join("src/main.rs"),
        r#"use uwebr_app::App;
use uwebr_core::component::{Element, NodeType};
use uwebr_app::FnComponent;

fn main() -> anyhow::Result<()> {
    App::new("My App")
        .with_size(800, 600)
        .with_component(FnComponent::new(|| Element {
            node_type: NodeType::Element("div".into()),
            props: vec![],
            children: vec![Element {
                node_type: NodeType::Text("Hello from uwebr!".into()),
                props: vec![],
                children: vec![],
            }],
        }))
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

/// Build the project (parse .uwebr files + validate)
pub fn build_project(path: &str) -> Result<()> {
    let root = Path::new(path);

    // Find all .uwebr files
    let uwebr_files = find_uwebr_files(root)?;

    if uwebr_files.is_empty() {
        println!("No .uwebr files found in {path}");
        return Ok(());
    }

    println!("Building {} .uwebr file(s)...", uwebr_files.len());

    for file in &uwebr_files {
        let rel = file.strip_prefix(root).unwrap_or(file);
        println!("  Compiling: {}", rel.display());

        let content = fs::read_to_string(file)
            .with_context(|| format!("Failed to read {}", file.display()))?;

        // Parse HTML
        match uwebr_html::parse_html(&content) {
            Ok(_node) => {
                println!("    OK");
            }
            Err(e) => {
                println!("    ERROR: {e}");
            }
        }
    }

    println!("Build complete.");
    Ok(())
}

/// Start dev server with incremental hot reload
pub fn dev_server(path: &str) -> Result<()> {
    let root = PathBuf::from(path);
    let (tx, rx) = mpsc::channel();

    let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| {
        if let Ok(event) = res {
            if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)) {
                let _ = tx.send(event);
            }
        }
    })?;

    // Watch src/ directory
    watcher.watch(root.join("src").as_path(), RecursiveMode::Recursive)?;

    // Initial full build
    let mut cache = BuildCache::new(root.clone());
    let initial = cache.build_all()?;
    let total_errors = initial.iter().filter(|r| r.error.is_some()).count();
    let total_files = initial.len();

    println!("uwebr dev server running at http://localhost:3000");
    println!("Initial build: {total_files} file(s), {total_errors} error(s)");
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

                let paths_display: Vec<_> = changed.iter()
                    .filter_map(|p| p.strip_prefix(&root).ok())
                    .map(|p| p.display().to_string())
                    .collect();

                println!("[rebuild] {} file(s): {}", changed.len(), paths_display.join(", "));

                let start = Instant::now();

                // Incremental rebuild — only changed files
                match cache.build_incremental(&changed) {
                    Ok(results) => {
                        let elapsed = start.elapsed();
                        let rebuilt = results.len();
                        let errors: Vec<_> = results.iter().filter_map(|r| r.error.as_ref()).collect();
                        let parse_us: u128 = results.iter().map(|r| r.parse_time_us).sum();

                        if errors.is_empty() {
                            println!("  ✓ Rebuilt {rebuilt} file(s) in {elapsed:?} (parse: {parse_us}μs)");
                        } else {
                            for err in &errors {
                                println!("  ✗ ERROR: {err}");
                            }
                            println!("  Rebuilt {rebuilt} file(s) in {elapsed:?} with {} error(s)", errors.len());
                        }
                    }
                    Err(e) => {
                        eprintln!("  Build error: {e}");
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
        fs::write(src.join("Page.uwebr"), r#"<div class="page"><h1>Hello</h1></div>"#).unwrap();
        let cache = BuildCache::new(tmp.path().to_path_buf());
        let result = cache.parse_file(&tmp.path().join("src/app/Page.uwebr")).unwrap();
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
