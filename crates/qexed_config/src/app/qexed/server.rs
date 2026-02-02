use serde::{Deserialize, Serialize};
#[derive(Debug, Serialize, Deserialize)]
pub struct Server {
    // 是否启用插件下载功能
    pub ip:String,
    // 是否启用正版验证
    pub online:bool,

}
impl Default for Server {
    fn default() -> Self {
        Self {
            ip:"0.0.0.0:25565".to_owned(),
            online:false,
        }
    }
}
