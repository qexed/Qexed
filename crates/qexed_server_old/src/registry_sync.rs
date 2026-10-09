//! 注册表同步：从本地 vanilla 数据构造 configuration 阶段的 registry/tags 包。
//! 移植自 v4 的 registry_sync（数据源改为 v6 的 assets + mojang_data 缓存）。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use qexed_nbt::{ListHeader, Tag, tag_id};
use qexed_protocol::to_client::configuration::registry_data::{Entries, RegistryData};
use serde_json::Value;

pub const VANILLA_FEATURE: &str = "minecraft:vanilla";

const DECOMPILED_ROOT: &str = "assets/decompiled_source/src/data/minecraft";
const REGISTRIES_REPORT: &str = "assets/reports/registries.json";

/// 需要在 configuration 阶段同步的动态注册表。
pub const SYNCHRONIZED_REGISTRIES: &[&str] = &[
    "worldgen/biome", "chat_type", "trim_pattern", "trim_material",
    "wolf_variant", "wolf_sound_variant", "pig_variant", "pig_sound_variant",
    "frog_variant", "cat_variant", "cat_sound_variant", "cow_variant",
    "cow_sound_variant", "chicken_variant", "chicken_sound_variant",
    "painting_variant", "dimension_type", "damage_type", "banner_pattern",
    "enchantment", "jukebox_song", "instrument", "dialog",
];

/// 数据根（相对运行目录）。
fn data_roots() -> Vec<PathBuf> {
    vec![PathBuf::from(DECOMPILED_ROOT)]
}

/// 加载全部同步注册表为 registry_data 包。
pub fn load_registry_packets(include_contents: bool) -> anyhow::Result<Vec<RegistryData>> {
    let roots = data_roots();
    let mut packets = Vec::new();
    for registry in SYNCHRONIZED_REGISTRIES {
        let Some(dir) = roots.iter().map(|r| r.join(registry)).find(|p| p.exists()) else {
            continue;
        };
        let mut entries = Vec::new();
        for path in json_files(&dir)? {
            let entry_id = entry_id_from_path(&dir, &path, registry)?;
            let data = if include_contents {
                Some(json_to_nbt(&read_json(&path)?)?)
            } else {
                None
            };
            entries.push(Entries { entry_id, data });
        }
        if entries.is_empty() { continue }
        entries.sort_by(|a, b| a.entry_id.cmp(&b.entry_id));
        packets.push(RegistryData {
            id: format!("minecraft:{registry}"),
            entries,
        });
    }
    Ok(packets)
}

// ---------------------------------------------------------------------------
// JSON -> NBT / 文件枚举（v4 util 的移植）
// ---------------------------------------------------------------------------

pub(crate) fn json_files(root: &std::path::Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_json_files(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_json_files(dir: &std::path::Path, files: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_json_files(&path, files)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("json") {
            files.push(path);
        }
    }
    Ok(())
}

fn entry_id_from_path(root: &std::path::Path, path: &std::path::Path, registry: &str) -> anyhow::Result<String> {
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

fn read_json(path: &std::path::Path) -> anyhow::Result<Value> {
    let content = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&content)?)
}

pub(crate) fn json_to_nbt(value: &Value) -> anyhow::Result<Tag> {
    match value {
        Value::Null => Ok(Tag::End),
        Value::Bool(v) => Ok(Tag::Byte(i8::from(*v))),
        Value::Number(v) => number_to_nbt(v),
        Value::String(v) => Ok(Tag::String(Arc::from(v.as_str()))),
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

fn number_to_nbt(value: &serde_json::Number) -> anyhow::Result<Tag> {
    if let Some(v) = value.as_i64() {
        if (i32::MIN as i64..=i32::MAX as i64).contains(&v) {
            Ok(Tag::Int(v as i32))
        } else {
            Ok(Tag::Long(v))
        }
    } else if let Some(v) = value.as_u64() {
        if v <= i32::MAX as u64 {
            Ok(Tag::Int(v as i32))
        } else {
            Ok(Tag::Long(v as i64))
        }
    } else if let Some(v) = value.as_f64() {
        Ok(Tag::Float(v as f32))
    } else {
        anyhow::bail!("unsupported JSON number: {value}")
    }
}

fn array_to_nbt(values: &[Value]) -> anyhow::Result<Tag> {
    if values.is_empty() {
        return Ok(Tag::List(ListHeader { tag_id: tag_id::END, length: 0 }, Arc::from([])));
    }
    let items: Vec<Tag> = values.iter().map(json_to_nbt).collect::<Result<Vec<_>, _>>()?;
    let tag_id = items[0].tag_id();
    if items.iter().all(|item| item.tag_id() == tag_id) {
        return Ok(Tag::List(ListHeader { tag_id, length: items.len() as i32 }, Arc::from(items)));
    }
    let wrapped = items
        .into_iter()
        .map(|item| {
            let mut map = HashMap::new();
            map.insert("value".to_string(), item);
            Tag::Compound(Arc::new(map))
        })
        .collect::<Vec<_>>();
    Ok(Tag::List(ListHeader { tag_id: tag_id::COMPOUND, length: wrapped.len() as i32 }, Arc::from(wrapped)))
}

/// 静态注册表 ID 映射（从 registries.json 报告）。
pub fn load_registry_id_map(registry_id: &str) -> anyhow::Result<HashMap<String, i32>> {
    let path = PathBuf::from(REGISTRIES_REPORT);
    let value: Value = if path.exists() {
        read_json(&path)?
    } else {
        serde_json::from_str(include_str!("../../../assets/reports/registries.json"))?
    };
    let entries = value
        .get(registry_id)
        .and_then(|r| r.get("entries"))
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow::anyhow!("registry not found: {registry_id}"))?;
    Ok(entries
        .iter()
        .filter_map(|(entry, v)| {
            v.get("protocol_id")
                .and_then(Value::as_i64)
                .and_then(|id| i32::try_from(id).ok())
                .map(|id| (normalize_identifier(entry), id))
        })
        .collect())
}

pub(crate) fn normalize_identifier(value: &str) -> String {
    if value.contains(':') {
        value.to_string()
    } else {
        format!("minecraft:{value}")
    }
}
