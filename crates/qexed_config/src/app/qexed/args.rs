use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "Qexed",
    version = "1.0",
    about = "Qexed server",
    long_about = "Qexed server launcher"
)]
#[derive(PartialEq, Clone, Default)]
pub struct ServerArgs {
    #[arg(trailing_var_arg = true)]
    pub extra_args: Vec<String>,

    #[arg(long)]
    pub language: Option<String>,

    #[arg(long)]
    pub config_path: Option<std::path::PathBuf>,

    #[arg(long)]
    pub plugin_path: Option<std::path::PathBuf>,
}
