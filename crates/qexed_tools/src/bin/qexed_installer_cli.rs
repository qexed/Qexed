use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "qexed_installer_cli")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Pack {
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    Install {
        #[arg(long)]
        archive: PathBuf,
        #[arg(long)]
        target: PathBuf,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Pack { source, output } => {
            qexed_tools::installer::create_package(&source, &output)?;
            println!("created {}", output.display());
        }
        Command::Install { archive, target } => {
            qexed_tools::installer::install(&archive, &target)?;
            println!("installed to {}", target.display());
        }
    }
    Ok(())
}
