use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use qexed_packet::net_types::VarInt;
use qexed_protocol::to_client::configuration::tags::{Tag as NetworkTag, Tags, Tags2};
use serde_json::Value;

use super::{
    DATA_ROOT, STATIC_TAG_REGISTRIES, SYNCHRONIZED_REGISTRIES,
    registries::{load_dynamic_registry_id_map, load_static_registry_id_maps},
    util::{json_files, normalize_identifier, read_json, tag_name_from_path, workspace_root},
};

pub fn load_tag_packet() -> Result<Tags> {
    let data_root = workspace_root().join(DATA_ROOT);
    let tags_root = data_root.join("tags");
    let static_id_maps = load_static_registry_id_maps()?;
    let mut registries = Vec::new();

    if !tags_root.exists() {
        return Ok(Tags { tags: registries });
    }

    for registry in SYNCHRONIZED_REGISTRIES {
        let registry_tags_dir = tags_root.join(registry);
        if !registry_tags_dir.exists() {
            continue;
        }

        let id_by_name = load_dynamic_registry_id_map(&data_root.join(registry), registry)?;
        let mut tags = load_registry_tags(&registry_tags_dir, &id_by_name)?;
        if tags.is_empty() {
            continue;
        }

        tags.sort_by(|a, b| a.name.cmp(&b.name));
        registries.push(Tags2 {
            registry: format!("minecraft:{registry}"),
            tags,
        });
    }

    for registry in STATIC_TAG_REGISTRIES {
        let registry_tags_dir = tags_root.join(registry);
        if !registry_tags_dir.exists() {
            continue;
        }

        let registry_id = format!("minecraft:{registry}");
        let Some(id_by_name) = static_id_maps.get(&registry_id) else {
            log::debug!("跳过缺少协议 ID 映射的静态标签注册表: {registry_id}");
            continue;
        };

        let mut tags = load_registry_tags(&registry_tags_dir, id_by_name)?;
        if tags.is_empty() {
            continue;
        }

        tags.sort_by(|a, b| a.name.cmp(&b.name));
        registries.push(Tags2 {
            registry: registry_id,
            tags,
        });
    }

    registries.sort_by(|a, b| a.registry.cmp(&b.registry));
    Ok(Tags { tags: registries })
}

fn load_registry_tags(
    tags_dir: &std::path::Path,
    id_by_name: &HashMap<String, i32>,
) -> Result<Vec<NetworkTag>> {
    let definitions = load_tag_definitions(tags_dir)?;
    let mut tags = Vec::new();

    for name in definitions.keys() {
        let entries = resolve_tag_entries(name, &definitions, id_by_name)?
            .into_iter()
            .map(VarInt)
            .collect();

        tags.push(NetworkTag {
            name: name.clone(),
            entries,
        });
    }

    Ok(tags)
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum TagValue {
    Entry(String),
    Tag(String),
}

fn load_tag_definitions(tags_dir: &std::path::Path) -> Result<HashMap<String, Vec<TagValue>>> {
    let mut definitions = HashMap::new();

    for path in json_files(tags_dir)? {
        let name = tag_name_from_path(tags_dir, &path)?;
        let value = read_json(&path)?;
        let values = value
            .get("values")
            .and_then(Value::as_array)
            .with_context(|| format!("标签文件缺少 values 数组: {}", path.display()))?;

        let entries = values
            .iter()
            .filter_map(tag_value_from_json)
            .collect::<Vec<_>>();
        definitions.insert(name, entries);
    }

    Ok(definitions)
}

fn tag_value_from_json(value: &Value) -> Option<TagValue> {
    match value {
        Value::String(name) => Some(tag_value_from_identifier(name)),
        Value::Object(map) => map
            .get("id")
            .and_then(Value::as_str)
            .map(tag_value_from_identifier),
        _ => None,
    }
}

fn tag_value_from_identifier(value: &str) -> TagValue {
    if let Some(tag) = value.strip_prefix('#') {
        TagValue::Tag(normalize_identifier(tag))
    } else {
        TagValue::Entry(normalize_identifier(value))
    }
}

fn resolve_tag_entries(
    tag_name: &str,
    definitions: &HashMap<String, Vec<TagValue>>,
    id_by_name: &HashMap<String, i32>,
) -> Result<Vec<i32>> {
    let mut entries = Vec::new();
    let mut seen_entries = HashSet::new();
    let mut visiting = HashSet::new();
    resolve_tag_entries_inner(
        tag_name,
        definitions,
        id_by_name,
        &mut visiting,
        &mut seen_entries,
        &mut entries,
    )?;
    Ok(entries)
}

fn resolve_tag_entries_inner(
    tag_name: &str,
    definitions: &HashMap<String, Vec<TagValue>>,
    id_by_name: &HashMap<String, i32>,
    visiting: &mut HashSet<String>,
    seen_entries: &mut HashSet<i32>,
    entries: &mut Vec<i32>,
) -> Result<()> {
    if !visiting.insert(tag_name.to_string()) {
        anyhow::bail!("标签引用形成循环: {tag_name}");
    }

    if let Some(values) = definitions.get(tag_name) {
        for value in values {
            match value {
                TagValue::Entry(name) => {
                    if let Some(id) = id_by_name.get(name).copied()
                        && seen_entries.insert(id)
                    {
                        entries.push(id);
                    }
                }
                TagValue::Tag(name) => resolve_tag_entries_inner(
                    name,
                    definitions,
                    id_by_name,
                    visiting,
                    seen_entries,
                    entries,
                )?,
            }
        }
    }

    visiting.remove(tag_name);
    Ok(())
}
