use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;
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

/// Start dev server with hot reload
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

    println!("uwebr dev server running at http://localhost:3000");
    println!("Watching for changes in src/...");
    println!("Press Ctrl+C to stop.");

    // Event loop
    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(event) => {
                let paths: Vec<_> = event.paths.iter()
                    .filter_map(|p| p.strip_prefix(&root).ok())
                    .map(|p| p.display().to_string())
                    .collect();

                let kind = match event.kind {
                    EventKind::Create(_) => "created",
                    EventKind::Modify(_) => "modified",
                    EventKind::Remove(_) => "removed",
                    _ => "changed",
                };

                println!("[rebuild] {kind}: {}", paths.join(", "));

                // Rebuild
                if let Err(e) = build_project(path) {
                    eprintln!("Build error: {e}");
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
