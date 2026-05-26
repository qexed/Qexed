use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use anyhow::{Context, Result};

const REGISTRIES_REPORT: &str = "assets/reports/registries.json";

pub(crate) fn entity_type_id(name: &str) -> Result<i32> {
    entity_type_registry()
        .get(name)
        .copied()
        .with_context(|| format!("missing entity type registry id: {name}"))
}

fn entity_type_registry() -> &'static HashMap<String, i32> {
    static REGISTRY: OnceLock<HashMap<String, i32>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        load_registry_id_map("minecraft:entity_type").unwrap_or_else(|err| {
            log::warn!("failed to load entity type registry ids: {err:#}");
            HashMap::from([
                ("minecraft:armor_stand".to_string(), 5),
                ("minecraft:player".to_string(), 155),
                ("minecraft:villager".to_string(), 139),
            ])
        })
    })
}

fn load_registry_id_map(registry_id: &str) -> Result<HashMap<String, i32>> {
    let path = workspace_root().join(REGISTRIES_REPORT);
    let content =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let value: serde_json::Value =
        serde_json::from_str(&content).with_context(|| format!("parse {}", path.display()))?;
    let entries = value
        .get(registry_id)
        .and_then(|registry| registry.get("entries"))
        .and_then(serde_json::Value::as_object)
        .with_context(|| format!("registry not found in {}: {registry_id}", path.display()))?;

    let mut ids = HashMap::new();
    for (name, value) in entries {
        let Some(id) = value
            .get("protocol_id")
            .and_then(serde_json::Value::as_i64)
            .and_then(|id| i32::try_from(id).ok())
        else {
            continue;
        };
        ids.insert(name.clone(), id);
    }

    Ok(ids)
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}
