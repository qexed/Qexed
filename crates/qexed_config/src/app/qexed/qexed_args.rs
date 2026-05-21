use clap::{Parser, ValueHint};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "minecraft-server",
    version = "1.0",
    about = "Minecraft Dedicated Server",
    long_about = "官方 Minecraft 专用服务器启动器"
)]
pub struct ServerArgs {
    /// 禁用图形用户界面
    #[arg(long, short = 'n')]
    pub nogui: bool,

    /// 仅初始化配置文件，然后退出
    #[arg(long)]
    pub init_settings: bool,

    /// 以演示模式启动
    #[arg(long)]
    pub demo: bool,

    /// 在新世界生成奖励箱
    #[arg(long)]
    pub bonus_chest: bool,

    /// 强制升级世界数据
    #[arg(long)]
    pub force_upgrade: bool,

    /// 升级时擦除区块缓存
    #[arg(long)]
    pub erase_cache: bool,

    /// 重新创建区域文件
    #[arg(long)]
    pub recreate_region_files: bool,

    /// 安全模式：仅加载原版数据包
    #[arg(long)]
    pub safe_mode: bool,

    /// 显示帮助信息
    #[arg(long, short = 'h')]
    pub help: bool,

    /// 服务器根目录（存放所有世界）
    #[arg(long, default_value = ".", value_hint = ValueHint::DirPath)]
    pub universe: PathBuf,

    /// 世界名称
    #[arg(long, value_name = "NAME")]
    pub world: Option<String>,

    /// 服务器端口
    #[arg(long, default_value = "-1")]
    pub port: i32,

    /// 服务器ID（用于BungeeCord等）
    #[arg(long)]
    pub server_id: Option<String>,

    /// 启用JFR性能分析
    #[arg(long)]
    pub jfr_profile: bool,

    /// 将PID写入指定文件
    #[arg(long, value_hint = ValueHint::FilePath)]
    pub pid_file: Option<PathBuf>,

    /// 剩余的非选项参数（将被忽略，但可用于向后兼容）
    #[arg(trailing_var_arg = true)]
    pub extra_args: Vec<String>,
}
