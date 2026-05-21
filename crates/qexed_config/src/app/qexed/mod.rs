use serde::{Deserialize, Serialize};

use crate::tool::AppConfigTrait;
pub mod plugin_download;
pub mod server;
pub mod qexed_args;

#[derive(Debug, Serialize, Deserialize)]
pub struct Qexed {
    pub version: i32,
    // 检查更新
    pub update_check: bool,
    pub language:String,
    pub plugin_download: plugin_download::PluginDownload,
    pub server:server::Server,
    
}
impl Qexed {
    fn get_system_language()->String{
        match sys_locale::get_locale(){
            Some(v)=>v,
            None=>{
                log::warn!("检测当前系统语言失败,使用默认语言 zh-CN,您可以手动修改配置文件 config/qexed.toml中的 language 来设置语言");
                "zh-CN".to_string()
            }
        }
    }
}
impl Default for Qexed {
    fn default() -> Self {
        Self {
            version: Default::default(),
            update_check: true,
            plugin_download: Default::default(),
            server:Default::default(),
            language:Qexed::get_system_language(),
        }
    }
}
impl AppConfigTrait for Qexed {
    const PATH: &'static str = "./config/";

    const NAME: &'static str = "qexed";
}
