use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "Qexed",
    version = "1.0",
    about = "Qexed 服务端",
    long_about = "Qexed 服务端启动器"
)]
#[derive(PartialEq, Clone, Default)]
pub struct ServerArgs {
    #[arg(long, default_value = "false")]
    pub init_settings: bool,

    #[arg(trailing_var_arg = true)]
    pub extra_args: Vec<String>,

    #[arg(long)]
    pub language: Option<String>,
}
