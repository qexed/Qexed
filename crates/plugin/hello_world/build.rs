use std::{
    env,
    fs::{self, File},
    io::{ Write},
    path::Path,
};

use serde::{Deserialize, Serialize};
#[derive(Serialize,Deserialize)]
pub struct Plugin {
    #[serde(default = "default_name")]
    pub name: String,
    #[serde(default = "default_author")]
    pub author: Vec<String>,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default = "default_description")]
    pub description: String,
    #[serde(default = "default_documentation")]
    pub documentation: Option<String>,
    #[serde(default)]
    pub permission:Vec<String>,// 申请的权限
    pub edition:u32,// API 版本
    #[serde(default)]
    pub api:Vec<String>,// 对外 API 接口
    #[serde(default)]
    pub depend:Vec<PluginApi>,// 依赖插件
    #[serde(default)]
    pub softdepend:Vec<PluginApi>,// 软依赖
    #[serde(default)]
    pub loadbefore:Vec<PluginApi>,// 列出该插件应优先于哪些插件之前加载
}
#[derive(Serialize,Deserialize)]
pub struct PluginApi{
    pub name:String,
    pub version:String,
    pub api:Vec<String>
}
// 我知道这么写很屎山，建议大佬改进()
fn get_cargo_metadata() -> (String, String, Vec<String>, String, Option<String>) {
    let cargo_toml_path = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("Cargo.toml");
    let manifest = cargo_toml::Manifest::from_path(&cargo_toml_path).expect("Failed to read Cargo.toml");
    let package = manifest.package.expect("Cargo.toml 缺少 package 部分");
    
    (
        package.name.clone(),
        package.version().to_string(),
        package.authors().to_vec(),
        match package.description(){
            Some(v) => v.to_string(),
            None => "未定义文档".to_string(),
        },
        match package.documentation(){
            Some(v) => Some(v.to_string()),
            None => None,
        },
    )
}

// 默认值函数（在反序列化时使用）
fn default_name() -> String {
    get_cargo_metadata().0
}

fn default_version() -> String {
    get_cargo_metadata().1
}

fn default_author() -> Vec<String> {
    get_cargo_metadata().2
}

fn default_description() -> String {
    get_cargo_metadata().3
}

fn default_documentation() -> Option<String> {
    get_cargo_metadata().4
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 验证TOML文件格式有效性
    let config_path = Path::new("plugin.toml");
    let content = fs::read_to_string(config_path)?;
    
    // 尝试解析TOML验证格式（不关心具体内容）
    let data:Plugin = toml::from_str(&content)?;

    // 2. 读取原始二进制数据
    let binding = toml::to_string(&data)?;
    let binary_data = binding.as_bytes();

    // 3. 生成Rust代码
    let out_dir = env::var("OUT_DIR")?;
    let dest_path = Path::new(&out_dir).join("generated.rs");
    
    let mut f = File::create(&dest_path)?;
    
    // 生成数组声明
    writeln!(f, "#[unsafe(no_mangle)]")?;
    writeln!(f, "pub static CONFIG_DATA: [u8; {}] = [{}];", 
        binary_data.len(),
        binary_data.iter()
            .map(|b| format!("{}", b))
            .collect::<Vec<_>>()
            .join(", "),
    )?;

    // 生成访问接口
    writeln!(f, "#[unsafe(no_mangle)]")?;
    writeln!(f, "pub extern \"C\" fn get_config_data() -> *const u8 {{ CONFIG_DATA.as_ptr() }}")?;
    writeln!(f, "#[unsafe(no_mangle)]")?;
    writeln!(f, "pub extern \"C\" fn get_config_len() -> usize {{ CONFIG_DATA.len() }}")?;

    // 声明重新生成条件
    println!("cargo:rerun-if-changed=config.toml");
    
    Ok(())
}