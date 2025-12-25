// crates/bin/qexed_entity/build.rs
use std::env;
use std::fs;
use std::path::Path;
use heck::ToUpperCamelCase;
use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct RegistryFile {
    #[serde(rename = "minecraft:entity_type")]
    entity_registry: EntityRegistry,
}

#[derive(Debug, Deserialize)]
struct EntityRegistry {
    #[serde(default)]
    default: String,
    entries: std::collections::HashMap<String, EntityEntry>,
    #[serde(rename = "protocol_id")]
    registry_protocol_id: u32,
}

#[derive(Debug, Deserialize)]
struct EntityEntry {
    #[serde(rename = "protocol_id")]
    id: u32,
}

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=../../../assets/registries.json");
    
    // 获取项目根目录
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")?;
    
    // 构建 JSON 文件路径
    let mut json_path = Path::new(&manifest_dir).to_path_buf();
    for _ in 0..3 {
        if let Some(parent) = json_path.parent() {
            json_path = parent.to_path_buf();
        }
    }
    json_path = json_path.join("assets").join("registries.json");
    
    println!("cargo:info=Loading entity registry from: {:?}", json_path);
    
    // 读取和解析 JSON
    let json_content = fs::read_to_string(&json_path)?;
    
    let registry: RegistryFile = match serde_json::from_str(&json_content) {
        Ok(r) => r,
        Err(e) => {
            println!("cargo:warning=Failed to parse registry: {}, using fallback", e);
            return generate_fallback_code();
        }
    };
    
    // 生成 Rust 代码
    let code = generate_entity_code(&registry.entity_registry)?;
    
    // 写入输出目录
    let out_dir = env::var("OUT_DIR")?;
    let dest_path = Path::new(&out_dir).join("entity_registry_generated.rs");
    
    fs::write(&dest_path, code)?;
    
    println!("cargo:info=Generated entity registry with {} entries", 
             registry.entity_registry.entries.len());
    
    Ok(())
}

