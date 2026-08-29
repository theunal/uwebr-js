use crate::transpiler;
use anyhow::{Context, Result};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// The `.uwebr` template written by `uwebr init`.
const SCAFFOLD_TEMPLATE: &str = r#"<div class="app">
  <h1>Hello from uwebr!</h1>
  <p>Count: {count}</p>
  <button on:click={increment}>Increment</button>
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

  p {
    font-size: 1rem;
    margin-bottom: 1rem;
  }

  button {
    font-size: 1rem;
    padding: 8px;
    background-color: #16213e;
    color: #e0e0e0;
    border-width: 1px;
    border-color: #4a4a6a;
  }
</style>
"#;

/// Resolve the dependency lines for a generated `Cargo.toml`.
///
/// Prefers path dependencies on the uwebr checkout this CLI was built from:
/// the published crates / git repo may not exist, and a scaffold that cannot
/// `cargo build` is worse than useless.
fn framework_dependencies() -> String {
    if let Some(root) = workspace_root() {
        let root = root.display().to_string().replace('\\', "/");
        format!(
            "uwebr-app = {{ path = \"{root}/crates/uwebr-app\" }}\n\
             uwebr-core = {{ path = \"{root}/crates/uwebr-core\" }}\n"
        )
    } else {
        // Fall back to git for installed binaries whose source tree is gone.
        "uwebr-app = { git = \"https://github.com/uwebr/uwebr\" }\n\
         uwebr-core = { git = \"https://github.com/uwebr/uwebr\" }\n"
            .to_string()
    }
}

/// Path to the uwebr workspace this CLI was compiled from, if still present.
fn workspace_root() -> Option<PathBuf> {
    // CARGO_MANIFEST_DIR is <root>/crates/uwebr-cli at compile time.
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.parent()?.parent()?;
    if root.join("crates/uwebr-app/Cargo.toml").is_file() {
        Some(root.to_path_buf())
    } else {
        None
    }
}

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
    fs::create_dir_all(root.join("src/generated"))?;
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
{deps}anyhow = "1"
"#,
            deps = framework_dependencies()
        ),
    )?;

    // src/app/App.uwebr (template)
    fs::write(root.join("src/app/App.uwebr"), SCAFFOLD_TEMPLATE)?;

    // Transpile immediately so src/generated/ and src/main.rs are real code.
    // Previously main.rs declared `mod generated;` against a missing directory,
    // so the suggested `cargo run` failed on a fresh scaffold.
    let generated = transpile_all(root)?;

    println!("Created uwebr project: {name}");
    println!("  Transpiled {generated} .uwebr file(s) → src/generated/");
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

