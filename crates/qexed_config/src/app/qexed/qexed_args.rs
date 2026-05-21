use clap::{Parser};


#[derive(Parser, Debug)]
#[command(
    name = "Qexed",
    version = "1.0",
    about = "Qexed 服务端",
    long_about = "Qexed 服务端启动器"
)]
pub struct ServerArgs {

    /// 仅初始化配置文件，然后退出
    #[arg(long,default_value = "false")]
    pub init_settings: bool,

    /// 仅向后兼容或插件使用，无其他作用
    #[arg(trailing_var_arg = true)]
    pub extra_args: Vec<String>,
}