/// 生成实体注册表代码
fn generate_entity_code(registry: &EntityRegistry) -> Result<String> {
    let mut entities = Vec::new();
    
    // 收集实体信息
    for (name, entry) in &registry.entries {
        // 跳过无效的名称
        if name.is_empty() {
            continue;
        }
        
        let enum_variant = name_to_enum_variant(name);
        if enum_variant.is_empty() {
            continue;
        }
        
        let display_name = name_to_display_name(name);
        
        entities.push(EntityInfo {
            id: entry.id,
            name: name.clone(),
            enum_variant,
            display_name,
        });
    }
    
    // 按ID排序
    entities.sort_by_key(|e| e.id);
    
    // 生成代码
    let mut code = String::new();
    
    // 添加文件头
    code.push_str("// 自动生成的实体注册表\n");
    code.push_str("// 此文件由 build.rs 生成，请勿手动修改\n\n");
    
    // 添加导入
    code.push_str("use std::collections::HashMap;\n");
    code.push_str("use once_cell::sync::Lazy;\n");
    code.push_str("use phf::phf_map;\n");
    code.push_str("use strum::{EnumIter, Display, IntoEnumIterator};\n\n");
    
    // 生成实体ID枚举
    if entities.is_empty() {
        // 回退枚举
        code.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, EnumIter, Display)]\n");
        code.push_str("#[repr(u32)]\n");
        code.push_str("pub enum EntityId {\n");
        code.push_str("    #[strum(to_string = \"Pig\")]\n");
        code.push_str("    Pig = 0,\n");
        code.push_str("    #[strum(to_string = \"Cow\")]\n");
        code.push_str("    Cow = 1,\n");
        code.push_str("    #[strum(to_string = \"Sheep\")]\n");
        code.push_str("    Sheep = 2,\n");
        code.push_str("    #[strum(to_string = \"Chicken\")]\n");
        code.push_str("    Chicken = 3,\n");
        code.push_str("    #[strum(to_string = \"Zombie\")]\n");
        code.push_str("    Zombie = 4,\n");
        code.push_str("}\n\n");
    } else {
        code.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, EnumIter, Display)]\n");
        code.push_str("#[repr(u32)]\n");
        code.push_str("pub enum EntityId {\n");
        
        for entity in &entities {
            // 确保枚举变体名有效
            if !entity.enum_variant.chars().next().map_or(false, |c| c.is_ascii_uppercase()) {
                continue;
            }
            
            code.push_str(&format!("    #[strum(to_string = \"{}\")]\n", entity.display_name));
            code.push_str(&format!("    {} = {},\n", entity.enum_variant, entity.id));
        }
        
        code.push_str("}\n\n");
    }
    
    // 生成转换方法
    code.push_str("impl EntityId {\n");
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
    
    // 实现 From trait
    code.push_str("impl From<EntityId> for i32 {\n");
    code.push_str("    fn from(id: EntityId) -> Self {\n");
    code.push_str("        id.to_i32()\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");
    
    code.push_str("impl From<EntityId> for u32 {\n");
    code.push_str("    fn from(id: EntityId) -> Self {\n");
    code.push_str("        id.to_u32()\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");
    
    // 实现 TryFrom trait
    code.push_str("impl TryFrom<i32> for EntityId {\n");
    code.push_str("    type Error = crate::error::EntityError;\n\n");
    code.push_str("    fn try_from(value: i32) -> Result<Self, Self::Error> {\n");
    code.push_str("        Self::try_from(value as u32)\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");
    
    code.push_str("impl TryFrom<u32> for EntityId {\n");
    code.push_str("    type Error = crate::error::EntityError;\n\n");
    code.push_str("    fn try_from(value: u32) -> Result<Self, Self::Error> {\n");
    code.push_str("        get_entity_by_id(value)\n");
    code.push_str("            .ok_or(crate::error::EntityError::InvalidId(value))\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");
    
    // 生成通过ID获取实体名称的函数
    code.push_str("/// 通过ID获取实体名称\n");
    code.push_str("pub fn get_entity_name_by_id(id: u32) -> Option<&'static str> {\n");
    
    if entities.is_empty() {
        code.push_str("    match id {\n");
        code.push_str("        0 => Some(\"minecraft:pig\"),\n");
        code.push_str("        1 => Some(\"minecraft:cow\"),\n");
        code.push_str("        2 => Some(\"minecraft:sheep\"),\n");
        code.push_str("        3 => Some(\"minecraft:chicken\"),\n");
        code.push_str("        4 => Some(\"minecraft:zombie\"),\n");
        code.push_str("        _ => None,\n");
        code.push_str("    }\n");
    } else {
        code.push_str("    match id {\n");
        
        for entity in &entities {
            code.push_str(&format!("        {} => Some(\"{}\"),\n", entity.id, entity.name));
        }
        
        code.push_str("        _ => None,\n");
        code.push_str("    }\n");
    }
    
    code.push_str("}\n\n");
    
    // 生成通过名称获取实体ID的函数
    code.push_str("/// 通过名称获取实体ID\n");
    code.push_str("pub fn get_entity_id_by_name(name: &str) -> Option<u32> {\n");
    
    if entities.is_empty() {
        code.push_str("    match name {\n");
        code.push_str("        \"minecraft:pig\" => Some(0),\n");
        code.push_str("        \"minecraft:cow\" => Some(1),\n");
        code.push_str("        \"minecraft:sheep\" => Some(2),\n");
        code.push_str("        \"minecraft:chicken\" => Some(3),\n");
        code.push_str("        \"minecraft:zombie\" => Some(4),\n");
        code.push_str("        _ => None,\n");
        code.push_str("    }\n");
    } else {
        code.push_str("    match name {\n");
        
        for entity in &entities {
            code.push_str(&format!("        \"{}\" => Some({}),\n", entity.name, entity.id));
        }
        
        code.push_str("        _ => None,\n");
        code.push_str("    }\n");
    }
    
    code.push_str("}\n\n");
    
    // 生成通过ID获取实体枚举的函数
    code.push_str("/// 通过ID获取实体枚举\n");
    code.push_str("pub fn get_entity_by_id(id: u32) -> Option<EntityId> {\n");
    
    if entities.is_empty() {
        code.push_str("    match id {\n");
        code.push_str("        0 => Some(EntityId::Pig),\n");
        code.push_str("        1 => Some(EntityId::Cow),\n");
        code.push_str("        2 => Some(EntityId::Sheep),\n");
        code.push_str("        3 => Some(EntityId::Chicken),\n");
        code.push_str("        4 => Some(EntityId::Zombie),\n");
        code.push_str("        _ => None,\n");
        code.push_str("    }\n");
    } else {
        code.push_str("    match id {\n");
        
        for entity in &entities {
            code.push_str(&format!("        {} => Some(EntityId::{}),\n", entity.id, entity.enum_variant));
        }
        
        code.push_str("        _ => None,\n");
        code.push_str("    }\n");
    }
    
    code.push_str("}\n\n");
    
    // 生成通过名称获取实体枚举的函数
    code.push_str("/// 通过名称获取实体枚举\n");
    code.push_str("pub fn get_entity_by_name(name: &str) -> Option<EntityId> {\n");
    code.push_str("    get_entity_id_by_name(name).and_then(get_entity_by_id)\n");
    code.push_str("}\n\n");
    
    // 生成实体信息结构
    code.push_str("/// 实体信息结构\n");
    code.push_str("#[derive(Debug, Clone)]\n");
    code.push_str("pub struct EntityInfo {\n");
    code.push_str("    /// 实体协议ID\n");
    code.push_str("    pub id: u32,\n");
    code.push_str("    /// 实体内部名称（如 \"minecraft:pig\"）\n");
    code.push_str("    pub name: &'static str,\n");
    code.push_str("    /// 枚举变体名（如 \"Pig\"）\n");
    code.push_str("    pub enum_variant: &'static str,\n");
    code.push_str("    /// 显示名称（如 \"Pig\"）\n");
    code.push_str("    pub display_name: &'static str,\n");
    code.push_str("}\n\n");
    
    // 生成静态映射
    if entities.is_empty() {
        code.push_str("pub static ENTITY_INFO_BY_ID: Lazy<HashMap<u32, EntityInfo>> = Lazy::new(|| {\n");
        code.push_str("    let mut map = HashMap::new();\n");
        code.push_str("    map.insert(0, EntityInfo {\n");
        code.push_str("        id: 0,\n");
        code.push_str("        name: \"minecraft:pig\",\n");
        code.push_str("        enum_variant: \"Pig\",\n");
        code.push_str("        display_name: \"Pig\",\n");
        code.push_str("    });\n");
        code.push_str("    map.insert(1, EntityInfo {\n");
        code.push_str("        id: 1,\n");
        code.push_str("        name: \"minecraft:cow\",\n");
        code.push_str("        enum_variant: \"Cow\",\n");
        code.push_str("        display_name: \"Cow\",\n");
        code.push_str("    });\n");
        code.push_str("    map.insert(2, EntityInfo {\n");
        code.push_str("        id: 2,\n");
        code.push_str("        name: \"minecraft:sheep\",\n");
        code.push_str("        enum_variant: \"Sheep\",\n");
        code.push_str("        display_name: \"Sheep\",\n");
        code.push_str("    });\n");
        code.push_str("    map.insert(3, EntityInfo {\n");
        code.push_str("        id: 3,\n");
        code.push_str("        name: \"minecraft:chicken\",\n");
        code.push_str("        enum_variant: \"Chicken\",\n");
        code.push_str("        display_name: \"Chicken\",\n");
        code.push_str("    });\n");
        code.push_str("    map.insert(4, EntityInfo {\n");
        code.push_str("        id: 4,\n");
        code.push_str("        name: \"minecraft:zombie\",\n");
        code.push_str("        enum_variant: \"Zombie\",\n");
        code.push_str("        display_name: \"Zombie\",\n");
        code.push_str("    });\n");
        code.push_str("    map\n");
        code.push_str("});\n\n");
    } else {
        code.push_str("pub static ENTITY_INFO_BY_ID: Lazy<HashMap<u32, EntityInfo>> = Lazy::new(|| {\n");
        code.push_str("    let mut map = HashMap::new();\n");
        
        for entity in &entities {
            code.push_str(&format!("    map.insert({}, EntityInfo {{\n", entity.id));
            code.push_str(&format!("        id: {},\n", entity.id));
            code.push_str(&format!("        name: \"{}\",\n", entity.name));
            code.push_str(&format!("        enum_variant: \"{}\",\n", entity.enum_variant));
            code.push_str(&format!("        display_name: \"{}\",\n", entity.display_name));
            code.push_str("    });\n");
        }
        
        code.push_str("    map\n");
        code.push_str("});\n\n");
    }
    
    // 生成PHF映射（编译时哈希）
    if !entities.is_empty() {
        code.push_str("/// 实体名称到ID的PHF映射（编译时哈希）\n");
        code.push_str("pub static ENTITY_ID_BY_NAME: phf::Map<&'static str, u32> = phf_map! {\n");
        
        for entity in &entities {
            code.push_str(&format!("    \"{}\" => {},\n", entity.name, entity.id));
        }
        
        code.push_str("};\n\n");
    }
    
    // 生成默认实体ID
    let default_entity_id = if let Some(default_entry) = registry.entries.get(&registry.default) {
        default_entry.id
    } else {
        0
    };
    
    code.push_str(&format!("/// 默认实体ID\n"));
    code.push_str(&format!("pub const DEFAULT_ENTITY_ID: u32 = {};\n", default_entity_id));
    
    // 找到默认实体枚举
    if let Some(default_entity) = entities.iter().find(|e| e.id == default_entity_id) {
        code.push_str(&format!("/// 默认实体枚举值\n"));
        code.push_str(&format!("pub const DEFAULT_ENTITY: EntityId = EntityId::{};\n\n", default_entity.enum_variant));
    } else {
        code.push_str(&format!("/// 默认实体枚举值\n"));
        code.push_str(&format!("pub const DEFAULT_ENTITY: EntityId = EntityId::Pig;\n\n"));
    }
    
    // 生成实体注册表协议ID
    code.push_str(&format!("/// 实体注册表的协议ID\n"));
    code.push_str(&format!("pub const ENTITY_REGISTRY_PROTOCOL_ID: u32 = {};\n\n", registry.registry_protocol_id));
    
    // 生成辅助函数
    code.push_str("/// 获取所有实体ID\n");
    code.push_str("pub fn all_entity_ids() -> Vec<u32> {\n");
    
    if entities.is_empty() {
        code.push_str("    vec![0, 1, 2, 3, 4]\n");
    } else {
        code.push_str("    vec![\n");
        for entity in &entities {
            code.push_str(&format!("        {},\n", entity.id));
        }
        code.push_str("    ]\n");
    }
    
    code.push_str("}\n\n");
    
    code.push_str("/// 获取所有实体枚举值\n");
    code.push_str("pub fn all_entities() -> Vec<EntityId> {\n");
    code.push_str("    EntityId::iter().collect()\n");
    code.push_str("}\n\n");
    
    // 生成实体分类函数
    code.push_str("/// 检查实体是否为生物\n");
    code.push_str("pub fn is_living_entity(entity: EntityId) -> bool {\n");
    code.push_str("    // 这里是简化的判断逻辑，实际实现需要更复杂的逻辑\n");
    code.push_str("    !is_item_entity(entity) && !is_projectile(entity)\n");
    code.push_str("}\n\n");
    
    code.push_str("/// 检查实体是否为物品实体\n");
    code.push_str("pub fn is_item_entity(_entity: EntityId) -> bool {\n");
    code.push_str("    false // 简化实现\n");
    code.push_str("}\n\n");
    
    code.push_str("/// 检查实体是否为弹射物\n");
    code.push_str("pub fn is_projectile(_entity: EntityId) -> bool {\n");
    code.push_str("    false // 简化实现\n");
    code.push_str("}\n\n");
    
    // 生成实体属性查询函数
    code.push_str("/// 获取实体的碰撞箱大小\n");
    code.push_str("pub fn get_entity_bounding_box(entity: EntityId) -> (f32, f32) {\n");
    code.push_str("    // 返回 (宽度, 高度)，简化实现\n");
    code.push_str("    match entity {\n");
    code.push_str("        _ => (0.6, 1.8), // 默认玩家大小\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");
    
    code.push_str("/// 获取实体的最大生命值\n");
    code.push_str("pub fn get_entity_max_health(entity: EntityId) -> f32 {\n");
    code.push_str("    // 简化实现\n");
    code.push_str("    match entity {\n");
    code.push_str("        EntityId::Pig => 10.0,\n");
    code.push_str("        EntityId::Cow => 10.0,\n");
    code.push_str("        EntityId::Sheep => 8.0,\n");
    code.push_str("        EntityId::Chicken => 4.0,\n");
    code.push_str("        EntityId::Zombie => 20.0,\n");
    code.push_str("        _ => 20.0, // 默认\n");
    code.push_str("    }\n");
    code.push_str("}\n");
    
    Ok(code)
}

