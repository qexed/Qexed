use serde::{Deserialize, Serialize};

use crate::tool::AppConfigTrait;
#[derive(Debug, Serialize, Deserialize,Clone)]
pub struct CommandConfig {
    pub version: i32,
    /// 启用tab补全功能(意味着你只能手动help查询帮助了)
    pub tab:bool,
    /// 启用服内stop命令(高风险命令!!!)
    pub game_stop_cmd:bool,
    /// 启用服内ver 命令(查看服务器信息)
    pub game_var_cmd:bool,

}
impl Default for CommandConfig {
    fn default() -> Self {
        Self {
            version: 0,
            tab: true,
            game_stop_cmd: false,
            game_var_cmd: true,

        }
    }
}
impl AppConfigTrait for CommandConfig {
    const PATH: &'static str = "./config/qexed_command/";
    const NAME: &'static str = "config";
}
