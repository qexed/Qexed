use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "qexed_code_of_conduct_cli")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Get {
        #[arg(long, default_value = "config/qexed.toml")]
        config: PathBuf,
        #[arg(long)]
        language: Option<String>,
    },
    Set {
        #[arg(long, default_value = "config/qexed.toml")]
        config: PathBuf,
        #[arg(long)]
        language: Option<String>,
        #[arg(long)]
        text: String,
    },
    SetFile {
        #[arg(long, default_value = "config/qexed.toml")]
        config: PathBuf,
        #[arg(long)]
        language: Option<String>,
        #[arg(long)]
        input: PathBuf,
    },
    Enable {
        #[arg(long, default_value = "config/qexed.toml")]
        config: PathBuf,
    },
    Disable {
        #[arg(long, default_value = "config/qexed.toml")]
        config: PathBuf,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Get { config, language } => {
            let language = content_language(&config, language);
            println!(
                "{}",
                qexed_tools::code_of_conduct::read_language(config, &language)?
            );
        }
        Command::Set {
            config,
            language,
            text,
        } => {
            let language = content_language(&config, language);
            qexed_tools::code_of_conduct::save(&config, &language, &text, true)?;
            println!("updated {} ({language})", config.display());
        }
        Command::SetFile {
            config,
            language,
            input,
        } => {
            let language = content_language(&config, language);
            let text = std::fs::read_to_string(&input)?;
            qexed_tools::code_of_conduct::save(&config, &language, &text, true)?;
            println!("updated {} ({language})", config.display());
        }
        Command::Enable { config } => {
            qexed_tools::code_of_conduct::set_enabled(&config, true)?;
            println!("enabled {}", config.display());
        }
        Command::Disable { config } => {
            qexed_tools::code_of_conduct::set_enabled(&config, false)?;
            println!("disabled {}", config.display());
        }
    }
    Ok(())
}

fn content_language(config: &PathBuf, language: Option<String>) -> String {
    language
        .as_deref()
        .map(qexed_tools::code_of_conduct::normalize_language_file_name)
        .unwrap_or_else(|| qexed_tools::code_of_conduct::default_language_for_config(config))
}
