use std::collections::HashMap;

use anyhow::{Context, Result};
use qexed_protocol::to_client::configuration::registry_data::{Entries, RegistryData};
use serde_json::Value;

use super::{
    DATA_ROOT, REGISTRIES_REPORT, SYNCHRONIZED_REGISTRIES,
    util::{
        entry_id_from_path, json_files, json_to_nbt, normalize_identifier, read_json,
        workspace_root,
    },
};

pub fn load_registry_packets(include_contents: bool) -> Result<Vec<RegistryData>> {
    let data_root = workspace_root().join(DATA_ROOT);
    let mut packets = Vec::new();

    for registry in SYNCHRONIZED_REGISTRIES {
        let registry_dir = data_root.join(registry);
        if !registry_dir.exists() {
            log::debug!("跳过缺失的同步注册表目录: {}", registry_dir.display());
            continue;
        }

        let mut entries = load_registry_entries(&registry_dir, registry, include_contents)?;
        if entries.is_empty() {
            continue;
        }

        entries.sort_by(|a, b| a.entry_id.cmp(&b.entry_id));
        packets.push(RegistryData {
            id: format!("minecraft:{registry}"),
            entries,
        });
    }

    Ok(packets)
}

fn load_registry_entries(
    registry_dir: &std::path::Path,
    registry: &str,
    include_contents: bool,
) -> Result<Vec<Entries>> {
    let mut entries = Vec::new();
    for path in json_files(registry_dir)? {
        let entry_id = entry_id_from_path(registry_dir, &path, registry)?;
        let data = if include_contents {
            let value = read_json(&path)?;
            Some(json_to_nbt(&value)?)
        } else {
            None
        };

        entries.push(Entries { entry_id, data });
    }
    Ok(entries)
}

pub(super) fn load_dynamic_registry_id_map(
    registry_dir: &std::path::Path,
    registry: &str,
) -> Result<HashMap<String, i32>> {
    let mut ids = HashMap::new();
    let mut entries = Vec::new();

    if !registry_dir.exists() {
        return Ok(ids);
    }

    for path in json_files(registry_dir)? {
        entries.push(entry_id_from_path(registry_dir, &path, registry)?);
    }
    entries.sort();

    for (index, entry) in entries.into_iter().enumerate() {
        ids.insert(entry, index as i32);
    }

    Ok(ids)
}

pub(super) fn load_static_registry_id_maps() -> Result<HashMap<String, HashMap<String, i32>>> {
    let path = workspace_root().join(REGISTRIES_REPORT);
    if !path.exists() {
        log::debug!("跳过缺失的静态注册表报告: {}", path.display());
        return Ok(HashMap::new());
    }

    let value = read_json(&path)?;
    let registries = value
        .as_object()
        .with_context(|| format!("注册表报告根节点不是对象: {}", path.display()))?;
    let mut maps = HashMap::new();

    for (registry, value) in registries {
        let Some(entries) = value.get("entries").and_then(Value::as_object) else {
            continue;
        };

        let mut id_by_name = HashMap::new();
        for (entry, value) in entries {
            let Some(id) = value.get("protocol_id").and_then(Value::as_i64) else {
                continue;
            };

            if let Ok(id) = i32::try_from(id) {
                id_by_name.insert(normalize_identifier(entry), id);
            }
        }

        if !id_by_name.is_empty() {
            maps.insert(registry.clone(), id_by_name);
        }
    }

    Ok(maps)
}
