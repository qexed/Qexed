mod event;
mod payload;
mod placeholders;

pub use event::PluginEvent;
pub use payload::{
    BlockDropItem, BlockDropPosition, BlockDropQuery, BlockDropResponse, BlockStepPayload,
    BlockStepPosition, ChunkPayload, ClickDetectedPayload, ConfigReloadPayload,
    CustomEntityDefinition, CustomEntityRegistryResponse, EntityAiEntityPayload, EntityAiOperation,
    EntityAiPlayerPayload, EntityAiTickQuery, EntityAiTickResponse, ItemEnchantment,
    LanguagePayload, MiningSpeedQuery, MiningSpeedResponse, NpcEntityPayload, NpcInteractPayload,
    NpcMutationOp, NpcMutationQuery, NpcMutationResponse, NpcUpsert, PathfindingQuery,
    PathfindingResponse, PlaceholderContext, PlaceholderQuery, PlaceholderReplacement,
    PlaceholderResponse, PlayerAction, PlayerInputPayload, PlayerInputState, PlayerItemPickupQuery,
    PlayerItemPickupResponse, PlayerMovePayload, PlayerPayload, PlayerPayloadOwned,
    PlayerPositionPayload, PluginCommandDefinition, PluginCommandQuery, PluginCommandResponse,
    PluginEnchantment, ProxyConnectResultPayload,
};
pub use placeholders::{
    NATIVE_PLACEHOLDER_DOCS, PlaceholderDoc, PlaceholderScope, native_placeholder_docs,
};

#[cfg(feature = "server")]
pub use payload::{
    player_input_state, player_payload, player_payload_owned, player_position_payload,
};
