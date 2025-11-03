use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "agentd")]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the ask daemon
    Ask,
    /// Start the notify daemon
    Notify,
    /// Start the hook daemon
    Hook,
    /// Start the monitor daemon
    Monitor,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Ask => {
            println!("Starting ask daemon...");
            // TODO: Start ask daemon
        }
        Commands::Notify => {
            println!("Starting notify daemon...");
            // TODO: Start notify daemon
        }
        Commands::Hook => {
            println!("Starting hook daemon...");
            // TODO: Start hook daemon
        }
        Commands::Monitor => {
            println!("Starting monitor daemon...");
            // TODO: Start monitor daemon
        }
    }

    Ok(())
}
