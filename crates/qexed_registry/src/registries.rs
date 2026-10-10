//! 动态注册表包构造 + 静态注册表 ID 映射（数据源：qexed_mojang_data 缓存）。

use std::collections::HashMap;

use qexed_protocol::to_client::configuration::registry_data::{Entries, RegistryData};
use serde_json::Value;

use crate::error::RegistryError;

use super::{
    SYNCHRONIZED_REGISTRIES,
    util::{
        entry_id_from_path, json_files, json_to_nbt, normalize_identifier, read_json,
    },
};

pub fn load_registry_packets(include_contents: bool) -> Result<Vec<RegistryData>, RegistryError> {
    let data_root = qexed_mojang_data::data_dir()?;
    let mut packets = Vec::new();

    for registry in SYNCHRONIZED_REGISTRIES {
        let registry_dir = data_root.join(registry);
        if !registry_dir.exists() {
            log::debug!("跳过缺失的同步注册表目录: {registry}");
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
) -> Result<Vec<Entries>, RegistryError> {
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

pub fn load_dynamic_registry_id_map(
    registry_dir: &std::path::Path,
    registry: &str,
) -> Result<HashMap<String, i32>, RegistryError> {
    let mut ids = HashMap::new();
    let mut entries = Vec::new();

    let registry_dir = if registry_dir.exists() {
        registry_dir.to_path_buf()
    } else {
        let data_root =
            qexed_mojang_data::data_dir()?;
        let dir = data_root.join(registry);
        if !dir.exists() {
            return Ok(ids);
        }
        dir
    };

    for path in json_files(&registry_dir)? {
        entries.push(entry_id_from_path(&registry_dir, &path, registry)?);
    }
    entries.sort();

    for (index, entry) in entries.into_iter().enumerate() {
        ids.insert(entry, index as i32);
    }

    Ok(ids)
}

pub(super) fn load_static_registry_id_maps(
) -> Result<HashMap<String, HashMap<String, i32>>, RegistryError> {
    let value = load_registry_report()?;
    let registries = value
        .as_object()
        .ok_or(RegistryError::RegistryReportNotObject)?;
    let mut maps = HashMap::new();

    for (registry, value) in registries {
        let Some(entries) = value.get("entries").and_then(Value::as_object) else {
            continue;
        };

        let id_by_name = registry_id_map_from_entries(entries);
        if !id_by_name.is_empty() {
            maps.insert(registry.clone(), id_by_name);
        }
    }

    Ok(maps)
}

pub fn load_registry_id_map(registry_id: &str) -> Result<HashMap<String, i32>, RegistryError> {
    let value = load_registry_report()?;
    let entries = value
        .get(registry_id)
        .and_then(|registry| registry.get("entries"))
        .and_then(Value::as_object)
        .ok_or_else(|| RegistryError::RegistryNotFound(registry_id.to_string()))?;
    Ok(registry_id_map_from_entries(entries))
}

pub fn load_blocks_report() -> Result<Value, RegistryError> {
    let reports_dir =
        qexed_mojang_data::reports_dir()?;
    read_json(&reports_dir.join("blocks.json"))
}

fn load_registry_report() -> Result<Value, RegistryError> {
    let reports_dir =
        qexed_mojang_data::reports_dir()?;
    read_json(&reports_dir.join("registries.json"))
}

fn registry_id_map_from_entries(entries: &serde_json::Map<String, Value>) -> HashMap<String, i32> {
    entries
        .iter()
        .filter_map(|(entry, value)| {
            value
                .get("protocol_id")
                .and_then(Value::as_i64)
                .and_then(|id| i32::try_from(id).ok())
                .map(|id| (normalize_identifier(entry), id))
        })
        .collect()
}