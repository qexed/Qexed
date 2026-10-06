pub mod advancements;
pub mod combat;
pub mod crafting;
pub mod durability;
pub mod effects;
pub mod enchanting;
pub mod furnace;
pub mod items;
pub mod oxygen;
pub mod redstone;
pub mod sounds;

use std::time::{Duration, Instant};

use crate::config::GameplayConfig;

#[derive(Debug)]
pub struct GameplayRuntime {
    pub(super) crafting: crafting::CraftingRuntime,
    pub(super) enchanting: enchanting::EnchantingRuntime,
    pub(super) furnace: furnace::FurnaceRuntime,
    pub(super) effects: effects::EffectRuntime,
    pub(super) oxygen: oxygen::OxygenRuntime,
    pub(super) advancements: advancements::AdvancementRuntime,
    pub(super) redstone: redstone::RedstoneRuntime,
    furnace_tick_due: Instant,
    redstone_tick_due: Instant,
    oxygen_tick_due: Instant,
    farmland_tick_due: Instant,
}

impl GameplayRuntime {
    pub fn new(config: &GameplayConfig) -> Self {
        let now = Instant::now();
        let furnace_interval = Duration::from_millis(config.furnace_tick_ms.max(1));
        let redstone_interval = Duration::from_millis(config.redstone_tick_ms.max(1));
        let oxygen_interval = Duration::from_millis(config.oxygen_tick_ms.max(1));
        let farmland_interval = Duration::from_millis(config.farmland_tick_ms.max(1));
        Self {
            crafting: crafting::CraftingRuntime::new(),
            enchanting: enchanting::EnchantingRuntime::new(),
            furnace: furnace::FurnaceRuntime::new(),
            effects: effects::EffectRuntime::new(),
            oxygen: oxygen::OxygenRuntime::new(),
            advancements: advancements::AdvancementRuntime::new(config),
            redstone: redstone::RedstoneRuntime::new(),
            furnace_tick_due: now + furnace_interval,
            redstone_tick_due: now + redstone_interval,
            oxygen_tick_due: now + oxygen_interval,
            farmland_tick_due: now + farmland_interval,
        }
    }

    pub fn should_tick_furnace(&mut self, config: &GameplayConfig) -> bool {
        tick_due(&mut self.furnace_tick_due, config.furnace_tick_ms)
    }

    pub fn should_tick_oxygen(&mut self, config: &GameplayConfig) -> bool {
        tick_due(&mut self.oxygen_tick_due, config.oxygen_tick_ms)
    }

    pub fn should_tick_redstone(&mut self, config: &GameplayConfig) -> bool {
        tick_due(&mut self.redstone_tick_due, config.redstone_tick_ms)
    }

    pub fn should_tick_farmland(&mut self, config: &GameplayConfig) -> bool {
        tick_due(&mut self.farmland_tick_due, config.farmland_tick_ms)
    }
}

/// 装配层可达的红石方块变更（dimension/位置/方块状态）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedstoneBlockChangePublic {
    pub dimension: String,
    pub position: qexed_packet::net_types::Position,
    pub block_state: i32,
}

impl GameplayRuntime {
    /// 红石 tick（v4 redstone 分支）：返回待应用的方块状态变更。
    /// 装配层负责把变更写入世界（place_blocks）并广播方块更新包。
    pub fn tick_redstone_changes(
        &mut self,
        world: &dyn crate::world_access::WorldBlockSource,
        world_rules: &qexed_world::world::WorldRulesManager,
        players: &qexed_player::PlayerManager,
        config: &crate::config::GameplayConfig,
    ) -> Vec<RedstoneBlockChangePublic> {
        self.redstone
            .tick(world, world_rules, players, config)
            .into_iter()
            .map(|change| RedstoneBlockChangePublic {
                dimension: change.dimension,
                position: change.position,
                block_state: change.block_state,
            })
            .collect()
    }

    /// 熔炉 tick（缓冲 sink 版）：把同步包写入 Vec<u8> 缓冲并返回
    /// 原始字节（已含包 ID，装配层直接经 PlayerManager 发送）与动作结果。
    pub async fn tick_furnace_buffered(
        &mut self,
        player: &qexed_player::OnlinePlayer,
        plugins: &qexed_plugins::PluginManager,
        config: &crate::config::GameplayConfig,
    ) -> crate::error::Result<(Vec<u8>, GameplayActionOutcome)> {
        let mut buffer = Vec::new();
        let mut sink = qexed_connection::transport::PacketSink::new(&mut buffer);
        let outcome = self
            .furnace
            .tick(&mut sink, player, plugins, config)
            .await?;
        sink.flush().await?;
        drop(sink);
        Ok((buffer, outcome))
    }

    /// 氧气 tick（缓冲 sink 版）：返回（原始包字节, 水下判定传入、
    /// 生存伤害结果）。
    pub async fn tick_oxygen_buffered(
        &mut self,
        player: &qexed_player::OnlinePlayer,
        plugins: &qexed_plugins::PluginManager,
        config: &crate::config::GameplayConfig,
        underwater: bool,
        inventory: &crate::inventory::PlayerInventory,
        survival: &mut crate::survival::SurvivalState,
        game_mode: crate::config::GameMode,
    ) -> crate::error::Result<(Vec<u8>, crate::survival::SurvivalDamage)> {
        let mut buffer = Vec::new();
        let mut sink = qexed_connection::transport::PacketSink::new(&mut buffer);
        let damage = self
            .oxygen
            .tick(
                &mut sink,
                player,
                plugins,
                config,
                underwater,
                inventory,
                &self.effects,
                survival,
                game_mode,
            )
            .await?;
        sink.flush().await?;
        drop(sink);
        Ok((buffer, damage))
    }
}
fn tick_due(deadline: &mut Instant, interval_ms: u64) -> bool {
    let now = Instant::now();
    if now < *deadline {
        return false;
    }
    let interval = Duration::from_millis(interval_ms.max(1));
    while *deadline <= now {
        *deadline += interval;
    }
    true
}

#[derive(Debug, Default)]
pub struct GameplayActionOutcome {
    pub(super) handled: bool,
    pub(super) inventory_changes: Vec<crate::inventory::InventorySlotChange>,
    pub(super) close_window: bool,
    pub(super) grant_triggers: Vec<crate::config::CustomAdvancementTrigger>,
}

impl GameplayActionOutcome {
    pub fn handled() -> Self {
        Self {
            handled: true,
            ..Self::default()
        }
    }

    pub fn merge(&mut self, mut other: Self) {
        self.handled |= other.handled;
        self.inventory_changes.append(&mut other.inventory_changes);
        self.close_window |= other.close_window;
        self.grant_triggers.append(&mut other.grant_triggers);
    }
}
