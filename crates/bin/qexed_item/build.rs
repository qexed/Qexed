// crates/bin/qexed_item/build.rs
use std::env;
use std::fs;
use std::path::Path;
use heck::ToUpperCamelCase;
use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct RegistryFile {
    #[serde(rename = "minecraft:item")]
    item_registry: ItemRegistry,
}

#[derive(Debug, Deserialize)]
struct ItemRegistry {
    #[serde(default)]
    default: String,
    entries: std::collections::HashMap<String, ItemEntry>,
    #[serde(rename = "protocol_id")]
    registry_protocol_id: u32,
}

#[derive(Debug, Deserialize)]
struct ItemEntry {
    #[serde(rename = "protocol_id")]
    id: u32,
}

fn main() -> Result<()> {
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
    let json_content = fs::read_to_string(&json_path)
        .with_context(|| format!("Failed to read registry file: {:?}", json_path))?;
    
    let registry: RegistryFile = serde_json::from_str(&json_content)
        .with_context(|| "Failed to parse registry JSON")?;
    
    // 生成 Rust 代码
    let code = generate_item_code(&registry.item_registry)?;
    
    // 写入输出目录
    let out_dir = env::var("OUT_DIR")
        .context("Failed to get OUT_DIR")?;
    let dest_path = Path::new(&out_dir).join("item_registry_generated.rs");
    
    fs::write(&dest_path, code)
        .with_context(|| format!("Failed to write generated code to: {:?}", dest_path))?;
    
    println!("cargo:info=Generated item registry with {} entries", 
             registry.item_registry.entries.len());
    
    Ok(())
}

