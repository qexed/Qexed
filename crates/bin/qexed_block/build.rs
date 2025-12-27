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
// 新增：方块状态数据结构
#[derive(Debug, Deserialize)]
struct BlocksFile {
    #[serde(flatten)]
    blocks: HashMap<String, BlockDefinition>,
}

#[derive(Debug, Deserialize)]
struct BlockDefinition {
    states: Vec<BlockState>,
    #[serde(default)]
    properties: HashMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct BlockState {
    id: u32,
    #[serde(default)]
    properties: HashMap<String, String>,
    #[serde(default)]
    default: bool,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=../../../assets/registries.json");
    println!("cargo:rerun-if-changed=../../../assets/reports/blocks.json");
    
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")?;
    println!("cargo:info=Manifest dir: {}", manifest_dir);
    
    // 构建项目根目录路径
    let mut project_root = Path::new(&manifest_dir).to_path_buf();
    for _ in 0..3 {
        project_root = project_root.parent().unwrap_or(Path::new("")).to_path_buf();
    }
    
    // 读取方块注册表
    let registry_path = project_root.join("assets").join("registries.json");
    println!("cargo:info=Loading registry from: {:?}", registry_path);
    let json_content = fs::read_to_string(&registry_path)?;
    let registry: RegistryFile = serde_json::from_str(&json_content)?;
    
    // 读取方块状态数据
    let blocks_path = project_root.join("assets").join("reports").join("blocks.json");
    println!("cargo:info=Loading blocks from: {:?}", blocks_path);
    let blocks_content = fs::read_to_string(&blocks_path)?;
    let blocks_data: HashMap<String, BlockDefinition> = serde_json::from_str(&blocks_content)?;
    
    // 生成 Rust 代码
    let code = generate_block_code(&registry.block_registry, &blocks_data);
    
    let out_dir = env::var("OUT_DIR")?;
    let dest_path = Path::new(&out_dir).join("block_registry_generated.rs");
    fs::write(dest_path, code)?;
    
    println!("cargo:info=Generated block registry with {} entries", 
             registry.block_registry.entries.len());
    println!("cargo:info=Processed {} block definitions", blocks_data.len());
    
    Ok(())
}

/// 生成方块注册表代码
fn generate_block_code(registry: &BlockRegistry, blocks_data: &HashMap<String, BlockDefinition>) -> String {
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

  // 新增：生成方块状态结构体和映射
    code.push_str("/// 方块状态信息\n");
    code.push_str("#[derive(Debug, Clone, PartialEq, Eq)]\n");
    code.push_str("pub struct BlockStateInfo {\n");
    code.push_str("    /// 方块状态ID\n");
    code.push_str("    pub id: u32,\n");
    code.push_str("    /// 方块名称\n");
    code.push_str("    pub block_name: &'static str,\n");
    code.push_str("    /// 状态属性\n");
    code.push_str("    pub properties: HashMap<&'static str, &'static str>,\n");
    code.push_str("    /// 是否为默认状态\n");
    code.push_str("    pub is_default: bool,\n");
    code.push_str("}\n\n");
    
    code.push_str("/// 方块状态ID到状态信息的映射\n");
    code.push_str("pub static BLOCK_STATE_BY_ID: Lazy<HashMap<u32, BlockStateInfo>> = Lazy::new(|| {\n");
    code.push_str("    let mut map = HashMap::new();\n");
    
    // 生成方块状态映射
    for (block_name, block_def) in blocks_data {
        for state in &block_def.states {
            code.push_str(&format!("    map.insert(\n"));
            code.push_str(&format!("        {},\n", state.id));
            code.push_str(&format!("        BlockStateInfo {{\n"));
            code.push_str(&format!("            id: {},\n", state.id));
            code.push_str(&format!("            block_name: \"{}\",\n", block_name));
            code.push_str(&format!("            properties: [\n"));
            
            for (prop_name, prop_value) in &state.properties {
                code.push_str(&format!("                (\"{}\", \"{}\"),\n", prop_name, prop_value));
            }
            
            code.push_str(&format!("            ].iter().cloned().collect(),\n"));
            code.push_str(&format!("            is_default: {},\n", state.default));
            code.push_str(&format!("        }},\n"));
            code.push_str(&format!("    );\n"));
        }
    }
    
    code.push_str("    map\n");
    code.push_str("});\n\n");
    
    // 新增：方块名称到状态列表的映射
    code.push_str("/// 方块名称到其所有状态的映射\n");
    code.push_str("pub static BLOCK_STATES_BY_NAME: Lazy<HashMap<&'static str, Vec<u32>>> = Lazy::new(|| {\n");
    code.push_str("    let mut map = HashMap::new();\n");
    
    for (block_name, block_def) in blocks_data {
        let state_ids: Vec<String> = block_def.states.iter().map(|s| s.id.to_string()).collect();
        code.push_str(&format!("    map.insert(\"{}\", vec![{}]);\n", block_name, state_ids.join(", ")));
    }
    
    code.push_str("    map\n");
    code.push_str("});\n\n");
    
    // 新增：获取默认状态ID的函数
    code.push_str("/// 获取方块的默认状态ID\n");
    code.push_str("pub fn get_default_state_id(block_name: &str) -> Option<u32> {\n");
    code.push_str("    BLOCK_STATES_BY_NAME.get(block_name).and_then(|states| {\n");
    code.push_str("        states.iter().find_map(|&state_id| {\n");
    code.push_str("            BLOCK_STATE_BY_ID.get(&state_id).and_then(|info| {\n");
    code.push_str("                if info.is_default { Some(state_id) } else { None }\n");
    code.push_str("            })\n");
    code.push_str("        })\n");
    code.push_str("    })\n");
    code.push_str("}\n\n");
    
    // 新增：通过属性查找状态ID的函数
    code.push_str("/// 通过方块名称和属性查找状态ID\n");
    code.push_str("pub fn find_state_id_by_properties(block_name: &str, properties: &HashMap<&str, &str>) -> Option<u32> {\n");
    code.push_str("    BLOCK_STATES_BY_NAME.get(block_name).and_then(|states| {\n");
    code.push_str("        states.iter().find_map(|&state_id| {\n");
    code.push_str("            BLOCK_STATE_BY_ID.get(&state_id).and_then(|info| {\n");
    code.push_str("                if &info.properties == properties {\n");
    code.push_str("                    Some(state_id)\n");
    code.push_str("                } else {\n");
    code.push_str("                    None\n");
    code.push_str("                }\n");
    code.push_str("            })\n");
    code.push_str("        })\n");
    code.push_str("    })\n");
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