/// 生成回退代码
fn generate_fallback_code() -> Result<()> {
    let out_dir = env::var("OUT_DIR")?;
    let dest_path = Path::new(&out_dir).join("entity_registry_generated.rs");
    
    let code = r#"//! 回退实体注册表
//! 此文件由 build.rs 生成，因为无法读取主注册表文件

use std::collections::HashMap;
use once_cell::sync::Lazy;
use strum::{EnumIter, Display, IntoEnumIterator};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, EnumIter, Display)]
#[repr(u32)]
pub enum EntityId {
    #[strum(to_string = "Pig")]
    Pig = 0,
    #[strum(to_string = "Cow")]
    Cow = 1,
    #[strum(to_string = "Sheep")]
    Sheep = 2,
    #[strum(to_string = "Chicken")]
    Chicken = 3,
    #[strum(to_string = "Zombie")]
    Zombie = 4,
}

impl EntityId {
    /// 转换为 i32（用于 VarInt）
    pub fn to_i32(self) -> i32 {
        self as u32 as i32
    }

    /// 转换为 u32
    pub fn to_u32(self) -> u32 {
        self as u32
    }

    /// 创建网络 VarInt
    pub fn to_var_int(self) -> qexed_packet::net_types::VarInt {
        qexed_packet::net_types::VarInt(self.to_i32())
    }
}

