//! 插件 API 层（v4 独立 crate qexed_plugin_api 并入本 crate）。
//!
//! v4 的 `server` feature 转换函数（player_payload / player_position_payload /
//! player_input_state）依赖 qexed_player::OnlinePlayer，v6 各功能 crate 尚未
//! 提供在线玩家结构，此处改为从 v6 协议类型直接构造（OnlinePlayer 相关入口
//! 由 qexed_server 装配层适配，见 manager 各 handle_* 的入参）。

mod event;
mod payload;
mod placeholders;

pub use event::PluginEvent;
pub use payload::{
    AdvancementGrantQuery, AdvancementGrantResponse, BedrockFormResponsePayload, BlockDropItem,
    BlockDropPosition, BlockDropQuery, BlockDropResponse, BlockStepPayload, BlockStepPosition,
    ChunkPayload, ClickDetectedPayload, ConfigReloadPayload, CraftItemQuery, CraftItemResponse,
    CraftingRecipeQuery, CraftingRecipeResponse, CustomEntityDefinition,
    CustomEntityRegistryResponse, EnchantingOption, EnchantingQuery, EnchantingResponse,
    EntityAiEntityPayload, EntityAiOperation, EntityAiPlayerPayload, EntityAiTickQuery,
    EntityAiTickResponse, FurnaceRecipeQuery, FurnaceRecipeResponse, FurnaceTickPayload,
    GeyserPlayerInfoQuery, GeyserPlayerInfoResponse, HttpHeader, HttpRequest, HttpResponse,
    ItemDurabilityQuery, ItemDurabilityResponse, ItemEnchantment, ItemStackPayload,
    LanguagePayload, LocalizedNameQuery, LocalizedNameResponse, MiningSpeedQuery,
    MiningSpeedResponse, NpcEntityPayload, NpcInteractPayload, NpcMove, NpcMutationOp,
    NpcMutationQuery, NpcMutationResponse, NpcPatrol, NpcPatrolPoint, NpcPosition, NpcUpsert,
    PathfindingQuery, PathfindingResponse, PlaceholderContext, PlaceholderQuery,
    PlaceholderReplacement, PlaceholderResponse, PlayerAction, PlayerAttackQuery,
    PlayerAttackResponse, PlayerBlockInteractPayload, PlayerClientPayload, PlayerDeathQuery,
    PlayerDeathResponse, PlayerInputPayload, PlayerInputState, PlayerItemPickupQuery,
    PlayerItemPickupResponse, PlayerMovePayload, PlayerOxygenTickQuery, PlayerOxygenTickResponse,
    PlayerPayload, PlayerPayloadOwned, PlayerPositionPayload, PlayerTickPayload,
    PlayerUseItemPayload, PluginApiCallQuery, PluginApiCallResponse, PluginCommandDefinition,
    PluginCommandQuery, PluginCommandResponse, PluginDependency, PluginEnchantment, PluginManifest,
    PluginServiceDefinition, PotionEffectTickQuery, PotionEffectTickResponse,
    ProjectileHitPlayerPayload, ProxyConnectResultPayload, SoundPayload, SoundResponse,
};
pub use placeholders::{
    NATIVE_PLACEHOLDER_DOCS, PlaceholderDoc, PlaceholderScope, native_placeholder_docs,
};

/// 从 v6 协议的玩家输入位标志（to_server::play::player_input）构造 payload。
///
/// v4 的 `player_input_state(flags)` 迁移；v6 常量路径不变。
pub fn player_input_state(flags: u8) -> PlayerInputState {
    PlayerInputState {
        forward: flags & qexed_protocol::to_server::play::player_input::FORWARD != 0,
        backward: flags & qexed_protocol::to_server::play::player_input::BACKWARD != 0,
        left: flags & qexed_protocol::to_server::play::player_input::LEFT != 0,
        right: flags & qexed_protocol::to_server::play::player_input::RIGHT != 0,
        jump: flags & qexed_protocol::to_server::play::player_input::JUMP != 0,
        shift: flags & qexed_protocol::to_server::play::player_input::SHIFT != 0,
        sprint: flags & qexed_protocol::to_server::play::player_input::SPRINT != 0,
    }
}