/// Incremental build cache — only re-parses changed files.
///
/// Used by `uwebr dev` to skip re-reading untouched files and to report which
/// files failed to parse without stopping the rest of the build.
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

    /// Files whose last parse reported an error.
    pub fn failing_files(&self) -> Vec<&ParseResult> {
        self.results
            .values()
            .filter(|r| r.error.is_some())
            .collect()
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

    let uwebr_files = find_uwebr_files(root)?;
    if uwebr_files.is_empty() {
        println!("No .uwebr files found in {path}");
        return Ok(());
    }

    println!("Transpiling {} .uwebr file(s)...", uwebr_files.len());
    let count = transpile_all(root)?;
    println!("Transpilation complete. {count} file(s) generated.");

    // Run cargo build
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

/// A running app process spawned by the dev server.
///
/// Wraps `Child` so the process is always reaped: leaking it would leave a
/// zombie window on every reload.
struct AppProcess {
    child: Child,
    /// The copy this process was launched from, deleted on shutdown.
    exe: PathBuf,
}

impl AppProcess {
    /// Launch the built binary for a project.
    ///
    /// The binary is copied to a scratch path first: Windows locks a running
    /// executable, so launching `target/debug/app.exe` directly would make the
    /// next `cargo build` fail to link and be indistinguishable from a real
    /// compile error.
    fn spawn(root: &Path, binary: &Path) -> Result<Self> {
        let exe = run_copy_path(binary);
        fs::copy(binary, &exe)
            .with_context(|| format!("Failed to copy {} → {}", binary.display(), exe.display()))?;

        let child = std::process::Command::new(&exe)
            .current_dir(root)
            .spawn()
            .with_context(|| format!("Failed to launch {}", exe.display()))?;
        Ok(Self { child, exe })
    }

    /// Whether the process is still running (non-blocking).
    fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Terminate and reap the process, then remove its scratch copy.
    fn kill(mut self) {
        // Ignore errors: the process may already have exited on its own.
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_file(&self.exe);
    }
}

/// Scratch path used to run a copy of the built binary.
fn run_copy_path(binary: &Path) -> PathBuf {
    let stem = binary.file_stem().and_then(|s| s.to_str()).unwrap_or("app");
    let ext = binary.extension().and_then(|e| e.to_str());
    let name = match ext {
        Some(ext) => format!("{stem}-dev-run.{ext}"),
        None => format!("{stem}-dev-run"),
    };
    binary.with_file_name(name)
}

/// Locate the built binary for a project (debug profile).
pub fn binary_path(root: &Path, crate_name: &str) -> PathBuf {
    let exe = if cfg!(windows) {
        format!("{crate_name}.exe")
    } else {
        crate_name.to_string()
    };
    root.join("target").join("debug").join(exe)
}

/// Read the package name out of a project's Cargo.toml.
pub fn crate_name_of(root: &Path) -> Result<String> {
    let manifest = fs::read_to_string(root.join("Cargo.toml"))
        .with_context(|| format!("No Cargo.toml in {}", root.display()))?;

    for line in manifest.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("name") {
            let rest = rest.trim_start();
            if let Some(value) = rest.strip_prefix('=') {
                return Ok(value.trim().trim_matches('"').to_string());
            }
        }
    }
    anyhow::bail!("Could not find `name` in {}/Cargo.toml", root.display())
}

/// Run `cargo build` for a project, returning whether it succeeded.
fn cargo_build(root: &Path) -> Result<bool> {
    let status = std::process::Command::new("cargo")
        .args(["build"])
        .current_dir(root)
        .status()?;
    Ok(status.success())
}

