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
    /// Build the project
    Build {
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
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { name } => {
            uwebr_cli::commands::init_project(&name)?;
        }
        Commands::Build { path } => {
            uwebr_cli::commands::build_project(&path)?;
        }
        Commands::Dev { path } => {
            uwebr_cli::commands::dev_server(&path)?;
        }
    }

    Ok(())
}
