// crates/bin/qexed_block/build.rs
use std::env;
use std::fs;
use std::path::Path;
use std::collections::HashMap;
use heck::ToUpperCamelCase;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct RegistryFile {
    #[serde(rename = "minecraft:block")]
    block_registry: BlockRegistry,
}

#[derive(Debug, Deserialize)]
struct BlockRegistry {
    #[serde(default)]
    default: String,
    entries: HashMap<String, BlockEntry>,
    #[serde(rename = "protocol_id")]
    registry_protocol_id: u32,
}

#[derive(Debug, Deserialize)]
struct BlockEntry {
    #[serde(rename = "protocol_id")]
    id: u32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=../../../assets/registries.json");
    
    // 获取项目根目录
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")?;
    println!("cargo:info=Manifest dir: {}", manifest_dir);
    // 构建 JSON 文件路径 (向上3级到项目根目录)
    let mut json_path = Path::new(&manifest_dir).to_path_buf();
    for _ in 0..3 {
        json_path = json_path.parent().unwrap_or(Path::new("")).to_path_buf();
    }
    json_path = json_path.join("assets").join("registries.json");
    
    println!("cargo:info=Loading registry from: {:?}", json_path);
    
    // 读取和解析 JSON
    let json_content = fs::read_to_string(&json_path)?;
    let registry: RegistryFile = serde_json::from_str(&json_content)?;
    
    // 生成 Rust 代码
    let code = generate_block_code(&registry.block_registry);
    
    // 写入输出目录
    let out_dir = env::var("OUT_DIR")?;
    let dest_path = Path::new(&out_dir).join("block_registry_generated.rs");
    fs::write(dest_path, code)?;
    
    println!("cargo:info=Generated block registry with {} entries", 
             registry.block_registry.entries.len());
    
    Ok(())
}

