use std::fs;
use tempfile::TempDir;
use uwebr_cli::commands;

#[test]
fn test_init_project_creates_structure() {
    let tmp = TempDir::new().unwrap();
    let project_dir = tmp.path().join("myapp");

    commands::init_project(project_dir.to_str().unwrap()).unwrap();

    // Check directory structure
    assert!(project_dir.join("Cargo.toml").exists());
    assert!(project_dir.join("src/main.rs").exists());
    assert!(project_dir.join("src/app").is_dir());
    assert!(project_dir.join("src/components").is_dir());
    assert!(project_dir.join("public").is_dir());
    assert!(project_dir.join("src/app/App.uwebr").exists());
}

#[test]
fn test_init_project_cargo_toml_content() {
    let tmp = TempDir::new().unwrap();
    let project_dir = tmp.path().join("testproj");

    commands::init_project(project_dir.to_str().unwrap()).unwrap();

    let cargo = fs::read_to_string(project_dir.join("Cargo.toml")).unwrap();
    assert!(cargo.contains("name = \"testproj\""));
    assert!(cargo.contains("uwebr-app"));
}

#[test]
fn test_init_project_main_rs_content() {
    let tmp = TempDir::new().unwrap();
    let project_dir = tmp.path().join("myapp");

    commands::init_project(project_dir.to_str().unwrap()).unwrap();

    let main_rs = fs::read_to_string(project_dir.join("src/main.rs")).unwrap();
    assert!(main_rs.contains("App::new"));
    assert!(main_rs.contains("FnComponent"));
}

#[test]
fn test_init_project_uwebr_template() {
    let tmp = TempDir::new().unwrap();
    let project_dir = tmp.path().join("myapp");

    commands::init_project(project_dir.to_str().unwrap()).unwrap();

    let template = fs::read_to_string(project_dir.join("src/app/App.uwebr")).unwrap();
    assert!(template.contains("<div"));
    assert!(template.contains("<script>"));
    assert!(template.contains("<style>"));
}

#[test]
fn test_build_project_no_files() {
    let tmp = TempDir::new().unwrap();

    // No .uwebr files — validate_project handles empty case
    commands::validate_project(tmp.path().to_str().unwrap()).unwrap();
}

#[test]
fn test_build_project_with_valid_uwebr() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir_all(&src).unwrap();

    fs::write(
        src.join("App.uwebr"),
        r#"<div class="app">
  <h1>Hello</h1>
</div>"#,
    ).unwrap();

    // validate_project parses all .uwebr files
    commands::validate_project(tmp.path().to_str().unwrap()).unwrap();
}

#[test]
fn test_find_uwebr_files() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src/app");
    fs::create_dir_all(&src).unwrap();

    fs::write(src.join("App.uwebr"), "<div>test</div>").unwrap();
    fs::write(src.join("Home.uwebr"), "<div>home</div>").unwrap();
    fs::write(src.join("other.rs"), "fn main() {}").unwrap();

    // validate_project will find and parse the .uwebr files
    commands::validate_project(tmp.path().to_str().unwrap()).unwrap();
}

// ── Incremental build tests ──────────────────────────────────

#[test]
fn test_build_cache_new() {
    let tmp = TempDir::new().unwrap();
    let cache = commands::BuildCache::new(tmp.path().to_path_buf());
    assert_eq!(cache.cached_count(), 0);
}

#[test]
fn test_build_cache_full() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src/app");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("Page.uwebr"), "<div>Hello</div>").unwrap();
    fs::write(src.join("Button.uwebr"), "<button>Click</button>").unwrap();

    let mut cache = commands::BuildCache::new(tmp.path().to_path_buf());
    let results = cache.build_all().unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(cache.cached_count(), 2);
}

#[test]
fn test_build_cache_incremental() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src/app");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("Page.uwebr"), "<div>Hello</div>").unwrap();
    fs::write(src.join("Button.uwebr"), "<button>Click</button>").unwrap();

    let mut cache = commands::BuildCache::new(tmp.path().to_path_buf());
    cache.build_all().unwrap();

    // Incremental: change one file
    let changed = vec![tmp.path().join("src/app/Page.uwebr")];
    let results = cache.build_incremental(&changed).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(cache.cached_count(), 2); // still 2 cached
}

#[test]
fn test_build_cache_parse_result() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src/app");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("Page.uwebr"), "<div class=\"page\"><h1>Hello</h1></div>").unwrap();

    let cache = commands::BuildCache::new(tmp.path().to_path_buf());
    let result = cache.parse_file(&tmp.path().join("src/app/Page.uwebr")).unwrap();
    assert!(result.error.is_none());
    assert!(result.html.contains("Hello"));
    assert!(result.parse_time_us > 0);
}

#[test]
fn test_build_cache_get_cached() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src/app");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("Page.uwebr"), "<div>Hello</div>").unwrap();

    let mut cache = commands::BuildCache::new(tmp.path().to_path_buf());
    cache.build_all().unwrap();

    let cached = cache.get_cached(&tmp.path().join("src/app/Page.uwebr"));
    assert!(cached.is_some());
    assert!(cached.unwrap().html.contains("Hello"));
}

#[test]
fn test_build_cache_no_files() {
    let tmp = TempDir::new().unwrap();
    let mut cache = commands::BuildCache::new(tmp.path().to_path_buf());
    let results = cache.build_all().unwrap();
    assert!(results.is_empty());
}

#[test]
fn test_build_cache_incremental_nonexistent() {
    let tmp = TempDir::new().unwrap();
    let mut cache = commands::BuildCache::new(tmp.path().to_path_buf());
    let changed = vec![tmp.path().join("nonexistent.uwebr")];
    let results = cache.build_incremental(&changed).unwrap();
    // File doesn't exist, so nothing to rebuild
    assert!(results.is_empty());
}

#[test]
fn test_build_cache_detects_script_and_style() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src/app");
    fs::create_dir_all(&src).unwrap();
    fs::write(
        src.join("Page.uwebr"),
        "<div>\n<script>let x = 1;</script>\n<style>.a { color: red; }</style>\n</div>",
    ).unwrap();

    let cache = commands::BuildCache::new(tmp.path().to_path_buf());
    let result = cache.parse_file(&tmp.path().join("src/app/Page.uwebr")).unwrap();
    assert!(result.has_script);
    assert!(result.has_style);
}
