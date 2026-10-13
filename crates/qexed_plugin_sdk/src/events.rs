//! 常用事件的 re-export（从 qexed_plugin_api 根）。
//!
//! v4 每个事件一个 export 函数；v6 是单一 `ctx.register::<E>()` 通道，
//! 这里只把迁移中最常用的事件集中命名一次。

pub use qexed_plugin_api::{
    ChatCommandEvent, EntityAiTickEvent, NpcInteractEvent, NpcMutationsEvent,
    PlaceholdersEvent, PlayerAttackEvent, PlayerBlockInteractEvent, PlayerBlockStepEvent,
    PlayerChatEvent, PlayerInputEvent, PlayerJoinEvent, PlayerLeaveEvent, PlayerMoveEvent,
    PlayerUseItemEvent, PluginLoadFinishEvent,
};
