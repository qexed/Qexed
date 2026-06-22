mod event;
mod payload;
mod placeholders;

pub use event::PluginEvent;
pub use payload::{
    AdvancementGrantQuery, AdvancementGrantResponse, BlockDropItem, BlockDropPosition,
    BlockDropQuery, BlockDropResponse, BlockStepPayload, BlockStepPosition, ChunkPayload,
    ClickDetectedPayload, ConfigReloadPayload, CraftItemQuery, CraftItemResponse,
    CraftingRecipeQuery, CraftingRecipeResponse, CustomEntityDefinition,
    CustomEntityRegistryResponse, EnchantingOption, EnchantingQuery, EnchantingResponse,
    EntityAiEntityPayload, EntityAiOperation, EntityAiPlayerPayload, EntityAiTickQuery,
    EntityAiTickResponse, FurnaceRecipeQuery, FurnaceRecipeResponse, FurnaceTickPayload,
    HttpHeader, HttpRequest, HttpResponse, ItemDurabilityQuery, ItemDurabilityResponse,
    ItemEnchantment, ItemStackPayload, LanguagePayload, LocalizedNameQuery, LocalizedNameResponse,
    MiningSpeedQuery, MiningSpeedResponse, NpcEntityPayload, NpcInteractPayload, NpcMutationOp,
    NpcMutationQuery, NpcMutationResponse, NpcUpsert, PathfindingQuery, PathfindingResponse,
    PlaceholderContext, PlaceholderQuery, PlaceholderReplacement, PlaceholderResponse,
    PlayerAction, PlayerAttackQuery, PlayerAttackResponse, PlayerBlockInteractPayload,
    PlayerDeathQuery, PlayerDeathResponse, PlayerInputPayload, PlayerInputState,
    PlayerItemPickupQuery, PlayerItemPickupResponse, PlayerMovePayload, PlayerOxygenTickQuery,
    PlayerOxygenTickResponse, PlayerPayload, PlayerPayloadOwned, PlayerPositionPayload,
    PlayerTickPayload, PlayerUseItemPayload, PluginApiCallQuery, PluginApiCallResponse,
    PluginCommandDefinition, PluginCommandQuery, PluginCommandResponse, PluginDependency,
    PluginEnchantment, PluginManifest, PluginServiceDefinition, PotionEffectTickQuery,
    PotionEffectTickResponse, ProjectileHitPlayerPayload, ProxyConnectResultPayload, SoundPayload,
    SoundResponse,
};
pub use placeholders::{
    NATIVE_PLACEHOLDER_DOCS, PlaceholderDoc, PlaceholderScope, native_placeholder_docs,
};

#[cfg(feature = "server")]
pub use payload::{
    player_input_state, player_payload, player_payload_owned, player_position_payload,
};
