use std::path::Path;

use anyhow::{Context as _, Result};
use serde::Deserialize;

pub(crate) fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read Mojang cache JSON: {}", path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("failed to parse Mojang cache JSON: {}", path.display()))
}

pub(crate) fn column_index(local_x: i32, local_z: i32) -> usize {
    (local_z * 16 + local_x) as usize
}

pub(crate) fn normalize_identifier(value: &str) -> String {
    if value.contains(':') {
        value.to_string()
    } else {
        format!("minecraft:{value}")
    }
}

pub(crate) fn normalize_dimension(value: &str) -> String {
    match normalize_identifier(value).as_str() {
        "minecraft:overworld" | "minecraft:the_nether" | "minecraft:the_end" => {
            normalize_identifier(value)
        }
        "minecraft:nether" => "minecraft:the_nether".to_string(),
        "minecraft:end" => "minecraft:the_end".to_string(),
        other => other.to_string(),
    }
}

pub(crate) fn ceil_log2(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        usize::BITS as usize - (value - 1).leading_zeros() as usize
    }
}

pub(crate) fn smoothstep(value: f64) -> f64 {
    value * value * (3.0 - 2.0 * value)
}

pub(crate) fn lerp(left: f64, right: f64, t: f64) -> f64 {
    left + (right - left) * t
}

pub(crate) fn default_air_name() -> String {
    "minecraft:air".to_string()
}