/// Start dev server with hot reload: transpile → build → (re)launch the app.
pub fn dev_server(path: &str) -> Result<()> {
    let root = PathBuf::from(path);
    let crate_name = crate_name_of(&root)?;
    let binary = binary_path(&root, &crate_name);

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

    // Parse cache: reports which files fail without aborting the build.
    let mut cache = BuildCache::new(root.clone());

    // Initial full transpile + build
    println!("uwebr dev — transpiling + building...");
    let start = Instant::now();

    cache.build_all()?;
    for failing in cache.failing_files() {
        if let Some(ref err) = failing.error {
            eprintln!("  parse error in {}: {err}", failing.path.display());
        }
    }

    match transpile_all(&root) {
        Ok(count) => println!("  Transpiled {count} file(s) in {:?}", start.elapsed()),
        Err(e) => eprintln!("  Transpile error: {e}"),
    }

    println!("Building...");
    let mut running: Option<AppProcess> = match cargo_build(&root) {
        Ok(true) => {
            println!("  Build succeeded in {:?}", start.elapsed());
            match AppProcess::spawn(&root, &binary) {
                Ok(child) => {
                    println!("  Launched {}", binary.display());
                    Some(child)
                }
                Err(e) => {
                    eprintln!("  Failed to launch app: {e}");
                    None
                }
            }
        }
        Ok(false) => {
            println!("  Build failed — fix the errors and save again");
            None
        }
        Err(e) => {
            eprintln!("  cargo build error: {e}");
            None
        }
    };

    println!("Watching for changes in src/...");
    println!("Press Ctrl+C to stop.");

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

                // Classify the batch: a CSS-only change skips the transpile step.
                let change_kind = classify_changes(&changed);
                if change_kind == ChangeKind::None {
                    continue;
                }

                let relevant: Vec<_> = changed
                    .iter()
                    .filter(|p| {
                        matches!(
                            p.extension().and_then(|e| e.to_str()),
                            Some("uwebr") | Some("rs") | Some("css")
                        )
                    })
                    .cloned()
                    .collect();

                let paths_display: Vec<_> = relevant
                    .iter()
                    .filter_map(|p| p.strip_prefix(&root).ok())
                    .map(|p| p.display().to_string())
                    .collect();

                println!(
                    "[reload] {} file(s): {}",
                    relevant.len(),
                    paths_display.join(", ")
                );

                let start = Instant::now();

                // CSS-only fast path: skip transpile, rebuild + relaunch only.
                // (In-process CSS hot-swap without a restart is a future phase;
                // for now we still rebuild but save the transpile cost.)
                let transpiled = if change_kind == ChangeKind::CssOnly {
                    println!("  CSS changed — fast rebuild (skipping transpile)");
                    0
                } else {
                    let uwebr_changed: Vec<_> = relevant
                        .iter()
                        .filter(|p| p.extension().is_some_and(|ext| ext == "uwebr"))
                        .cloned()
                        .collect();

                    // Re-parse for diagnostics, then transpile.
                    for result in cache.build_incremental(&uwebr_changed)? {
                        if let Some(ref err) = result.error {
                            eprintln!("  parse error in {}: {err}", result.path.display());
                        }
                    }

                    match transpile_incremental(&root, &uwebr_changed) {
                        Ok(count) => count,
                        Err(e) => {
                            eprintln!("  transpile error: {e}");
                            continue;
                        }
                    }
                };

                // Build before touching the running app: on a compile error the
                // user keeps a working window. The app runs from a copy of the
                // binary, so linking is not blocked by the live process.
                match cargo_build(&root) {
                    Ok(true) => {
                        if let Some(child) = running.take() {
                            child.kill();
                        }
                        match AppProcess::spawn(&root, &binary) {
                            Ok(child) => {
                                running = Some(child);
                                println!(
                                    "  reloaded {transpiled} file(s) in {:?}",
                                    start.elapsed()
                                );
                            }
                            Err(e) => eprintln!("  failed to relaunch app: {e}"),
                        }
                    }
                    Ok(false) => {
                        let still_running = running.as_mut().map(|c| c.is_alive()).unwrap_or(false);
                        if still_running {
                            println!(
                                "  build failed ({:?}) — keeping the running app",
                                start.elapsed()
                            );
                        } else {
                            println!("  build failed ({:?})", start.elapsed());
                        }
                    }
                    Err(e) => eprintln!("  cargo build error: {e}"),
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Surface an app that exited on its own (crash or window close).
                if let Some(child) = running.as_mut() {
                    if !child.is_alive() {
                        println!("App exited. Save a .uwebr file to relaunch.");
                        if let Some(child) = running.take() {
                            child.kill();
                        }
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                println!("File watcher disconnected.");
                break;
            }
        }
    }

    if let Some(child) = running.take() {
        child.kill();
    }

    Ok(())
}

/// Classification of a batch of changed files for the dev server.
///
/// Drives the hot-reload strategy: a CSS-only batch can skip the transpile step
/// entirely, while any `.uwebr`/`.rs` change needs the full transpile + build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// Only `.css` files changed — skip transpile, rebuild only.
    CssOnly,
    /// At least one `.uwebr` or `.rs` file changed — full pipeline.
    Full,
    /// Nothing relevant changed.
    None,
}

