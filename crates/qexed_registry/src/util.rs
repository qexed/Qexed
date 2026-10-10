//! registry_sync 的文件/JSON/NBT 工具（v4 util 移植，workspace_root 换成 mojang 缓存）。

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use qexed_nbt::{ListHeader, Tag, tag_id};
use serde_json::Value;

use crate::error::RegistryError;

pub(super) fn json_files(root: &Path) -> Result<Vec<PathBuf>, RegistryError> {
    let mut files = Vec::new();
    collect_json_files(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_json_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), RegistryError> {
    let entries = std::fs::read_dir(dir).map_err(|source| RegistryError::ReadDir {
        path: dir.display().to_string(),
        source,
    })?;

    for entry in entries {
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

pub(super) fn entry_id_from_path(
    root: &Path,
    path: &Path,
    registry: &str,
) -> Result<String, RegistryError> {
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

pub(super) fn tag_name_from_path(root: &Path, path: &Path) -> Result<String, RegistryError> {
    let relative = path.strip_prefix(root)?;
    let id = relative
        .with_extension("")
        .to_string_lossy()
        .replace('\\', "/");
    Ok(format!("minecraft:{id}"))
}

pub(super) fn read_json(path: &Path) -> Result<Value, RegistryError> {
    let path_str = path.display().to_string();
    let content = std::fs::read_to_string(path).map_err(|source| RegistryError::ReadJsonFile {
        path: path_str.clone(),
        source,
    })?;
    serde_json::from_str(&content).map_err(|source| RegistryError::JsonSyntax {
        path: path_str,
        source,
    })
}

pub(super) fn json_to_nbt(value: &Value) -> Result<Tag, RegistryError> {
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

fn number_to_nbt(value: &serde_json::Number) -> Result<Tag, RegistryError> {
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
            Err(RegistryError::NbtNumberOverflow(value.to_string()))
        }
    } else if let Some(value) = value.as_f64() {
        let value = value as f32;
        if !value.is_finite() {
            return Err(RegistryError::NbtFloatNotFinite(value));
        }
        Ok(Tag::Float(value))
    } else {
        Err(RegistryError::UnsupportedJsonNumber(value.to_string()))
    }
}

fn array_to_nbt(values: &[Value]) -> Result<Tag, RegistryError> {
    if values.is_empty() {
        return Ok(Tag::List(
            ListHeader {
                tag_id: tag_id::END,
                length: 0,
            },
            Arc::from([]),
        ));
    }

    let items: Vec<Tag> = values
        .iter()
        .map(json_to_nbt)
        .collect::<Result<Vec<_>, _>>()?;
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

pub(super) fn normalize_identifier(value: &str) -> String {
    if value.contains(':') {
        value.to_string()
    } else {
        format!("minecraft:{value}")
    }
}