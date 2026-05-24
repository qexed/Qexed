use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "qexed_plugin_manager_cli")]
struct Args {
    #[arg(long, default_value = "plugins")]
    dir: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    List,
    Install { file: PathBuf },
    Enable { name: String },
    Disable { name: String },
    Remove { name: String },
}

fn main() -> Result<()> {
    let args = Args::parse();
    match args.command {
        Command::List => {
            for plugin in qexed_tools::plugins::list(&args.dir)? {
                let state = if plugin.enabled {
                    "enabled"
                } else {
                    "disabled"
                };
                println!("{state}\t{}\t{} bytes", plugin.name, plugin.size);
            }
        }
        Command::Install { file } => {
            let target = qexed_tools::plugins::install(&args.dir, file)?;
            println!("installed {}", target.display());
        }
        Command::Enable { name } => {
            let target = qexed_tools::plugins::enable(&args.dir, &name)?;
            println!("enabled {}", target.display());
        }
        Command::Disable { name } => {
            let target = qexed_tools::plugins::disable(&args.dir, &name)?;
            println!("disabled {}", target.display());
        }
        Command::Remove { name } => {
            qexed_tools::plugins::remove(&args.dir, &name)?;
            println!("removed {name}");
        }
    }
    Ok(())
}