impl From<EntityId> for i32 {
    fn from(id: EntityId) -> Self {
        id.to_i32()
    }
}

impl From<EntityId> for u32 {
    fn from(id: EntityId) -> Self {
        id.to_u32()
    }
}

impl TryFrom<i32> for EntityId {
    type Error = crate::error::EntityError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Self::try_from(value as u32)
    }
}

impl TryFrom<u32> for EntityId {
    type Error = crate::error::EntityError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        get_entity_by_id(value)
            .ok_or(crate::error::EntityError::InvalidId(value))
    }
}

/// 通过ID获取实体名称
pub fn get_entity_name_by_id(id: u32) -> Option<&'static str> {
    match id {
        0 => Some("minecraft:pig"),
        1 => Some("minecraft:cow"),
        2 => Some("minecraft:sheep"),
        3 => Some("minecraft:chicken"),
        4 => Some("minecraft:zombie"),
        _ => None,
    }
}

/// 通过名称获取实体ID
pub fn get_entity_id_by_name(name: &str) -> Option<u32> {
    match name {
        "minecraft:pig" => Some(0),
        "minecraft:cow" => Some(1),
        "minecraft:sheep" => Some(2),
        "minecraft:chicken" => Some(3),
        "minecraft:zombie" => Some(4),
        _ => None,
    }
}