/// 生成物品注册表代码
fn generate_item_code(registry: &ItemRegistry) -> Result<String> {
    let mut items = Vec::new();
    
    // 收集物品信息
    for (name, entry) in &registry.entries {
        let enum_variant = name_to_enum_variant(name);
        let display_name = name_to_display_name(name);
        
        items.push(ItemInfo {
            id: entry.id,
            name: name.clone(),
            enum_variant,
            display_name,
        });
    }
    
    // 按ID排序
    items.sort_by_key(|b| b.id);
    
    // 生成代码
    let mut code = String::new();
    
    // 添加文件头
    code.push_str("// 自动生成的物品注册表\n");
    code.push_str("// 此文件由 build.rs 生成，请勿手动修改\n\n");
    
    // 添加导入
    code.push_str("use std::collections::HashMap;\n");
    code.push_str("use once_cell::sync::Lazy;\n");
    code.push_str("use phf::phf_map;\n");
    code.push_str("use strum::{EnumIter, Display, IntoEnumIterator};\n\n");
    
    // 生成物品ID枚举
    code.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, EnumIter, Display)]\n");
    code.push_str("#[repr(u32)]\n");
    code.push_str("pub enum ItemId {\n");
    
    for item in &items {
        code.push_str(&format!("    #[strum(to_string = \"{}\")]\n", item.display_name));
        code.push_str(&format!("    {} = {},\n", item.enum_variant, item.id));
    }
    
    code.push_str("}\n\n");
    
    // 生成 From/Into 实现
    code.push_str("impl ItemId {\n");
    code.push_str("    /// 转换为 i32（用于 VarInt）\n");
    code.push_str("    pub fn to_i32(self) -> i32 {\n");
    code.push_str("        self as u32 as i32\n");
    code.push_str("    }\n\n");
    
    code.push_str("    /// 转换为 u32\n");
    code.push_str("    pub fn to_u32(self) -> u32 {\n");
    code.push_str("        self as u32\n");
    code.push_str("    }\n\n");
    
    code.push_str("    /// 创建网络 VarInt\n");
    code.push_str("    pub fn to_var_int(self) -> qexed_packet::net_types::VarInt {\n");
    code.push_str("        qexed_packet::net_types::VarInt(self.to_i32())\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");
    
    code.push_str("impl From<ItemId> for i32 {\n");
    code.push_str("    fn from(id: ItemId) -> Self {\n");
    code.push_str("        id.to_i32()\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");
    
    code.push_str("impl From<ItemId> for u32 {\n");
    code.push_str("    fn from(id: ItemId) -> Self {\n");
    code.push_str("        id.to_u32()\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");
    
    code.push_str("impl TryFrom<i32> for ItemId {\n");
    code.push_str("    type Error = crate::error::ItemError;\n\n");
    code.push_str("    fn try_from(value: i32) -> Result<Self, Self::Error> {\n");
    code.push_str("        Self::try_from(value as u32)\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");
    
    code.push_str("impl TryFrom<u32> for ItemId {\n");
    code.push_str("    type Error = crate::error::ItemError;\n\n");
    code.push_str("    fn try_from(value: u32) -> Result<Self, Self::Error> {\n");
    code.push_str("        get_item_by_id(value)\n");
    code.push_str("            .ok_or(crate::error::ItemError::InvalidId(value))\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");
    
    code.push_str("impl TryFrom<qexed_packet::net_types::VarInt> for ItemId {\n");
    code.push_str("    type Error = crate::error::ItemError;\n\n");
    code.push_str("    fn try_from(var_int: qexed_packet::net_types::VarInt) -> Result<Self, Self::Error> {\n");
    code.push_str("        Self::try_from(var_int.0)\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");
    
    // 生成通过ID获取物品名称的函数
    code.push_str("/// 通过ID获取物品名称\n");
    code.push_str("pub fn get_item_name_by_id(id: u32) -> Option<&'static str> {\n");
    code.push_str("    match id {\n");
    
    for item in &items {
        code.push_str(&format!("        {} => Some(\"{}\"),\n", item.id, item.name));
    }
    
    code.push_str("        _ => None,\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");
    
    // 生成通过名称获取物品ID的函数
    code.push_str("/// 通过名称获取物品ID\n");
    code.push_str("pub fn get_item_id_by_name(name: &str) -> Option<u32> {\n");
    code.push_str("    match name {\n");
    
    for item in &items {
        code.push_str(&format!("        \"{}\" => Some({}),\n", item.name, item.id));
    }
    
    code.push_str("        _ => None,\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");
    
    // 生成通过ID获取物品枚举的函数
    code.push_str("/// 通过ID获取物品枚举\n");
    code.push_str("pub fn get_item_by_id(id: u32) -> Option<ItemId> {\n");
    code.push_str("    match id {\n");
    
    for item in &items {
        code.push_str(&format!("        {} => Some(ItemId::{}),\n", item.id, item.enum_variant));
    }
    
    code.push_str("        _ => None,\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");
    
    // 生成通过名称获取物品枚举的函数
    code.push_str("/// 通过名称获取物品枚举\n");
    code.push_str("pub fn get_item_by_name(name: &str) -> Option<ItemId> {\n");
    code.push_str("    get_item_id_by_name(name).and_then(get_item_by_id)\n");
    code.push_str("}\n\n");
    
    // 生成物品信息结构
    code.push_str("/// 物品信息结构\n");
    code.push_str("#[derive(Debug, Clone)]\n");
    code.push_str("pub struct ItemInfo {\n");
    code.push_str("    /// 物品协议ID\n");
    code.push_str("    pub id: u32,\n");
    code.push_str("    /// 物品内部名称（如 \"minecraft:stone\"）\n");
    code.push_str("    pub name: &'static str,\n");
    code.push_str("    /// 枚举变体名（如 \"Stone\"）\n");
    code.push_str("    pub enum_variant: &'static str,\n");
    code.push_str("    /// 显示名称（如 \"Stone\"）\n");
    code.push_str("    pub display_name: &'static str,\n");
    code.push_str("}\n\n");
    
    // 生成静态映射
    code.push_str("/// 物品ID到信息的静态映射\n");
    code.push_str("pub static ITEM_INFO_BY_ID: Lazy<HashMap<u32, ItemInfo>> = Lazy::new(|| {\n");
    code.push_str("    let mut map = HashMap::new();\n");
    
    for item in &items {
        code.push_str(&format!("    map.insert(\n"));
        code.push_str(&format!("        {},\n", item.id));
        code.push_str(&format!("        ItemInfo {{\n"));
        code.push_str(&format!("            id: {},\n", item.id));
        code.push_str(&format!("            name: \"{}\",\n", item.name));
        code.push_str(&format!("            enum_variant: \"{}\",\n", item.enum_variant));
        code.push_str(&format!("            display_name: \"{}\",\n", item.display_name));
        code.push_str(&format!("        }},\n"));
        code.push_str(&format!("    );\n"));
    }
    
    code.push_str("    map\n");
    code.push_str("});\n\n");
    
    // 生成PHF映射（编译时哈希）
    code.push_str("/// 物品名称到ID的PHF映射（编译时哈希）\n");
    code.push_str("pub static ITEM_ID_BY_NAME: phf::Map<&'static str, u32> = phf_map! {\n");
    
    for item in &items {
        code.push_str(&format!("    \"{}\" => {},\n", item.name, item.id));
    }
    
    code.push_str("};\n\n");
    
    // 生成默认物品ID
    let default_item_id = if let Some(default_entry) = registry.entries.get(&registry.default) {
        default_entry.id
    } else {
        0
    };
    
    code.push_str(&format!("/// 默认物品ID (通常是空气)\n"));
    code.push_str(&format!("pub const DEFAULT_ITEM_ID: u32 = {};\n", default_item_id));
    
    code.push_str(&format!("/// 默认物品枚举值\n"));
    if let Some(default_item) = items.iter().find(|i| i.id == default_item_id) {
        code.push_str(&format!("pub const DEFAULT_ITEM: ItemId = ItemId::{};\n\n", default_item.enum_variant));
    } else {
        code.push_str(&format!("pub const DEFAULT_ITEM: ItemId = ItemId::Air;\n\n"));
    }
    
    // 生成物品注册表协议ID
    code.push_str(&format!("/// 物品注册表的协议ID\n"));
    code.push_str(&format!("pub const ITEM_REGISTRY_PROTOCOL_ID: u32 = {};\n\n", registry.registry_protocol_id));
    
    // 生成辅助函数
    code.push_str("/// 获取所有物品ID\n");
    code.push_str("pub fn all_item_ids() -> Vec<u32> {\n");
    code.push_str("    vec![\n");
    
    for item in &items {
        code.push_str(&format!("        {},\n", item.id));
    }
    
    code.push_str("    ]\n");
    code.push_str("}\n\n");
    
    code.push_str("/// 获取所有物品名称\n");
    code.push_str("pub fn all_item_names() -> Vec<&'static str> {\n");
    code.push_str("    vec![\n");
    
    for item in &items {
        code.push_str(&format!("        \"{}\",\n", item.name));
    }
    
    code.push_str("    ]\n");
    code.push_str("}\n\n");
    
    code.push_str("/// 获取所有物品枚举值\n");
    code.push_str("pub fn all_items() -> Vec<ItemId> {\n");
    code.push_str("    ItemId::iter().collect()\n");
    code.push_str("}\n\n");
    
   code.push_str("/// 检查物品是否为工具\n");
   code.push_str("pub fn is_tool(_item: ItemId) -> bool {\n");
   code.push_str("    false\n");
   code.push_str("}\n\n");

   code.push_str("/// 检查物品是否为武器\n");
   code.push_str("pub fn is_weapon(_item: ItemId) -> bool {\n");
   code.push_str("    false\n");
   code.push_str("}\n\n");

   code.push_str("/// 检查物品是否为食物\n");
   code.push_str("pub fn is_food(_item: ItemId) -> bool {\n");
   code.push_str("    false\n");
   code.push_str("}\n");
    
    Ok(code)
}

/// 物品信息结构（用于代码生成）
struct ItemInfo {
    id: u32,
    name: String,
    enum_variant: String,
    display_name: String,
}

/// 将物品名称转换为枚举变体名
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

/// 将物品名称转换为显示名称
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