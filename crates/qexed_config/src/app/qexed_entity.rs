use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};

use crate::app::qexed::server::{Entities, Entity, EntityKind, EntitySpawning};

#[derive(Debug, Clone, Serialize, Deserialize, AutoDoc)]
pub struct QexedEntityConfig {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.enable")]
    pub enable: bool,

    #[serde(default = "default_entities_dimension")]
    #[AutoDoc(key = "config.qexed.server.entities.dimension")]
    pub dimension: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[AutoDoc(key = "config.qexed.server.entities.list", sub)]
    pub list: Vec<QexedEntityEntry>,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.spawning", sub)]
    pub spawning: EntitySpawning,
}

impl Default for QexedEntityConfig {
    fn default() -> Self {
        Self {
            enable: false,
            dimension: default_entities_dimension(),
            list: Vec::new(),
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
            spawning: config.spawning,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, AutoDoc, PartialEq)]
pub struct QexedEntityEntry {
    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.id")]
    pub id: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.kind")]
    pub kind: QexedEntityKind,

    #[serde(default = "default_entity_type")]
    #[AutoDoc(key = "config.qexed.server.entities.list.entity_type")]
    pub entity_type: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.name")]
    pub name: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.display_name")]
    pub display_name: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.x")]
    pub x: f64,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.y")]
    pub y: f64,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.z")]
    pub z: f64,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.yaw")]
    pub yaw: f32,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.pitch")]
    pub pitch: f32,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.on_ground")]
    pub on_ground: bool,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.data")]
    pub data: i32,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.ai")]
    pub ai: String,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.ai_params")]
    pub ai_params: std::collections::BTreeMap<String, serde_json::Value>,

    #[serde(default)]
    #[AutoDoc(key = "config.qexed.server.entities.list.auto_jump")]
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
            auto_jump: false,
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

#[derive(Debug, Clone, Serialize, Deserialize, AutoDoc)]
pub struct QexedEntity {
    #[AutoDoc(key = "config.qexed.server.entities", sub)]
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

    fn config_file_description(lang: &str, config_file: &str, _root_path: Option<&str>) -> String {
        rust_i18n::t!(
            "autodoc.file_description.qexed.entities",
            locale = lang,
            file = config_file
        )
        .to_string()
    }
}
