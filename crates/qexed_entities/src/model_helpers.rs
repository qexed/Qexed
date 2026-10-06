//! 配置实体 key / 稳定 UUID（v4 entities/model.rs 迁移）。

use crate::config::Entity;

pub(super) fn configured_entity_key(index: usize, config: &Entity) -> String {
    let id = config.id.trim();
    if id.is_empty() {
        format!("entity-{index}")
    } else {
        id.to_string()
    }
}

pub(crate) fn stable_entity_uuid(key: &str) -> uuid::Uuid {
    uuid::Uuid::new_v3(
        &uuid::Uuid::NAMESPACE_OID,
        format!("qexed:entity:{key}").as_bytes(),
    )
}