/// 通过ID获取实体枚举
pub fn get_entity_by_id(id: u32) -> Option<EntityId> {
    match id {
        0 => Some(EntityId::Pig),
        1 => Some(EntityId::Cow),
        2 => Some(EntityId::Sheep),
        3 => Some(EntityId::Chicken),
        4 => Some(EntityId::Zombie),
        _ => None,
    }
}

/// 通过名称获取实体枚举
pub fn get_entity_by_name(name: &str) -> Option<EntityId> {
    get_entity_id_by_name(name).and_then(get_entity_by_id)
}

/// 实体信息结构
#[derive(Debug, Clone)]
pub struct EntityInfo {
    /// 实体协议ID
    pub id: u32,
    /// 实体内部名称（如 "minecraft:pig"）
    pub name: &'static str,
    /// 枚举变体名（如 "Pig"）
    pub enum_variant: &'static str,
    /// 显示名称（如 "Pig"）
    pub display_name: &'static str,
}

pub static ENTITY_INFO_BY_ID: Lazy<HashMap<u32, EntityInfo>> = Lazy::new(|| {
    let mut map = HashMap::new();
    map.insert(0, EntityInfo {
        id: 0,
        name: "minecraft:pig",
        enum_variant: "Pig",
        display_name: "Pig",
    });
    map.insert(1, EntityInfo {
        id: 1,
        name: "minecraft:cow",
        enum_variant: "Cow",
        display_name: "Cow",
    });
    map.insert(2, EntityInfo {
        id: 2,
        name: "minecraft:sheep",
        enum_variant: "Sheep",
        display_name: "Sheep",
    });
    map.insert(3, EntityInfo {
        id: 3,
        name: "minecraft:chicken",
        enum_variant: "Chicken",
        display_name: "Chicken",
    });
    map.insert(4, EntityInfo {
        id: 4,
        name: "minecraft:zombie",
        enum_variant: "Zombie",
        display_name: "Zombie",
    });
    map
});

