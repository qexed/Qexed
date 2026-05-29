use std::{collections::HashMap, sync::OnceLock};

use anyhow::{Context, Result};

pub(crate) fn entity_type_id(name: &str) -> Result<i32> {
    entity_type_registry()
        .get(name)
        .copied()
        .with_context(|| format!("missing entity type registry id: {name}"))
}

fn entity_type_registry() -> &'static HashMap<String, i32> {
    static REGISTRY: OnceLock<HashMap<String, i32>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        crate::registry_sync::load_registry_id_map("minecraft:entity_type").unwrap_or_else(|err| {
            log::warn!("failed to load entity type registry ids: {err:#}");
            HashMap::from([
                ("minecraft:item".to_string(), 71),
                ("minecraft:armor_stand".to_string(), 5),
                ("minecraft:player".to_string(), 155),
                ("minecraft:villager".to_string(), 139),
            ])
        })
    })
}