/// 生成方块注册表代码
fn generate_block_code(registry: &BlockRegistry) -> String {
    let mut blocks = Vec::new();
    
    // 收集方块信息
    for (name, entry) in &registry.entries {
        let enum_variant = name_to_enum_variant(name);
        let display_name = name_to_display_name(name);
        
        blocks.push(BlockInfo {
            id: entry.id,
            name: name.clone(),
            enum_variant,
            display_name,
        });
    }
    
    // 按ID排序
    blocks.sort_by_key(|b| b.id);
    
    // 生成代码
    let mut code = String::new();
    
    // 添加文件头
    code.push_str("// 自动生成的方块注册表\n");
    code.push_str("// 此文件由 build.rs 生成，请勿手动修改\n\n");
    
    // 添加导入
    code.push_str("use std::collections::HashMap;\n");
    code.push_str("use once_cell::sync::Lazy;\n\n");
    
    // 生成方块ID枚举
    code.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]\n");
    code.push_str("#[repr(u32)]\n");
    code.push_str("pub enum BlockId {\n");
    
    for block in &blocks {
        code.push_str(&format!("    /// {}\n", block.display_name));
        code.push_str(&format!("    {} = {},\n", block.enum_variant, block.id));
    }
    
    code.push_str("}\n\n");
    
    // 生成From<u32>实现
    code.push_str("impl TryFrom<u32> for BlockId {\n");
    code.push_str("    type Error = ();\n\n");
    code.push_str("    fn try_from(value: u32) -> Result<Self, Self::Error> {\n");
    code.push_str("        match value {\n");
    
    for block in &blocks {
        code.push_str(&format!("            {} => Ok(BlockId::{}),\n", block.id, block.enum_variant));
    }
    
    code.push_str("            _ => Err(()),\n");
    code.push_str("        }\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");
    
    // 生成From<BlockId>实现
    code.push_str("impl From<BlockId> for u32 {\n");
    code.push_str("    fn from(id: BlockId) -> Self {\n");
    code.push_str("        id as u32\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");
    
    // 生成get_block_name_by_id函数
    code.push_str("/// 通过ID获取方块名称\n");
    code.push_str("pub fn get_block_name_by_id(id: u32) -> Option<&'static str> {\n");
    code.push_str("    match id {\n");
    
    for block in &blocks {
        code.push_str(&format!("        {} => Some(\"{}\"),\n", block.id, block.name));
    }
    
    code.push_str("        _ => None,\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");
    
    // 生成get_block_id_by_name函数
    code.push_str("/// 通过名称获取方块ID\n");
    code.push_str("pub fn get_block_id_by_name(name: &str) -> Option<u32> {\n");
    code.push_str("    match name {\n");
    
    for block in &blocks {
        code.push_str(&format!("        \"{}\" => Some({}),\n", block.name, block.id));
    }
    
    code.push_str("        _ => None,\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");
    
    // 生成静态映射
    code.push_str("/// 方块ID到信息的静态映射\n");
    code.push_str("pub static BLOCK_INFO_BY_ID: Lazy<HashMap<u32, BlockInfo>> = Lazy::new(|| {\n");
    code.push_str("    let mut map = HashMap::new();\n");
    
    for block in &blocks {
        code.push_str(&format!("    map.insert(\n"));
        code.push_str(&format!("        {},\n", block.id));
        code.push_str(&format!("        BlockInfo {{\n"));
        code.push_str(&format!("            id: {},\n", block.id));
        code.push_str(&format!("            name: \"{}\",\n", block.name));
        code.push_str(&format!("            enum_variant: \"{}\",\n", block.enum_variant));
        code.push_str(&format!("            display_name: \"{}\",\n", block.display_name));
        code.push_str(&format!("        }},\n"));
        code.push_str(&format!("    );\n"));
    }
    
    code.push_str("    map\n");
    code.push_str("});\n\n");
    
    // 生成默认方块ID
    let default_block_id = if let Some(default_entry) = registry.entries.get(&registry.default) {
        default_entry.id
    } else {
        0
    };
    
    code.push_str(&format!("/// 默认方块ID (通常是空气)\n"));
    code.push_str(&format!("pub const DEFAULT_BLOCK_ID: u32 = {};\n\n", default_block_id));
    
    // 生成方块注册表协议ID
    code.push_str(&format!("/// 方块注册表的协议ID\n"));
    code.push_str(&format!("pub const BLOCK_REGISTRY_PROTOCOL_ID: u32 = {};\n\n", registry.registry_protocol_id));
    
    // 生成辅助函数
    code.push_str("/// 获取所有方块ID\n");
    code.push_str("pub fn all_block_ids() -> Vec<u32> {\n");
    code.push_str("    vec![\n");
    
    for block in &blocks {
        code.push_str(&format!("        {},\n", block.id));
    }
    
    code.push_str("    ]\n");
    code.push_str("}\n\n");
    
    code.push_str("/// 获取所有方块名称\n");
    code.push_str("pub fn all_block_names() -> Vec<&'static str> {\n");
    code.push_str("    vec![\n");
    
    for block in &blocks {
        code.push_str(&format!("        \"{}\",\n", block.name));
    }
    
    code.push_str("    ]\n");
    code.push_str("}\n\n");
    
    // 生成BlockInfo结构
    code.push_str("/// 方块信息结构\n");
    code.push_str("#[derive(Debug, Clone)]\n");
    code.push_str("pub struct BlockInfo {\n");
    code.push_str("    /// 方块协议ID\n");
    code.push_str("    pub id: u32,\n");
    code.push_str("    /// 方块内部名称（如 \"minecraft:stone\"）\n");
    code.push_str("    pub name: &'static str,\n");
    code.push_str("    /// 枚举变体名（如 \"Stone\"）\n");
    code.push_str("    pub enum_variant: &'static str,\n");
    code.push_str("    /// 显示名称（如 \"Stone\"）\n");
    code.push_str("    pub display_name: &'static str,\n");
    code.push_str("}\n");
    
    code
}

/// 方块信息结构（用于代码生成）
struct BlockInfo {
    id: u32,
    name: String,
    enum_variant: String,
    display_name: String,
}

/// 将方块名称转换为枚举变体名
fn name_to_enum_variant(name: &str) -> String {
    // 移除命名空间前缀
    let name_without_namespace = if let Some(pos) = name.find(':') {
        &name[pos + 1..]
    } else {
        name
    };
    
    // 转换为大驼峰命名
    name_without_namespace.to_upper_camel_case()
}

/// 将方块名称转换为显示名称
fn name_to_display_name(name: &str) -> String {
    // 移除命名空间前缀
    let name_without_namespace = if let Some(pos) = name.find(':') {
        &name[pos + 1..]
    } else {
        name
    };
    
    // 用空格替换下划线，并首字母大写
    let mut result = String::new();
    for (i, part) in name_without_namespace.split('_').enumerate() {
        if i > 0 {
            result.push(' ');
        }
        if let Some(first_char) = part.chars().next() {
            result.push(first_char.to_ascii_uppercase());
            result.push_str(&part[1..]);
        }
    }
    
    result
}