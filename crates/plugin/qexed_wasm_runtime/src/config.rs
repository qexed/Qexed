use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize,Debug)]
pub struct Plugin {
    pub name: String,

    pub author: Vec<String>,

    pub version: String,

    pub description: String,

    pub documentation: Option<String>,
    #[serde(default)]
    pub permission: Vec<String>, // 申请的权限
    pub edition: u32, // API 版本
    #[serde(default)]
    pub api: Vec<String>, // 对外 API 接口
    #[serde(default)]
    pub depend: HashMap<String,PluginApi>, // 依赖插件
    #[serde(default)]
    pub softdepend: HashMap<String,PluginApi>, // 软依赖
    #[serde(default)]
    pub loadbefore: HashMap<String,PluginApi>, // 列出该插件应优先于哪些插件之前加载
    // 内部元素
    // 插件文件路径
    #[serde(skip)]
    pub path:Option<PathBuf>,
    // 运行阶段
    #[serde(skip,default)]
    pub part:u8,
    // 显示名
    #[serde(skip,default)]
    pub display:String
}
#[derive(Serialize, Deserialize,Debug)]
pub struct PluginApi {
    pub version: String,
    pub api: Vec<String>,
}
