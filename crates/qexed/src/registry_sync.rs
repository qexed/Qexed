use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};

use anyhow::{Context, Result};
use qexed_nbt::{ListHeader, Tag, tag_id};
use qexed_packet::net_types::VarInt;
use qexed_protocol::{
    to_client::configuration::{
        registry_data::{Entries, RegistryData},
        tags::{Tag as NetworkTag, Tags, Tags2},
    },
    types::KnownPacks,
};
use serde_json::Value;

pub const VANILLA_FEATURE: &str = "minecraft:vanilla";

const DATA_ROOT: &str = "assets/decompiled_source/src/data/minecraft";
const REGISTRIES_REPORT: &str = "assets/reports/registries.json";

const STATIC_TAG_REGISTRIES: &[&str] = &[
    "block",
    "entity_type",
    "fluid",
    "game_event",
    "item",
    "point_of_interest_type",
    "potion",
];

const SYNCHRONIZED_REGISTRIES: &[&str] = &[
    "worldgen/biome",
    "chat_type",
    "trim_pattern",
    "trim_material",
    "wolf_variant",
    "wolf_sound_variant",
    "pig_variant",
    "pig_sound_variant",
    "frog_variant",
    "cat_variant",
    "cat_sound_variant",
    "cow_sound_variant",
    "cow_variant",
    "chicken_sound_variant",
    "chicken_variant",
    "zombie_nautilus_variant",
    "painting_variant",
    "dimension_type",
    "damage_type",
    "banner_pattern",
    "enchantment",
    "jukebox_song",
    "instrument",
    "test_environment",
    "test_instance",
    "dialog",
    "world_clock",
    "timeline",
];

pub fn known_packs() -> Vec<KnownPacks> {
    vec![KnownPacks {
        namespace: "minecraft".to_string(),
        id: "core".to_string(),
        version: qexed_config::MC_VERSION.to_string(),
    }]
}

pub fn accepts_vanilla_core_pack(packs: &[KnownPacks]) -> bool {
    let known = known_packs();
    packs.iter().any(|pack| known.contains(pack))
}

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

