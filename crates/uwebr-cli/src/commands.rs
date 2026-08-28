use anyhow::Result;

pub fn init_project(name: &str) -> Result<()> {
    println!("Creating uwebr project: {}", name);
    // TODO: Create project structure with Cargo.toml, src/, app.uwebr
    Ok(())
}

pub fn build_project(path: &str) -> Result<()> {
    println!("Building project at: {}", path);
    // TODO: Parse .uwebr files, transpile JS, generate HTML/CSS, build Rust
    Ok(())
}

pub fn dev_server(path: &str) -> Result<()> {
    println!("Starting dev server at: {}", path);
    // TODO: Watch files, hot reload, serve on localhost
    Ok(())
}
