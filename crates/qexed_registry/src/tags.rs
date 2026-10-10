//! tags 包构造（数据源：qexed_mojang_data 缓存的 data/minecraft/tags）。

use std::collections::{HashMap, HashSet};

use qexed_packet::net_types::VarInt;
use qexed_protocol::to_client::configuration::tags::{Tag as NetworkTag, Tags, Tags2};
use serde_json::Value;

use crate::error::RegistryError;

use super::{
    STATIC_TAG_REGISTRIES, SYNCHRONIZED_REGISTRIES,
    registries::{load_dynamic_registry_id_map, load_static_registry_id_maps},
    util::{json_files, normalize_identifier, read_json, tag_name_from_path},
};

const DAMAGE_TYPE_IS_FIRE: &[&str] = &[
    "minecraft:in_fire",
    "minecraft:campfire",
    "minecraft:on_fire",
    "minecraft:lava",
    "minecraft:hot_floor",
    "minecraft:unattributed_fireball",
    "minecraft:fireball",
];

pub fn load_tag_packet() -> Result<Tags, RegistryError> {
    let data_root = qexed_mojang_data::data_dir()?;
    let static_id_maps = load_static_registry_id_maps()?;
    let mut registries = Vec::new();

    for registry in SYNCHRONIZED_REGISTRIES {
        let registry_dir = data_root.join(registry);
        let id_by_name = load_dynamic_registry_id_map(&registry_dir, registry)?;
        let tags_dir = data_root.join("tags").join(registry);
        let mut tags = if tags_dir.exists() {
            load_registry_tags(&tags_dir, &id_by_name)?
        } else {
            Vec::new()
        };
        if *registry == "damage_type" {
            ensure_required_damage_type_tags(&mut tags, &id_by_name)?;
        }
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
        let registry_tags_dir = data_root.join("tags").join(registry);
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
) -> Result<Vec<NetworkTag>, RegistryError> {
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

fn ensure_required_damage_type_tags(
    tags: &mut Vec<NetworkTag>,
    id_by_name: &HashMap<String, i32>,
) -> Result<(), RegistryError> {
    if tags.iter().any(|tag| tag.name == "minecraft:is_fire") {
        return Ok(());
    }

    let entries = DAMAGE_TYPE_IS_FIRE
        .iter()
        .filter_map(|name| id_by_name.get(*name).copied())
        .map(VarInt)
        .collect::<Vec<_>>();
    if entries.is_empty() {
        return Err(RegistryError::DamageTypeFireTagMissing);
    }

    tags.push(NetworkTag {
        name: "minecraft:is_fire".to_string(),
        entries,
    });
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum TagValue {
    Entry(String),
    Tag(String),
}

fn load_tag_definitions(
    tags_dir: &std::path::Path,
) -> Result<HashMap<String, Vec<TagValue>>, RegistryError> {
    let mut definitions = HashMap::new();

    for path in json_files(tags_dir)? {
        let name = tag_name_from_path(tags_dir, &path)?;
        let value = read_json(&path)?;
        let values = value
            .get("values")
            .and_then(Value::as_array)
            .ok_or_else(|| RegistryError::TagMissingValues(path.display().to_string()))?;

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
) -> Result<Vec<i32>, RegistryError> {
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
) -> Result<(), RegistryError> {
    if !visiting.insert(tag_name.to_string()) {
        return Err(RegistryError::TagCycle(tag_name.to_string()));
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{NetworkTag, ensure_required_damage_type_tags};

    #[test]
    fn required_damage_type_fire_tag_is_injected_when_assets_are_incomplete() {
        let mut id_by_name = HashMap::new();
        id_by_name.insert("minecraft:in_fire".to_string(), 1);
        id_by_name.insert("minecraft:on_fire".to_string(), 2);
        let mut tags = Vec::new();

        ensure_required_damage_type_tags(&mut tags, &id_by_name).unwrap();

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "minecraft:is_fire");
        assert_eq!(
            tags[0]
                .entries
                .iter()
                .map(|entry| entry.0)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
    }

    #[test]
    fn required_damage_type_fire_tag_is_not_duplicated() {
        let mut tags = vec![NetworkTag {
            name: "minecraft:is_fire".to_string(),
            entries: Vec::new(),
        }];

        ensure_required_damage_type_tags(&mut tags, &HashMap::new()).unwrap();

        assert_eq!(tags.len(), 1);
    }
}