fn load_dynamic_registry_id_map(
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

fn load_static_registry_id_maps() -> Result<HashMap<String, HashMap<String, i32>>> {
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
                    if let Some(id) = id_by_name.get(name).copied() {
                        if seen_entries.insert(id) {
                            entries.push(id);
                        }
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

fn json_files(root: &std::path::Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_json_files(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_json_files(dir: &std::path::Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in
        std::fs::read_dir(dir).with_context(|| format!("无法读取目录 {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_json_files(&path, files)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            files.push(path);
        }
    }
    Ok(())
}

fn entry_id_from_path(
    root: &std::path::Path,
    path: &std::path::Path,
    registry: &str,
) -> Result<String> {
    let relative = path.strip_prefix(root)?;
    let mut id = relative
        .with_extension("")
        .to_string_lossy()
        .replace('\\', "/");
    if id.contains(':') {
        return Ok(id);
    }

    if registry.contains('/') {
        id = id.trim_start_matches('/').to_string();
    }
    Ok(format!("minecraft:{id}"))
}

fn tag_name_from_path(root: &std::path::Path, path: &std::path::Path) -> Result<String> {
    let relative = path.strip_prefix(root)?;
    let id = relative
        .with_extension("")
        .to_string_lossy()
        .replace('\\', "/");
    Ok(format!("minecraft:{id}"))
}

fn read_json(path: &std::path::Path) -> Result<Value> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("无法读取 JSON: {}", path.display()))?;
    serde_json::from_str(&content).with_context(|| format!("JSON 格式错误: {}", path.display()))
}

fn json_to_nbt(value: &Value) -> Result<Tag> {
    match value {
        Value::Null => Ok(Tag::End),
        Value::Bool(value) => Ok(Tag::Byte(i8::from(*value))),
        Value::Number(value) => number_to_nbt(value),
        Value::String(value) => Ok(Tag::String(Arc::from(value.as_str()))),
        Value::Array(values) => array_to_nbt(values),
        Value::Object(values) => {
            let mut map = HashMap::new();
            for (key, value) in values {
                map.insert(key.clone(), json_to_nbt(value)?);
            }
            Ok(Tag::Compound(Arc::new(map)))
        }
    }
}

fn number_to_nbt(value: &serde_json::Number) -> Result<Tag> {
    if let Some(value) = value.as_i64() {
        if (i32::MIN as i64..=i32::MAX as i64).contains(&value) {
            Ok(Tag::Int(value as i32))
        } else {
            Ok(Tag::Long(value))
        }
    } else if let Some(value) = value.as_u64() {
        if value <= i32::MAX as u64 {
            Ok(Tag::Int(value as i32))
        } else if value <= i64::MAX as u64 {
            Ok(Tag::Long(value as i64))
        } else {
            anyhow::bail!("JSON 数字超出 NBT long 范围: {value}")
        }
    } else if let Some(value) = value.as_f64() {
        Ok(Tag::Double(value))
    } else {
        anyhow::bail!("不支持的 JSON 数字: {value}")
    }
}

fn array_to_nbt(values: &[Value]) -> Result<Tag> {
    if values.is_empty() {
        return Ok(Tag::List(
            ListHeader {
                tag_id: tag_id::END,
                length: 0,
            },
            Arc::from([]),
        ));
    }

    let items: Vec<Tag> = values.iter().map(json_to_nbt).collect::<Result<Vec<_>>>()?;
    let tag_id = items[0].tag_id();

    if items.iter().all(|item| item.tag_id() == tag_id) {
        return Ok(Tag::List(
            ListHeader {
                tag_id,
                length: items.len() as i32,
            },
            Arc::from(items),
        ));
    }

    let wrapped = items
        .into_iter()
        .map(|item| {
            let mut map = HashMap::new();
            map.insert("value".to_string(), item);
            Tag::Compound(Arc::new(map))
        })
        .collect::<Vec<_>>();

    Ok(Tag::List(
        ListHeader {
            tag_id: tag_id::COMPOUND,
            length: wrapped.len() as i32,
        },
        Arc::from(wrapped),
    ))
}

fn normalize_identifier(value: &str) -> String {
    if value.contains(':') {
        value.to_string()
    } else {
        format!("minecraft:{value}")
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::{
        STATIC_TAG_REGISTRIES, VANILLA_FEATURE, accepts_vanilla_core_pack, known_packs,
        load_registry_packets, load_tag_packet,
    };

    #[test]
    fn loads_vanilla_registry_packets_from_assets() {
        let packets = load_registry_packets(true).unwrap();
        assert!(
            packets
                .iter()
                .any(|packet| packet.id == "minecraft:worldgen/biome")
        );
        assert!(
            packets
                .iter()
                .any(|packet| packet.id == "minecraft:dimension_type")
        );
    }

    #[test]
    fn can_skip_vanilla_registry_contents_for_known_pack_clients() {
        let packets = load_registry_packets(false).unwrap();
        let biome = packets
            .iter()
            .find(|packet| packet.id == "minecraft:worldgen/biome")
            .unwrap();

        assert!(!biome.entries.is_empty());
        assert!(biome.entries.iter().all(|entry| entry.data.is_none()));
    }

    #[test]
    fn loads_tags_from_assets() {
        let packet = load_tag_packet().unwrap();
        assert!(
            packet
                .tags
                .iter()
                .any(|tags| tags.registry == "minecraft:damage_type")
        );
        assert!(STATIC_TAG_REGISTRIES.iter().any(|registry| {
            packet
                .tags
                .iter()
                .any(|tags| tags.registry == format!("minecraft:{registry}"))
        }));
    }

    #[test]
    fn known_pack_matches_vanilla_core_pack() {
        let packs = known_packs();
        assert_eq!(VANILLA_FEATURE, "minecraft:vanilla");
        assert_eq!(packs[0].namespace, "minecraft");
        assert_eq!(packs[0].id, "core");
        assert!(accepts_vanilla_core_pack(&packs));
    }
}
