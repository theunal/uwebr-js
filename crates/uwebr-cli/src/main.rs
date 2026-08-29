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
    },
    /// Print performance metrics (cold start, layout, binary size)
    Metrics,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { name } => {
            uwebr_cli::commands::init_project(&name)?;
        }
        Commands::Build { path, release } => {
            uwebr_cli::commands::build_project(&path, release)?;
        }
        Commands::Check { path } => {
            uwebr_cli::commands::validate_project(&path)?;
        }
        Commands::Dev { path } => {
            uwebr_cli::commands::dev_server(&path)?;
        }
        Commands::Metrics => {
            uwebr_cli::commands::metrics_command();
        }
    }

    Ok(())
}