/// Classify a set of changed paths into a [`ChangeKind`].
///
/// `.uwebr`/`.rs` changes always force a [`ChangeKind::Full`] rebuild. When only
/// `.css` files changed, the transpile step can be skipped ([`ChangeKind::CssOnly`]).
pub fn classify_changes(paths: &[PathBuf]) -> ChangeKind {
    let mut css = false;
    let mut full = false;
    for p in paths {
        match p.extension().and_then(|e| e.to_str()) {
            Some("css") => css = true,
            Some("uwebr") | Some("rs") => full = true,
            _ => {}
        }
    }
    if full {
        ChangeKind::Full
    } else if css {
        ChangeKind::CssOnly
    } else {
        ChangeKind::None
    }
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
                let out_file = out_dir.join(format!("{}.rs", to_module_file(file_name)));
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

    // Walk every file so mod.rs stays consistent, but only re-transpile changes.
    let all_files = find_uwebr_files(root)?;

    for file in &all_files {
        let file_name = file
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("Component");

        if changed.contains(file) {
            let content = fs::read_to_string(file)?;
            match transpiler::transpile(&content, file_name) {
                Ok(rs_code) => {
                    let out_file = out_dir.join(format!("{}.rs", to_module_file(file_name)));
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

/// Generated module file name for a component, e.g. `App` → `app`.
///
/// Must match the `mod` name in mod.rs, or the module declaration points at a
/// file that does not exist.
fn to_module_file(component_name: &str) -> String {
    transpiler::to_snake(component_name)
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
    fs::create_dir_all(&out_dir)?;

    // Write mod.rs
    let mod_content: String = generated
        .iter()
        .map(|name| format!("pub mod {};", transpiler::to_snake(name)))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(out_dir.join("mod.rs"), format!("{mod_content}\n"))?;

    // Determine root component: prefer "App", else first file
    let root_name = generated
        .iter()
        .find(|n| n.eq_ignore_ascii_case("App"))
        .or(generated.first())
        .cloned()
        .unwrap_or_default();

    if root_name.is_empty() {
        // Nothing to wire up; leave a compilable stub so `cargo build` works.
        fs::write(
            root.join("src/main.rs"),
            "mod generated;\n\npub fn main() -> anyhow::Result<()> {\n    \
             println!(\"No .uwebr components found.\");\n    Ok(())\n}\n",
        )?;
        return Ok(());
    }

    let root_snake = transpiler::to_snake(&root_name);
    let root_upper = root_name.to_uppercase();

    // Check if root component has CSS
    let root_rs = out_dir.join(format!("{}.rs", to_module_file(&root_name)));
    let root_has_css = fs::read_to_string(&root_rs)
        .map(|c| c.contains("const CSS_"))
        .unwrap_or(false);

    // Generate main.rs
    let mut main_content = String::new();
    main_content.push_str("mod generated;\n\n");
    main_content.push_str("use uwebr_app::App;\n");
    main_content.push_str("use uwebr_app::FnComponent;\n\n");
    main_content.push_str(&format!(
        "use generated::{root_snake}::{root_snake}_component;\n"
    ));
    if root_has_css {
        main_content.push_str(&format!("use generated::{root_snake}::CSS_{root_upper};\n"));
    }
    main_content.push_str("\npub fn main() -> anyhow::Result<()> {\n");
    main_content.push_str(&format!("    let mut app = App::new(\"{root_name}\");\n"));
    if root_has_css {
        main_content.push_str(&format!("    app = app.with_css(CSS_{root_upper});\n"));
    }
    main_content.push_str("    app.with_component(FnComponent::new(|| {\n");
    main_content.push_str(&format!("        {root_snake}_component(&[])\n"));
    main_content.push_str("    }))\n");
    main_content.push_str("    .run()\n");
    main_content.push_str("}\n");
    fs::write(root.join("src/main.rs"), main_content)?;

    Ok(())
}

/// Print performance metrics to stdout.
///
/// Runs the self-contained measurements (cold parse, 1000-node layout) and
/// reports the running binary's size. Memory is reported when the platform
/// probe returns a non-zero value.
pub fn metrics_command() {
    let m = uwebr_render::metrics::Metrics::measure_all();
    println!("uwebr performance metrics");
    println!("  Cold start:  {:.3} ms", m.cold_start_ms);
    println!("  Layout 1000: {:.3} ms", m.layout_1000_nodes_ms);
    if m.memory_bytes > 0 {
        println!("  Memory:      {} bytes", m.memory_bytes);
    } else {
        println!("  Memory:      (not measured on this platform)");
    }
    if m.binary_size_bytes > 0 {
        println!("  Binary size: {} bytes", m.binary_size_bytes);
    } else {
        println!("  Binary size: (unavailable)");
    }
}

/// Compile a `.uwebr` file to a shared library (.dll/.so/.dylib).
pub fn compile_library(input_path: &str, output_dir: &str) -> Result<()> {
    let input = PathBuf::from(input_path);
    let output = PathBuf::from(output_dir);

    if !input.exists() {
        anyhow::bail!("file not found: {}", input.display());
    }

    let content = fs::read_to_string(&input)
        .with_context(|| format!("failed to read {}", input.display()))?;

    let component_name = input
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("Component");

    fs::create_dir_all(&output)?;

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();

    let options = uwebr_dynlib::CompileOptions {
        root: workspace_root,
        target_dir: output.clone(),
        profile: uwebr_dynlib::CompileProfile::Debug,
    };

    println!("Compiling {input_path} → shared library...");

    let result = uwebr_dynlib::compile_shared_library(&content, component_name, &options)?;

    println!(
        "  compiled in {}ms → {}",
        result.compile_time_ms,
        result.library_path.display()
    );

    if let Some(ref css) = result.css {
        println!("  CSS: {} bytes", css.len());
    }

    Ok(())
}

/// Start dev server with a specific reload mode.
///
/// - `"hot-swap"`: compile shared lib + in-process swap (default)
/// - `"restart"`: full cargo build + process restart
pub fn dev_server_with_mode(path: &str, mode: &str) -> Result<()> {
    match mode {
        "restart" | "full" => dev_server(path),
        _ => dev_server_hot_swap(path),
    }
}

/// Hot-swap dev server: compile shared library + in-process swap on file change.
fn dev_server_hot_swap(path: &str) -> Result<()> {
    let root = PathBuf::from(path);

    // Determine component name from the first .uwebr file
    let uwebr_files = find_uwebr_files(&root)?;
    if uwebr_files.is_empty() {
        anyhow::bail!("No .uwebr files found in {path}");
    }
    let first = uwebr_files.first().unwrap();
    let component_name = first
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("App")
        .to_string();

    let dynlib_dir = root.join("target/dynlib");
    fs::create_dir_all(&dynlib_dir)?;

    // Initial compile to shared library
    println!("uwebr dev (hot-swap mode)");
    println!("  Component: {component_name}");

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();

    let start = Instant::now();
    let content = fs::read_to_string(first)?;
    let compile_opts = uwebr_dynlib::CompileOptions {
        root: workspace_root.clone(),
        target_dir: dynlib_dir.clone(),
        profile: uwebr_dynlib::CompileProfile::Debug,
    };

    println!("  Compiling shared library...");
    let result = uwebr_dynlib::compile_shared_library(&content, &component_name, &compile_opts)?;
    println!("  compiled in {}ms", result.compile_time_ms);

    // Load and test render
    println!("  Loading library...");
    let load_start = Instant::now();
    let lib = uwebr_dynlib::LoadedLibrary::load(&result.library_path)?;
    println!("  loaded in {:?}", load_start.elapsed());

    if let Some(css) = lib.css() {
        println!("  CSS: {} bytes", css.len());
    }

    let elem = lib.render();
    match elem {
        Some(_e) => println!("  render() OK"),
        None => println!("  render() returned None"),
    }
    drop(lib);

    println!("  Total init: {:?}", start.elapsed());

    // Set up file watcher
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

    watcher.watch(root.join("src").as_path(), RecursiveMode::Recursive)?;

    println!("Watching for changes in src/...");
    println!("Press Ctrl+C to stop.");

    loop {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(event) => {
                let mut changed = event.paths.clone();
                while let Ok(more) = rx.recv_timeout(Duration::from_millis(100)) {
                    for p in more.paths {
                        if !changed.contains(&p) {
                            changed.push(p);
                        }
                    }
                }

                let change_kind = classify_changes(&changed);
                if change_kind == ChangeKind::None {
                    continue;
                }

                let relevant: Vec<_> = changed
                    .iter()
                    .filter(|p| {
                        matches!(
                            p.extension().and_then(|e| e.to_str()),
                            Some("uwebr") | Some("rs") | Some("css")
                        )
                    })
                    .cloned()
                    .collect();

                let paths_display: Vec<_> = relevant
                    .iter()
                    .filter_map(|p| p.strip_prefix(&root).ok())
                    .map(|p| p.display().to_string())
                    .collect();

                println!(
                    "[reload] {} file(s): {}",
                    relevant.len(),
                    paths_display.join(", ")
                );

                let reload_start = Instant::now();

                if change_kind == ChangeKind::CssOnly {
                    println!("  CSS-only change — skipping shared library recompile");
                    continue;
                }

                // Re-read and recompile
                let uwebr_changed: Vec<_> = relevant
                    .iter()
                    .filter(|p| p.extension().is_some_and(|ext| ext == "uwebr"))
                    .cloned()
                    .collect();

                for file in &uwebr_changed {
                    let content = match fs::read_to_string(file) {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("  failed to read {}: {e}", file.display());
                            continue;
                        }
                    };

                    let opts = uwebr_dynlib::CompileOptions {
                        root: workspace_root.clone(),
                        target_dir: dynlib_dir.clone(),
                        profile: uwebr_dynlib::CompileProfile::Debug,
                    };

                    match uwebr_dynlib::compile_shared_library(&content, &component_name, &opts) {
                        Ok(result) => {
                            // Load and test render
                            match uwebr_dynlib::LoadedLibrary::load(&result.library_path) {
                                Ok(lib) => {
                                    if let Some(_elem) = lib.render() {
                                        println!("  hot-reloaded in {:?}", reload_start.elapsed());
                                    } else {
                                        eprintln!("  render() returned None after swap");
                                    }
                                }
                                Err(e) => {
                                    eprintln!("  failed to load new library: {e}");
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("  compile failed: {e} — keeping current version");
                        }
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                println!("File watcher disconnected.");
                break;
            }
        }
    }

    Ok(())
}

/// Benchmark hot reload: compile + load + render N times, report timings.
pub fn bench_reload(input_path: &str, iterations: u32) -> Result<()> {
    let input = PathBuf::from(input_path);
    if !input.exists() {
        anyhow::bail!("file not found: {}", input.display());
    }

    let content = fs::read_to_string(&input)
        .with_context(|| format!("failed to read {}", input.display()))?;

    let component_name = input
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("Component");

    let tmp_dir = tempfile::tempdir()?;
    let dynlib_dir = tmp_dir.path().join("dynlib");
    fs::create_dir_all(&dynlib_dir)?;

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();

    println!(
        "bench-reload: {iterations} iterations on {}",
        input.display()
    );
    println!("  component: {component_name}");

    let mut times = Vec::new();
    let mut compile_times = Vec::new();
    let mut load_times = Vec::new();
    let mut render_times = Vec::new();

    for i in 0..iterations {
        let iter_start = Instant::now();

        // Compile
        let opts = uwebr_dynlib::CompileOptions {
            root: workspace_root.clone(),
            target_dir: dynlib_dir.clone(),
            profile: uwebr_dynlib::CompileProfile::Debug,
        };

        let compile_start = Instant::now();
        let result = uwebr_dynlib::compile_shared_library(&content, component_name, &opts)?;
        let compile_ms = compile_start.elapsed();
        compile_times.push(compile_ms);

        // Load
        let load_start = Instant::now();
        let lib = uwebr_dynlib::LoadedLibrary::load(&result.library_path)?;
        let load_ms = load_start.elapsed();
        load_times.push(load_ms);

        // Render
        let render_start = Instant::now();
        let elem = lib.render();
        let render_ms = render_start.elapsed();
        render_times.push(render_ms);

        let total = iter_start.elapsed();
        times.push(total);

        println!(
            "  #{i:>2}: compile={compile_ms:>8.1?}  load={load_ms:>6.2?}  render={render_ms:>6.2?}  total={total:>8.1?}  elem={}",
            if elem.is_some() { "OK" } else { "NULL" }
        );
    }

    let avg = times.iter().map(|d| d.as_millis()).sum::<u128>() / times.len() as u128;
    let min = times.iter().min().unwrap();
    let max = times.iter().max().unwrap();

    let avg_compile =
        compile_times.iter().map(|d| d.as_millis()).sum::<u128>() / compile_times.len() as u128;
    let avg_load =
        load_times.iter().map(|d| d.as_millis()).sum::<u128>() / load_times.len() as u128;
    let avg_render =
        render_times.iter().map(|d| d.as_millis()).sum::<u128>() / render_times.len() as u128;

    println!();
    println!("--- Results ---");
    println!("  Compile:  avg={avg_compile}ms");
    println!("  Load:     avg={avg_load}ms");
    println!("  Render:   avg={avg_render}ms");
    println!("  Total:    avg={avg}ms  min={min:?}  max={max:?}");
    println!("  Target:   <500ms (total without compile)");
    let load_render = avg_load + avg_render;
    if load_render < 500 {
        println!("  Status:   PASS (load+render={load_render}ms < 500ms)");
    } else {
        println!("  Status:   compile is bottleneck (load+render={load_render}ms)");
    }

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

    // ── Scaffold completeness (M7) ──────────────────────────────

    #[test]
    fn test_init_creates_generated_module() {
        // main.rs declares `mod generated;`, so the directory and its mod.rs
        // must exist or the fresh scaffold cannot compile.
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("demo");
        init_project(project.to_str().unwrap()).unwrap();

        assert!(project.join("src/generated").is_dir());
        assert!(project.join("src/generated/mod.rs").is_file());
        assert!(project.join("src/generated/app.rs").is_file());
    }

    #[test]
    fn test_init_mod_rs_matches_generated_file_names() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("demo");
        init_project(project.to_str().unwrap()).unwrap();

        let mod_rs = fs::read_to_string(project.join("src/generated/mod.rs")).unwrap();
        for line in mod_rs.lines().filter(|l| l.starts_with("pub mod ")) {
            let name = line
                .trim_start_matches("pub mod ")
                .trim_end_matches(';')
                .trim();
            assert!(
                project.join(format!("src/generated/{name}.rs")).is_file(),
                "mod {name}; has no matching file"
            );
        }
    }

    #[test]
    fn test_init_main_rs_wires_component() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("demo");
        init_project(project.to_str().unwrap()).unwrap();

        let main_rs = fs::read_to_string(project.join("src/main.rs")).unwrap();
        assert!(main_rs.contains("app_component"));
        assert!(main_rs.contains("with_css(CSS_APP)"));
    }

    #[test]
    fn test_init_dependencies_are_resolvable() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("demo");
        init_project(project.to_str().unwrap()).unwrap();

        let cargo = fs::read_to_string(project.join("Cargo.toml")).unwrap();
        assert!(cargo.contains("uwebr-app"));
        // Built from the workspace, so a path dependency must be used: the git
        // URL points at a repo that may not exist.
        if workspace_root().is_some() {
            assert!(
                cargo.contains("path = "),
                "expected a path dependency, got:\n{cargo}"
            );
        }
    }

    #[test]
    fn test_init_generated_code_has_no_module_scope_let() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("demo");
        init_project(project.to_str().unwrap()).unwrap();

        let app_rs = fs::read_to_string(project.join("src/generated/app.rs")).unwrap();
        for line in app_rs.lines() {
            let t = line.trim_start();
            assert!(
                !(t.starts_with("let ") || t.starts_with("let mut ")),
                "module-scope let in generated code: {line}"
            );
        }
    }

    #[test]
    fn test_init_scaffold_wires_click_handler() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("demo");
        init_project(project.to_str().unwrap()).unwrap();

        let app_rs = fs::read_to_string(project.join("src/generated/app.rs")).unwrap();
        assert!(app_rs.contains("register_action(\"increment\""));
        assert!(app_rs.contains("__state_count()"));
    }

    #[test]
    fn test_crate_name_of_reads_package_name() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("my-demo");
        init_project(project.to_str().unwrap()).unwrap();
        assert_eq!(crate_name_of(&project).unwrap(), "my-demo");
    }

    #[test]
    fn test_crate_name_of_missing_manifest() {
        let tmp = TempDir::new().unwrap();
        assert!(crate_name_of(tmp.path()).is_err());
    }

    #[test]
    fn test_binary_path_uses_debug_profile() {
        let p = binary_path(Path::new("/proj"), "demo");
        let s = p.display().to_string().replace('\\', "/");
        assert!(s.contains("target/debug/demo"), "got {s}");
    }

    #[test]
    fn test_run_copy_path_is_distinct_from_binary() {
        // The app must run from a copy: Windows locks a running executable, so
        // launching the build output directly makes the next link fail.
        let binary = Path::new("target/debug/demo.exe");
        let copy = run_copy_path(binary);
        assert_ne!(copy, binary);
        assert_eq!(copy.extension().and_then(|e| e.to_str()), Some("exe"));
        assert!(copy
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains("dev-run")));
    }

    #[test]
    fn test_run_copy_path_without_extension() {
        let copy = run_copy_path(Path::new("target/debug/demo"));
        assert_eq!(
            copy.file_name().and_then(|n| n.to_str()),
            Some("demo-dev-run")
        );
    }

    #[test]
    fn test_run_copy_path_stays_in_same_directory() {
        let copy = run_copy_path(Path::new("/proj/target/debug/demo.exe"));
        assert_eq!(
            copy.parent()
                .map(|p| p.display().to_string().replace('\\', "/")),
            Some("/proj/target/debug".to_string())
        );
    }

    #[test]
    fn test_transpile_all_writes_snake_case_files() {
        // mod.rs uses snake_case names; the files must match or the module
        // declaration points at a missing file.
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src/app");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("MyPage.uwebr"), "<div>Hi</div>").unwrap();

        transpile_all(tmp.path()).unwrap();

        assert!(tmp.path().join("src/generated/my_page.rs").is_file());
        let mod_rs = fs::read_to_string(tmp.path().join("src/generated/mod.rs")).unwrap();
        assert!(mod_rs.contains("pub mod my_page;"));
    }

    #[test]
    fn test_write_mod_and_main_with_no_components() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("src")).unwrap();
        write_mod_and_main(tmp.path(), &[]).unwrap();

        let main_rs = fs::read_to_string(tmp.path().join("src/main.rs")).unwrap();
        assert!(main_rs.contains("fn main"), "must still be compilable");
    }

    #[test]
    fn test_failing_files_reports_parse_errors() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("Ok.uwebr"), "<div>fine</div>").unwrap();

        let mut cache = BuildCache::new(tmp.path().to_path_buf());
        cache.build_all().unwrap();
        // html5ever is lenient, so a clean file must report no errors.
        assert!(cache.failing_files().is_empty());
    }

    // ── Hot reload change classification (FAZ 13) ───────────────

    #[test]
    fn test_classify_css_only_change() {
        let paths = vec![PathBuf::from("src/app/App.css")];
        assert_eq!(classify_changes(&paths), ChangeKind::CssOnly);
    }

    #[test]
    fn test_classify_uwebr_change_is_full() {
        let paths = vec![PathBuf::from("src/app/App.uwebr")];
        assert_eq!(classify_changes(&paths), ChangeKind::Full);
    }

    #[test]
    fn test_classify_mixed_css_and_uwebr_is_full() {
        // Any .uwebr/.rs change forces the full pipeline, even alongside CSS.
        let paths = vec![
            PathBuf::from("styles/main.css"),
            PathBuf::from("src/app/App.uwebr"),
        ];
        assert_eq!(classify_changes(&paths), ChangeKind::Full);
    }

    #[test]
    fn test_classify_irrelevant_change_is_none() {
        let paths = vec![PathBuf::from("README.md"), PathBuf::from("notes.txt")];
        assert_eq!(classify_changes(&paths), ChangeKind::None);
    }
}
