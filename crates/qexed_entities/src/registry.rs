//! 实体类型注册表 id 查询。
//!
//! v4 通过 `crate::registry_sync::load_registry_id_map("minecraft:entity_type")`
//! 从 Mojang 报告加载完整映射；v6 该机制尚未迁移（registry_sync 属 server 域），
//! 这里保留内置兜底表 + 运行时注入接口，等 server 域迁移后接回完整数据源。

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

use crate::error::EntitiesError;

/// 注入完整注册表映射（由 server 域启动时调用；未注入时使用内置兜底表）。
pub fn install_entity_type_registry(map: HashMap<String, i32>) {
    let registry = dynamic_registry();
    let mut guard = registry.lock().expect("entity type registry poisoned");
    *guard = Some(map);
}

pub fn entity_type_id(name: &str) -> Result<i32, EntitiesError> {
    builtin_registry()
        .get(name)
        .copied()
        .or_else(|| {
            dynamic_registry()
                .lock()
                .expect("entity type registry poisoned")
                .as_ref()
                .and_then(|map| map.get(name).copied())
        })
        .ok_or_else(|| EntitiesError::MissingEntityTypeRegistryId {
            entity_type: name.to_string(),
        })
}

fn dynamic_registry() -> &'static Mutex<Option<HashMap<String, i32>>> {
    static REGISTRY: OnceLock<Mutex<Option<HashMap<String, i32>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(None))
}

/// 内置兜底表：26.3 常用实体 id（与 v4 缺省表一致 + 迁移中用到的类型）。
fn builtin_registry() -> &'static HashMap<String, i32> {
    static REGISTRY: OnceLock<HashMap<String, i32>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        HashMap::from([
            ("minecraft:item".to_string(), 71),
            ("minecraft:armor_stand".to_string(), 5),
            ("minecraft:player".to_string(), 155),
            ("minecraft:villager".to_string(), 139),
            ("minecraft:text_display".to_string(), 131),
            ("minecraft:zombie".to_string(), 173),
            ("minecraft:skeleton".to_string(), 156),
            ("minecraft:creeper".to_string(), 36),
            ("minecraft:spider".to_string(), 152),
            ("minecraft:cave_spider".to_string(), 30),
            ("minecraft:slime".to_string(), 148),
            ("minecraft:magma_cube".to_string(), 106),
            ("minecraft:witch".to_string(), 168),
            ("minecraft:blaze".to_string(), 15),
            ("minecraft:ghast".to_string(), 54),
            ("minecraft:shulker".to_string(), 137),
            ("minecraft:breeze".to_string(), 20),
            ("minecraft:wither".to_string(), 166),
            ("minecraft:wither_skeleton".to_string(), 167),
            ("minecraft:husk".to_string(), 88),
            ("minecraft:stray".to_string(), 157),
            ("minecraft:bogged".to_string(), 18),
            ("minecraft:drowned".to_string(), 46),
            ("minecraft:guardian".to_string(), 65),
            ("minecraft:elder_guardian".to_string(), 25),
            ("minecraft:evoker".to_string(), 52),
            ("minecraft:vex".to_string(), 162),
            ("minecraft:evoker_fangs".to_string(), 51),
            ("minecraft:enderman".to_string(), 47),
            ("minecraft:phantom".to_string(), 126),
            ("minecraft:warden".to_string(), 163),
            ("minecraft:ravager".to_string(), 135),
            ("minecraft:zoglin".to_string(), 172),
            ("minecraft:hoglin".to_string(), 86),
            ("minecraft:piglin".to_string(), 128),
            ("minecraft:piglin_brute".to_string(), 129),
            ("minecraft:zombified_piglin".to_string(), 174),
            ("minecraft:vindicator".to_string(), 161),
            ("minecraft:pillager".to_string(), 127),
            ("minecraft:illusioner".to_string(), 90),
            ("minecraft:iron_golem".to_string(), 94),
            ("minecraft:snow_golem".to_string(), 147),
            ("minecraft:wolf".to_string(), 170),
            ("minecraft:fox".to_string(), 29),
            ("minecraft:rabbit".to_string(), 133),
            ("minecraft:sheep".to_string(), 141),
            ("minecraft:pig".to_string(), 126),
            ("minecraft:cow".to_string(), 91),
            ("minecraft:chicken".to_string(), 32),
            ("minecraft:cat".to_string(), 28),
            ("minecraft:ocelot".to_string(), 118),
            ("minecraft:horse".to_string(), 89),
            ("minecraft:bee".to_string(), 14),
            ("minecraft:polar_bear".to_string(), 130),
            ("minecraft:wandering_trader".to_string(), 162),
            ("minecraft:arrow".to_string(), 8),
            ("minecraft:small_fireball".to_string(), 144),
            ("minecraft:fireball".to_string(), 55),
            ("minecraft:splash_potion".to_string(), 149),
            ("minecraft:shulker_bullet".to_string(), 138),
            ("minecraft:wind_charge".to_string(), 169),
            ("minecraft:wither_skull".to_string(), 168),
            ("minecraft:trident".to_string(), 159),
            ("minecraft:snowball".to_string(), 146),
        ])
    })
}