/// 默认实体ID
pub const DEFAULT_ENTITY_ID: u32 = 0;

/// 默认实体枚举值
pub const DEFAULT_ENTITY: EntityId = EntityId::Pig;

/// 实体注册表的协议ID
pub const ENTITY_REGISTRY_PROTOCOL_ID: u32 = 0;

/// 获取所有实体ID
pub fn all_entity_ids() -> Vec<u32> {
    vec![0, 1, 2, 3, 4]
}

/// 获取所有实体枚举值
pub fn all_entities() -> Vec<EntityId> {
    EntityId::iter().collect()
}

/// 检查实体是否为生物
pub fn is_living_entity(_entity: EntityId) -> bool {
    true
}

/// 检查实体是否为物品实体
pub fn is_item_entity(_entity: EntityId) -> bool {
    false
}

/// 检查实体是否为弹射物
pub fn is_projectile(_entity: EntityId) -> bool {
    false
}

/// 获取实体的碰撞箱大小
pub fn get_entity_bounding_box(entity: EntityId) -> (f32, f32) {
    match entity {
        EntityId::Pig => (0.9, 0.9),
        EntityId::Cow => (0.9, 1.4),
        EntityId::Sheep => (0.9, 1.3),
        EntityId::Chicken => (0.4, 0.7),
        EntityId::Zombie => (0.6, 1.95),
        _ => (0.6, 1.8),
    }
}

/// 获取实体的最大生命值
pub fn get_entity_max_health(entity: EntityId) -> f32 {
    match entity {
        EntityId::Pig => 10.0,
        EntityId::Cow => 10.0,
        EntityId::Sheep => 8.0,
        EntityId::Chicken => 4.0,
        EntityId::Zombie => 20.0,
        _ => 20.0,
    }
}
"#;
    
    fs::write(dest_path, code)?;
    println!("cargo:info=Generated fallback entity registry");
    Ok(())
}

/// 实体信息结构（用于代码生成）
struct EntityInfo {
    id: u32,
    name: String,
    enum_variant: String,
    display_name: String,
}

/// 将实体名称转换为枚举变体名
fn name_to_enum_variant(name: &str) -> String {
    // 移除命名空间前缀
    let name_without_namespace = if let Some(pos) = name.find(':') {
        &name[pos + 1..]
    } else {
        name
    };
    
    // 检查名称是否有效
    if name_without_namespace.is_empty() {
        return String::new();
    }
    
    // 转换为大驼峰命名
    name_without_namespace.to_upper_camel_case()
}

/// 将实体名称转换为显示名称
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