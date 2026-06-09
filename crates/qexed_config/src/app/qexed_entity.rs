use serde::{Deserialize, Serialize};

use crate::app::qexed::server::{Entities, Entity, EntityAiOverride, EntityKind, EntitySpawning};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QexedEntityConfig {
    #[serde(default)]
    pub enable: bool,

    #[serde(default = "default_entities_dimension")]
    pub dimension: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub list: Vec<QexedEntityEntry>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub disabled_entity_types: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ai_overrides: Vec<EntityAiOverride>,

    #[serde(default)]
    pub spawning: EntitySpawning,
}

impl Default for QexedEntityConfig {
    fn default() -> Self {
        Self {
            enable: false,
            dimension: default_entities_dimension(),
            list: Vec::new(),
            disabled_entity_types: Vec::new(),
            ai_overrides: Vec::new(),
            spawning: EntitySpawning::default(),
        }
    }
}

impl From<QexedEntityConfig> for Entities {
    fn from(config: QexedEntityConfig) -> Self {
        Self {
            enable: config.enable,
            dimension: config.dimension,
            list: config.list.into_iter().map(Into::into).collect(),
            disabled_entity_types: config.disabled_entity_types,
            ai_overrides: config.ai_overrides,
            spawning: config.spawning,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QexedEntityEntry {
    #[serde(default)]
    pub id: String,

    #[serde(default)]
    pub kind: QexedEntityKind,

    #[serde(default = "default_entity_type")]
    pub entity_type: String,

    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub display_name: String,

    #[serde(default)]
    pub x: f64,

    #[serde(default)]
    pub y: f64,

    #[serde(default)]
    pub z: f64,

    #[serde(default)]
    pub yaw: f32,

    #[serde(default)]
    pub pitch: f32,

    #[serde(default)]
    pub on_ground: bool,

    #[serde(default)]
    pub data: i32,

    #[serde(default)]
    pub ai: String,

    #[serde(default)]
    pub ai_params: std::collections::BTreeMap<String, serde_json::Value>,

    #[serde(default = "default_entity_auto_jump")]
    pub auto_jump: bool,
}

impl Default for QexedEntityEntry {
    fn default() -> Self {
        Self {
            id: String::new(),
            kind: QexedEntityKind::default(),
            entity_type: default_entity_type(),
            name: String::new(),
            display_name: String::new(),
            x: 0.0,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
            data: 0,
            ai: String::new(),
            ai_params: std::collections::BTreeMap::new(),
            auto_jump: default_entity_auto_jump(),
        }
    }
}

impl From<QexedEntityEntry> for Entity {
    fn from(entry: QexedEntityEntry) -> Self {
        Entity {
            id: entry.id,
            kind: entry.kind.into(),
            entity_type: entry.entity_type,
            name: entry.name,
            display_name: entry.display_name,
            x: entry.x,
            y: entry.y,
            z: entry.z,
            yaw: entry.yaw,
            pitch: entry.pitch,
            on_ground: entry.on_ground,
            data: entry.data,
            ai: entry.ai,
            ai_params: entry.ai_params,
            auto_jump: entry.auto_jump,
            ..Entity::default()
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QexedEntityKind {
    #[default]
    Entity,
    Hologram,
}

impl From<QexedEntityKind> for EntityKind {
    fn from(kind: QexedEntityKind) -> Self {
        match kind {
            QexedEntityKind::Entity => EntityKind::Entity,
            QexedEntityKind::Hologram => EntityKind::Hologram,
        }
    }
}

fn default_entities_dimension() -> String {
    "minecraft:overworld".to_string()
}

fn default_entity_type() -> String {
    "minecraft:armor_stand".to_string()
}

fn default_entity_auto_jump() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QexedEntity {
    pub entities: QexedEntityConfig,
}

impl Default for QexedEntity {
    fn default() -> Self {
        Self {
            entities: QexedEntityConfig::default(),
        }
    }
}

impl qexed_config::tool::AppConfigTrait for QexedEntity {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed_entity";
}
