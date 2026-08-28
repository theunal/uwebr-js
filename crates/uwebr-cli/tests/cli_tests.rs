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

    // No .uwebr files
    commands::build_project(tmp.path().to_str().unwrap()).unwrap();
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

    commands::build_project(tmp.path().to_str().unwrap()).unwrap();
}

#[test]
fn test_find_uwebr_files() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src/app");
    fs::create_dir_all(&src).unwrap();

    fs::write(src.join("App.uwebr"), "<div>test</div>").unwrap();
    fs::write(src.join("Home.uwebr"), "<div>home</div>").unwrap();
    fs::write(src.join("other.rs"), "fn main() {}").unwrap();

    // We can't call find_uwebr_files directly (private), but build_project will use it
    // Just verify build_project finds the files
    commands::build_project(tmp.path().to_str().unwrap()).unwrap();
}
