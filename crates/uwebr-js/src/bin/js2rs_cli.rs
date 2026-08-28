use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "js2rs")]
#[command(about = "JavaScript to Rust transpiler")]
#[command(version = "0.1.0")]
struct Cli {
    #[arg(short, long)]
    input: PathBuf,

    #[arg(short, long)]
    output: Option<PathBuf>,

    #[arg(short, long, default_value = "false")]
    module: bool,

    #[arg(long)]
    module_name: Option<String>,

    #[arg(long, default_value = "4")]
    indent: usize,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let js_code = std::fs::read_to_string(&cli.input)?;

    let options = uwebr_js::TranspileOptions {
        module_name: cli.module_name.clone(),
        use_serde: true,
        use_tokio: true,
        indent: cli.indent,
    };

    let result = if cli.module {
        let module_name = cli
            .module_name
            .as_deref()
            .unwrap_or("module");
        uwebr_js::transpile_to_module(&js_code, module_name)?
    } else {
        uwebr_js::transpile_with_options(&js_code, &options)?
    };

    if let Some(output_path) = &cli.output {
        std::fs::write(output_path, &result.code)?;
        eprintln!("Transpiled {} -> {}", cli.input.display(), output_path.display());
    } else {
        print!("{}", result.code);
    }

    for warning in &result.warnings {
        eprintln!("Warning: {}", warning);
    }

    Ok(())
}
