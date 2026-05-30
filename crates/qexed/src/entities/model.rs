pub(super) fn configured_entity_key(
    index: usize,
    config: &qexed_config::app::qexed::server::Entity,
) -> String {
    let id = config.id.trim();
    if id.is_empty() {
        format!("entity-{index}")
    } else {
        id.to_string()
    }
}

pub(super) fn stable_entity_uuid(key: &str) -> uuid::Uuid {
    uuid::Uuid::new_v3(
        &uuid::Uuid::NAMESPACE_OID,
        format!("qexed:entity:{key}").as_bytes(),
    )
}
