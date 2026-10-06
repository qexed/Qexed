#[derive(Debug, thiserror::Error)]
pub enum EntitiesError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Config Error: {0}")]
    ConfigError(#[from] qexed_config::error::ConfigError),
    #[error("packet error: {0}")]
    Packet(#[from] qexed_packet::PacketError),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("missing entity type registry id: {entity_type}")]
    MissingEntityTypeRegistryId { entity_type: String },
    #[error("entity id cannot be empty")]
    EmptyEntityId,
    #[error("duplicate entity id: {0}")]
    DuplicateEntityId(String),
    #[error("duplicate configured entity id: {0}")]
    DuplicateConfiguredEntityId(String),
    #[error("entity not found: {0}")]
    EntityNotFound(String),
    #[error("unsupported projectile kind: {0}")]
    UnsupportedProjectileKind(String),
    #[error("projectile velocity must be non-zero")]
    ZeroProjectileVelocity,
    #[error("npc skin lookup unavailable in this build: {player_id}")]
    NpcSkinLookupUnavailable { player_id: String },
    /// NPC 皮肤查询的 HTTP 层错误（reqwest）。
    #[error("npc skin http error: {0}")]
    NpcSkinHttp(#[from] reqwest::Error),
}
