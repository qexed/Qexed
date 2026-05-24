use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "qexed_favicon_cli")]
#[command(about = "Convert PNG images to Qexed favicon data URI.")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Convert {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    Apply {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value = "config/qexed.toml")]
        config: PathBuf,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Convert { input, output } => {
            let data_uri = qexed_tools::favicon::convert_to_data_uri(input)?;
            qexed_tools::favicon::write_data_uri(&output, &data_uri)?;
            println!("written {}", output.display());
        }
        Command::Apply { input, config } => {
            let data_uri = qexed_tools::favicon::convert_to_data_uri(input)?;
            qexed_tools::favicon::update_config(&config, &data_uri)?;
            println!("updated {}", config.display());
        }
    }
    Ok(())
}
