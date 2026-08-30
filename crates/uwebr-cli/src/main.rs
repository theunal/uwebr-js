use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "uwebr", about = "uwebr — Rust-native desktop app framework")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new uwebr project
    Init {
        /// Project name
        name: String,
    },
    /// Transpile .uwebr → Rust and compile to binary
    Build {
        /// Project path
        #[arg(default_value = ".")]
        path: String,
        /// Build in release mode
        #[arg(long)]
        release: bool,
        /// Run the built binary after compilation
        #[arg(long)]
        open: bool,
    },
    /// Validate .uwebr files (parse-only check)
    Check {
        /// Project path
        #[arg(default_value = ".")]
        path: String,
    },
    /// Start dev server with hot reload
    Dev {
        /// Project path
        #[arg(default_value = ".")]
        path: String,
        /// Reload mode: hot-swap (shared lib) or restart (full rebuild)
        #[arg(long, default_value = "hot-swap")]
        mode: String,
    },
    /// Print performance metrics (cold start, layout, binary size)
    Metrics,
    /// Compile .uwebr to shared library (for hot reload)
    Compile {
        /// .uwebr file path
        #[arg(long)]
        input: String,
        /// Output directory for .dll/.so
        #[arg(long, default_value = "target/dynlib")]
        output: String,
    },
    /// Benchmark hot reload: compile + load + render N times
    BenchReload {
        /// .uwebr file path
        #[arg(long)]
        input: String,
        /// Number of iterations (default: 10)
        #[arg(long, default_value_t = 10)]
        iterations: u32,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { name } => {
            uwebr_cli::commands::init_project(&name)?;
        }
        Commands::Build {
            path,
            release,
            open,
        } => {
            uwebr_cli::commands::build_project(&path, release)?;
            if open {
                // Run the built binary
                let bin_path = std::path::Path::new(&path)
                    .join("target")
                    .join(if release { "release" } else { "debug" })
                    .join("app");
                #[cfg(target_os = "windows")]
                let bin_path = bin_path.with_extension("exe");
                if bin_path.exists() {
                    println!("  Running {}...", bin_path.display());
                    std::process::Command::new(&bin_path).spawn().map(|_| ())?;
                } else {
                    eprintln!("  Binary not found at {}", bin_path.display());
                }
            }
        }
        Commands::Check { path } => {
            uwebr_cli::commands::validate_project(&path)?;
        }
        Commands::Dev { path, mode } => {
            uwebr_cli::commands::dev_server_with_mode(&path, &mode)?;
        }
        Commands::Metrics => {
            uwebr_cli::commands::metrics_command();
        }
        Commands::Compile { input, output } => {
            uwebr_cli::commands::compile_library(&input, &output)?;
        }
        Commands::BenchReload { input, iterations } => {
            uwebr_cli::commands::bench_reload(&input, iterations)?;
        }
    }

    Ok(())
}
