use std::collections::HashMap;

use anyhow::{Context, Result};
use qexed_protocol::to_client::configuration::registry_data::{Entries, RegistryData};
use serde_json::Value;

use super::{
    mojang,
    registry_sync::{BLOCKS_REPORT, REGISTRIES_REPORT, SYNCHRONIZED_REGISTRIES, data_roots},
    util::{entry_id_from_path, json_files, json_to_nbt, normalize_identifier, read_json},
};

pub fn load_registry_packets(include_contents: bool) -> Result<Vec<RegistryData>> {
    let data_roots = data_roots();
    let mut packets = Vec::new();

    for registry in SYNCHRONIZED_REGISTRIES {
        let Some(registry_dir) = first_existing_registry_dir(&data_roots, registry) else {
            continue;
        };

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

pub fn dimension_type_holder_id(dimension_type: &str) -> Result<i32> {
    Ok(dynamic_registry_entry_id("dimension_type", dimension_type)?.unwrap_or(0) + 1)
}

pub fn dynamic_registry_entry_id(registry: &str, entry: &str) -> Result<Option<i32>> {
    let ids = load_dynamic_registry_id_map(&std::path::PathBuf::new(), registry)?;
    Ok(ids.get(&normalize_identifier(entry)).copied())
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

pub fn load_dynamic_registry_id_map(
    registry_dir: &std::path::Path,
    registry: &str,
) -> Result<HashMap<String, i32>> {
    let mut ids = HashMap::new();
    let mut entries = Vec::new();

    let registry_dir = if registry_dir.exists() {
        registry_dir.to_path_buf()
    } else {
        let Some(registry_dir) = first_existing_registry_dir(&data_roots(), registry) else {
            return Ok(ids);
        };
        registry_dir
    };

    if !registry_dir.exists() {
        return Ok(ids);
    }

    for path in json_files(&registry_dir)? {
        entries.push(entry_id_from_path(&registry_dir, &path, registry)?);
    }
    entries.sort();

    for (index, entry) in entries.into_iter().enumerate() {
        ids.insert(entry, index as i32);
    }

    Ok(ids)
}

pub(super) fn load_static_registry_id_maps() -> Result<HashMap<String, HashMap<String, i32>>> {
    let value = load_registry_report()?;
    let registries = value
        .as_object()
        .with_context(|| "注册表报告根节点不是对象".to_string())?;
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

#[allow(dead_code)]
pub fn load_registry_id_map(registry_id: &str) -> Result<HashMap<String, i32>> {
    let value = load_registry_report()?;
    let entries = value
        .get(registry_id)
        .and_then(|registry| registry.get("entries"))
        .and_then(Value::as_object)
        .with_context(|| format!("registry not found in static registry report: {registry_id}"))?;
    Ok(registry_id_map_from_entries(entries))
}

#[allow(dead_code)]
pub fn load_blocks_report() -> Result<Value> {
    read_report_json(BLOCKS_REPORT)
}

fn load_registry_report() -> Result<Value> {
    read_report_json(REGISTRIES_REPORT)
}

fn read_report_json(name: &str) -> Result<Value> {
    let reports_root = mojang::reports_root()
        .with_context(|| "Mojang reports 尚未初始化，请先初始化 qexed_registry")?;
    let path = reports_root.join(name);
    read_json(&path)
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

fn first_existing_registry_dir(
    data_roots: &[std::path::PathBuf],
    registry: &str,
) -> Option<std::path::PathBuf> {
    data_roots
        .iter()
        .map(|root| root.join(registry))
        .find(|path| path.exists())
}
