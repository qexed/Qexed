//! 实体类型注册表 id 查询。
//!
//! 数据源：assets/reports/registries.json 的 minecraft:entity_type 全量表
//! （qexed_mojang_data::registry_sync::load_registry_id_map，OnceLock 缓存，
//! 首次调用加载 161 个实体类型）。加载失败时回落内置兜底表；
//! install_entity_type_registry 注入的映射优先级最高（server 域覆盖用）。

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

use crate::error::EntitiesError;

/// 注入完整注册表映射（由 server 域启动时调用；覆盖报告加载的表）。
pub fn install_entity_type_registry(map: HashMap<String, i32>) {
    let registry = dynamic_registry();
    let mut guard = registry.lock().expect("entity type registry poisoned");
    *guard = Some(map);
}

pub fn entity_type_id(name: &str) -> Result<i32, EntitiesError> {
    let name = normalize(name);
    builtin_registry()
        .get(name.as_str())
        .copied()
        .or_else(|| report_registry().get(name.as_str()).copied())
        .or_else(|| {
            dynamic_registry()
                .lock()
                .expect("entity type registry poisoned")
                .as_ref()
                .and_then(|map| map.get(name.as_str()).copied())
        })
        .ok_or_else(|| EntitiesError::MissingEntityTypeRegistryId {
            entity_type: name,
        })
}

fn normalize(name: &str) -> String {
    if name.contains(':') {
        name.to_string()
    } else {
        format!("minecraft:{name}")
    }
}

/// 报告全量表：registries.json minecraft:entity_type（OnceLock 缓存）。
fn report_registry() -> &'static HashMap<String, i32> {
    static REGISTRY: OnceLock<HashMap<String, i32>> = OnceLock::new();
    REGISTRY.get_or_init(|| match load_report_registry() {
        Ok(map) => map,
        Err(err) => {
            log::warn!(
                "{}",
                qexed_language::t("qexed.entities.registry_fallback")
                    .replace("%{error}", &format!("{err:#}"))
            );
            HashMap::new()
        }
    })
}

fn load_report_registry(
) -> Result<HashMap<String, i32>, qexed_mojang_data::registry_sync::RegistryError> {
    qexed_mojang_data::registry_sync::load_registry_id_map("minecraft:entity_type")
        .map_err(std::convert::Into::into)
}

fn dynamic_registry() -> &'static Mutex<Option<HashMap<String, i32>>> {
    static REGISTRY: OnceLock<Mutex<Option<HashMap<String, i32>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(None))
}

/// 内置兜底表（报告缺失时的最小集，v6 26.3 id）。
fn builtin_registry() -> &'static HashMap<String, i32> {
    static REGISTRY: OnceLock<HashMap<String, i32>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        HashMap::from([
            ("minecraft:item".to_string(), 72),
            ("minecraft:armor_stand".to_string(), 5),
            ("minecraft:player".to_string(), 159),
            ("minecraft:villager".to_string(), 143),
            ("minecraft:text_display".to_string(), 135),
            ("minecraft:zombie".to_string(), 154),
            ("minecraft:skeleton".to_string(), 135),
            ("minecraft:creeper".to_string(), 32),
        ])
    })
}