//! 组装层的真 GameplayHooks：把 session_core 的回调接到 play 域子系统。
//!
//! 接线总览（v4 play.rs 内联逻辑 → v6 hook 装配）：
//! - survival_tick        → survival::SurvivalState::tick（饥饿/自然恢复/饿死）
//! - survival_movement    → survival::SurvivalState::apply_movement（坠落/虚空伤害）
//! - apply_damage         → survival::SurvivalState::apply_damage + gameplay::combat
//!                          的反馈包（DamageEvent/HurtAnimation/SetEntityMotion）
//! - handle_death         → survival::SurvivalState::respawn + world spawn 重生点 +
//!                          死亡掉落（drop_inventory_on_death）
//! - collect_nearby_drops → drops::collect_dropped_items + inventory 部分拾取
//! - handle_command       → chat.rs 入口（pub(crate) + 12 会话参数）在 hook 面
//!                          不可达，待 play-core 基座定稿后接线（返回 false）
//! - gameplay_tick        → GameplayRuntime 的 furnace/oxygen/redstone 节拍推进；
//!                          子系统 tick 体为 pub(in crate::gameplay) 且需
//!                          async sink，节拍照常推进、本体待 play-core 会话入口
//!
//! 协议出口：hook 是同步的（trait 无 sink 参数），所有发往会话玩家自身的包经
//! PlayerManager::send_packets_to（ClientboundPackets 事件回到本会话 sink），
//! 其他玩家经 broadcast_packets_except。

use std::sync::Arc;
use std::time::Duration;

use qexed_entities::{EntityManager, EntityRendering};
use qexed_packet::net_types::VarInt;
use qexed_play::gameplay::GameplayRuntime;
use qexed_play::plugin_bridge::PlayerViewerBridge;
use qexed_play::survival::{
    DeathMessage, FallContext, SurvivalState, StoredSurvival, death_message_broadcast_packet,
    external_death_message_broadcast_packet,
};
use qexed_play::{
    GameplayHooks, GameMode, PlayConfig, Result, SessionTickContext, collect_dropped_items,
};
use qexed_player::PlayerManager;
use qexed_protocol::to_client::play::{
    damage_event::DamageEvent, set_health::SetHealth, take_item_entity::TakeItemEntity,
};
use qexed_protocol::types::EntityPosition;

/// survival_tick 的会话节拍（session_core SURVIVAL_TICK_INTERVAL 同值）。
const SURVIVAL_TICK_INTERVAL: Duration = Duration::from_secs(1);

pub struct RealGameplay {
    config: PlayConfig,
    runtime: std::sync::Mutex<GameplayRuntime>,
    survival: std::sync::Mutex<SurvivalState>,
    /// 玩家统计（原版统计页数据源）。
    pub stats: std::sync::Arc<qexed_statistics::StatsCounter>,
    players: Arc<PlayerManager>,
    entities: Arc<EntityManager>,
    rendering: EntityRendering,
    /// 世界句柄（FallContext 方块查询 / 世界写冲刷）。
    world: Arc<qexed_world::world::WorldManager>,
    /// 插件域句柄（plugin_player_tick 查询）。
    plugins: Arc<qexed_plugins::PluginManager>,
    /// 维度规则句柄（红石 tick 的日光感器时间查询）。
    world_rules: Arc<qexed_world::world::WorldRulesManager>,
    /// 会话背包（v4 inventory::PlayerInventory；本会话独占，经 hook 面维护）。
    inventory: std::sync::Arc<tokio::sync::Mutex<qexed_play::inventory::PlayerInventory>>,
}

impl RealGameplay {
    #[allow(clippy::too_many_arguments)]
    /// 注入共享统计计数器（默认自建——跨会话共享用全局 Registry 实例）。
    pub fn with_stats(mut self, stats: std::sync::Arc<qexed_statistics::StatsCounter>) -> Self {
        self.stats = stats;
        self
    }

    pub fn new(
        config: PlayConfig,
        players: Arc<PlayerManager>,
        entities: Arc<EntityManager>,
        rendering: EntityRendering,
        stored: StoredSurvival,
        stored_inventory: &qexed_player::StoredInventory,
        world: Arc<qexed_world::world::WorldManager>,
        plugins: Arc<qexed_plugins::PluginManager>,
        world_rules: Arc<qexed_world::world::WorldRulesManager>,
    ) -> Self {
        Self {
            runtime: std::sync::Mutex::new(GameplayRuntime::new(&config.gameplay)),
            stats: qexed_statistics::StatsCounter::shared(),
            survival: std::sync::Mutex::new(SurvivalState::from_stored(
                stored,
                config.world.game_mode,
            )),
            players,
            entities,
            rendering,
            world,
            plugins,
            world_rules,
            inventory: std::sync::Arc::new(tokio::sync::Mutex::new(
                qexed_play::inventory::PlayerInventory::from_stored(
                    stored_inventory,
                ),
            )),
            config,
        }
    }

    /// 会话背包句柄（chat 命令 /give 与插件 GiveItem/ResetInventory
    /// action 经此共享；与 gameplay hook 面使用同一份状态）。
    pub fn inventory_handle(
        &self,
    ) -> &std::sync::Arc<tokio::sync::Mutex<qexed_play::inventory::PlayerInventory>> {
        &self.inventory
    }

        /// 当前生存态快照（装配层 autosave/disconnect 存档用；session_core 的
    /// save_player_runtime 目前仍读 PlayerData 内嵌 survival，接线后启用）。
    #[allow(dead_code)]
    pub fn survival_stored(&self) -> StoredSurvival {
        self.survival
            .lock()
            .expect("survival state poisoned")
            .to_stored()
    }
}

impl GameplayHooks for RealGameplay {
    fn survival_movement(
        &self,
        ctx: &SessionTickContext<'_>,
        previous: EntityPosition,
    ) -> Result<bool> {
        // FallContext 经 world 方块查询构造（着陆地/攀爬物/岩浆）。
        let fall_context = self.fall_context(ctx);
        let outcome = self
            .survival
            .lock()
            .expect("survival state poisoned")
            .apply_movement(ctx.game_mode, previous, ctx.position, fall_context);
        self.sync_health_if_changed(ctx, outcome);
        // 坠落伤害不触发传送。
        Ok(false)
    }

    fn survival_tick(&self, ctx: &SessionTickContext<'_>) -> Result<Option<String>> {
        // 原版行为：play_time 累计（统计页"游戏时间"数据源）。
        self.stats.add(
            qexed_statistics::StatKey::custom(qexed_statistics::CUSTOM_PLAY_TIME),
            1,
        );
        log::info!("[stats-diag] play_time tick, total={}", self.stats.get(&qexed_statistics::StatKey::custom(qexed_statistics::CUSTOM_PLAY_TIME)));
        let outcome = self
            .survival
            .lock()
            .expect("survival state poisoned")
            .tick(ctx.game_mode, SURVIVAL_TICK_INTERVAL);
        self.sync_health_if_changed(ctx, outcome);
        // 死亡消息按协议语义返回翻译键（如 death.attack.starve），
        // session_core 收到后回调 handle_death。
        Ok(outcome
            .death_message()
            .map(|message| message.translation_key().to_string()))
    }

    fn apply_damage(
        &self,
        ctx: &SessionTickContext<'_>,
        amount: f32,
        kind: qexed_player::PlayerDamageKind,
        source_entity_id: i32,
        source_position: EntityPosition,
        knockback: f32,
    ) -> Result<Option<String>> {
        if ctx.game_mode != GameMode::Survival || !amount.is_finite() || amount <= 0.0 {
            return Ok(None);
        }

        let mut survival = self.survival.lock().expect("survival state poisoned");
        if survival.is_dead() {
            return Ok(None);
        }

        // 伤害计算（v4 apply_external_player_damage）：先反馈受击动画/击退，
        // 再落到生存态；死亡时返回按协议语义的死亡消息键。
        let message = qexed_play::survival::external_damage_death_message(kind);
        let outcome = survival.apply_damage(amount, message);
        drop(survival);
        if !outcome.changed() {
            return Ok(None);
        }

        self.send_external_damage_feedback(ctx, source_entity_id, source_position, knockback);
        if let Some(message) = outcome.death_message() {
            self.broadcast_death(ctx, message, kind, source_entity_id);
            Ok(Some(external_death_message_key(kind)))
        } else {
            self.send_health(ctx);
            Ok(None)
        }
    }

    fn handle_death(&self, ctx: &SessionTickContext<'_>, _message: &str) -> Result<()> {
        let mut survival = self.survival.lock().expect("survival state poisoned");
        if !survival.is_dead() {
            return Ok(());
        }
        survival.respawn();
        drop(survival);

        // 死亡掉落（v4 drop_player_inventory_on_death 的 play 子集）。
        if self.config.gameplay.drop_inventory_on_death {
            self.drop_inventory_on_death(ctx);
        }

        // 重生点：world spawn 简化（v4 respawn_player 的 spawn_position）。
        // 完整重生包序列（Respawn + PlayerPosition + respawn_player_state + 区块
        // 重置）需要 sink/chunk_state，归 session_core 的 respawn 路径；hook 侧把
        // 位置同步到 PlayerManager，会话循环的 chunk center 逻辑随之收敛。
        let spawn = qexed_play::spawn_position(&self.config.world.spawn);
        let dimension = self.config.world.default_dimension().to_string();
        self.players.teleport_player(ctx.profile_id, dimension, spawn);
        // 重生后满血包（经事件通道回到会话 sink）。
        self.send_health_at(ctx.profile_id, 20.0, 20, 5.0);
        Ok(())
    }

    fn gameplay_tick(&self, ctx: &SessionTickContext<'_>) -> Result<()> {
        // PlayConfig 构造时持有（不逐 tick default()）。
        let gameplay_config = &self.config.gameplay;
        let mut runtime = self.runtime.lock().expect("gameplay runtime poisoned");
        // 节拍推进（到期即推进 deadline，避免积压）。
        let furnace_due = runtime.should_tick_furnace(gameplay_config);
        let oxygen_due = runtime.should_tick_oxygen(gameplay_config);
        let redstone_due = runtime.should_tick_redstone(gameplay_config);
        let _farmland_due = runtime.should_tick_farmland(gameplay_config);

        // 红石 tick：同步驱动（无 sink），变更经 WorldManager 写入并广播。
        if redstone_due {
            let world_source = crate::world_adapter::RealWorld(std::sync::Arc::clone(&self.world));
            let changes = runtime.tick_redstone_changes(
                &world_source,
                &self.world_rules,
                &self.players,
                gameplay_config,
            );
            self.apply_redstone_changes(ctx, changes)?;
        }

        // 熔炉/氧气 tick：async（缓冲 sink 发包），多线程 runtime 下
        // 经 block_in_place 桥接；包经 PlayerManager 事件通道回到会话 sink。
        if furnace_due || oxygen_due {
            let player = self
                .players
                .player_by_uuid(ctx.profile_id)
                .ok_or_else(|| qexed_play::PlayError::msg("session player missing"))?;
            let plugins = self.plugins.clone();
            let players = self.players.clone();
            let profile_id = ctx.profile_id;
            let handle = tokio::runtime::Handle::current();
            let mut runtime_guard = runtime;
            tokio::task::block_in_place(|| {
                handle.block_on(async {
                    if furnace_due {
                        let (buffer, outcome) = runtime_guard
                            .tick_furnace_buffered(&player, &plugins, gameplay_config)
                            .await?;
                        Self::send_buffer(&players, profile_id, buffer);
                        let _ = outcome; // 进度触发/背包变更已同步进包
                    }
                    if oxygen_due {
                        let underwater = self.is_underwater(ctx);
                        let inventory = self.inventory.clone();
                        let inventory_guard = inventory.lock().await;
                        let mut survival = self.survival.lock().expect("survival state poisoned");
                        let (buffer, damage) = runtime_guard
                            .tick_oxygen_buffered(
                                &player,
                                &plugins,
                                gameplay_config,
                                underwater,
                                &inventory_guard,
                                &mut survival,
                                ctx.game_mode,
                            )
                            .await?;
                        drop((inventory_guard, survival));
                        Self::send_buffer(&players, profile_id, buffer);
                        self.sync_health_if_changed(ctx, damage);
                        if let Some(message) = damage.death_message() {
                            self.broadcast_death(ctx, message, qexed_player::PlayerDamageKind::Generic, 0);
                        }
                    }
                    qexed_play::Result::Ok(())
                })
            })?;
        }
        Ok(())
    }

    fn plugin_player_tick(&self, ctx: &SessionTickContext<'_>, interval_ms: u64) -> Result<()> {
        // v4 plugins.handle_player_tick：查询插件 player tick 事件并应用响应
        // action。完整 action 应用（teleport/give/menu 等）需要会话状态（见
        // chat::apply_plugin_action 的 12+ 参数面），此处应用事件通道可达的
        // SystemMessage 子集，其余记录后跳过。
        let player = self.players.list_except(ctx.profile_id);
        let _ = player; // 单玩家查询接口（PlayerManager 无 get(uuid)），payload 从 ctx 构造
        let payload = qexed_plugins::api::PlayerPayloadOwned {
            uuid: ctx.profile_id.to_string(),
            username: self.players.display_name(ctx.profile_id),
            entity_id: ctx.entity_id,
            language: String::new(),
            dimension: ctx.dimension.to_string(),
        };
        let position = qexed_plugins::api::PlayerPositionPayload {
            x: ctx.position.x,
            y: ctx.position.y,
            z: ctx.position.z,
            yaw: ctx.position.yaw,
            pitch: ctx.position.pitch,
            on_ground: ctx.position.on_ground,
        };
        let response = self.plugins.handle_player_tick(
            &payload,
            ctx.dimension,
            position,
            interval_ms,
        );
        for action in response.actions {
            self.apply_plugin_action_reachable(ctx, action);
        }
        Ok(())
    }

    fn handle_command(&self, _ctx: &SessionTickContext<'_>, _command: &str) -> Result<bool> {
        // chat::handle_chat_command 是 qexed_play 的 pub(crate) 入口且需要
        // sink/chunk_state/menus 等 12+ 会话参数——hook 面不可达（返回 false
        // 交由 session_core 记录跳过）。命令面归 play-core 基座定稿后的
        // chat 接线任务。
        Ok(false)
    }

    fn collect_nearby_drops(&self, ctx: &SessionTickContext<'_>) -> Result<()> {
        let items = collect_dropped_items(&self.entities, ctx.dimension, ctx.position)?;
        if items.is_empty() {
            return Ok(());
        }

        let mut inventory = self.lock_inventory_blocking();
        let mut picked = Vec::new();
        let mut changes = Vec::new();
        for item in items {
            let original_count = item.item.item_count.0;
            let (mut item_changes, picked_count) = inventory.add_item_stack_partial(&item.item);
            if picked_count > 0 {
                changes.append(&mut item_changes);
                picked.push((item.clone(), picked_count));
                if picked_count < original_count {
                    let mut remaining = item;
                    remaining.item.item_count = VarInt(original_count - picked_count);
                    self.entities.restore_dropped_item(remaining);
                }
            } else {
                self.entities.restore_dropped_item(item);
            }
        }
        drop(inventory);

        if picked.is_empty() {
            return Ok(());
        }

        // 背包变更同步（v4 sync_inventory_changes 的 SetPlayerInventory 子集；
        // InventorySlotChange 已携带最新槽内容，无需再读背包）。
        let sync_packets: Vec<_> = changes
            .iter()
            .filter_map(inventory_change_packet)
            .collect();
        for packet in sync_packets {
            let bytes = qexed_player::packet_bytes(packet)?;
            self.players.send_packets_to(ctx.profile_id, vec![bytes]);
        }

        // 拾取动画（v4 item_pickup_packets：部分拾取 TakeItemEntity + metadata，
        // 全拾取 pickup_packets 的 Take + Remove）。
        for (item, amount) in picked {
            let packets = pickup_packets(&item, ctx.entity_id, amount)?;
            self.players.send_packets_to(ctx.profile_id, packets.clone());
            self.players
                .broadcast_packets_except(ctx.profile_id, packets);
        }
        Ok(())
    }

    fn inventory_snapshot(&self) -> qexed_player::StoredInventory {
        self.lock_inventory_blocking().to_stored()
    }

    fn flush_world_writes(&self) {
        // v4 world.flush_block_writes（autosave 后调用）：
        // WorldManager 的 flush_block_writes 为 pub(crate)，经公开包装调用。
        self.world.flush_block_writes_public();
    }
}

impl RealGameplay {
    /// 红石方块变更落世界 + 广播（v4 apply_block_updates 子集）。
    fn apply_redstone_changes(
        &self,
        ctx: &SessionTickContext<'_>,
        changes: Vec<qexed_play::gameplay::RedstoneBlockChangePublic>,
    ) -> Result<()> {
        if changes.is_empty() {
            return Ok(());
        }
        // 按维度分组写入（异维变更不能合并一次 place_blocks）。
        let mut by_dimension: std::collections::HashMap<String, Vec<_>> =
            std::collections::HashMap::new();
        for change in changes {
            by_dimension
                .entry(change.dimension)
                .or_default()
                .push((change.position, change.block_state));
        }
        let mut packets = Vec::new();
        for (dimension, blocks) in by_dimension {
            let updates = self
                .world
                .place_blocks(dimension.as_str(), blocks)
                .map_err(|err| qexed_play::PlayError::msg(err.to_string()))?;
            for update in updates {
                packets.push(qexed_player::packet_bytes(
                    qexed_protocol::to_client::play::block_update::BlockUpdate {
                        location: update.location,
                        block_state: update.block_state,
                    },
                )?);
            }
        }
        self.players
            .broadcast_packets_except(ctx.profile_id, packets.clone());
        self.players.send_packets_to(ctx.profile_id, packets);
        self.world.flush_block_writes_public();
        Ok(())
    }

    /// 头部方块水性判定（v4 oxygen 分支的最小实现：
    /// 眼位方块为 water 即视为水下；未加载返回 false）。
    fn is_underwater(&self, ctx: &SessionTickContext<'_>) -> bool {
        use qexed_play::world_access::WorldBlockSource as _;
        let position = qexed_packet::net_types::Position {
            x: ctx.position.x.floor() as i32,
            y: ctx.position.y.floor() as i32,
            z: ctx.position.z.floor() as i32,
        };
        self.world
            .block_state_at(ctx.dimension, &position)
            .and_then(|state| qexed_play::inventory::block_name_for_state(state))
            .is_some_and(|name| name == "minecraft:water")
    }

    /// 原始包字节经 PlayerManager 发送（事件通道回到会话 sink）。
    fn send_buffer(
        players: &Arc<PlayerManager>,
        profile_id: uuid::Uuid,
        buffer: Vec<u8>,
    ) {
        if buffer.is_empty() {
            return;
        }
        // 缓冲区含多个完整帧（已含包 ID 与长度前缀），作为单个原始帧发送。
        players.send_packets_to(profile_id, vec![bytes::Bytes::from(buffer)]);
    }

    /// 同步 hook 面获取背包守卫（tokio 锁的阻塞获取：
    /// 多线程 runtime 下 block_in_place + block_on，低频路径可接受）。
    fn lock_inventory_blocking(
        &self,
    ) -> tokio::sync::OwnedMutexGuard<qexed_play::inventory::PlayerInventory> {
        let mutex = std::sync::Arc::clone(&self.inventory);
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move { mutex.lock_owned().await })
        })
    }

    /// 坠落上下文（v4 survival_movement 的 world 查询子集）：
    /// 脚下方块判岩浆/攀爬物，正下方方块判软着陆面。
    fn fall_context(&self, ctx: &SessionTickContext<'_>) -> FallContext {
        use qexed_play::survival::{FallLanding, FallLocation};

        let feet = block_position(ctx.position.x, ctx.position.y, ctx.position.z);
        let below = block_position(ctx.position.x, ctx.position.y - 1.0, ctx.position.z);
        let feet_state = self
            .world
            .block_state_at(ctx.dimension, &feet);
        let below_state = self.world.block_state_at(ctx.dimension, &below);
        let feet_name = feet_state
            .and_then(qexed_play::inventory::block_name_for_state)
            .unwrap_or_default();
        let below_name = below_state
            .and_then(qexed_play::inventory::block_name_for_state)
            .unwrap_or_default();

        FallContext {
            in_lava: feet_name == "minecraft:lava",
            landing: match below_name.as_str() {
                "minecraft:hay_block" => FallLanding::Hay,
                "minecraft:honey_block" => FallLanding::Honey,
                "minecraft:slime_block" => FallLanding::Slime,
                "minecraft:powder_snow" => FallLanding::PowderSnow,
                "minecraft:pointed_dripstone" => FallLanding::PointedDripstoneTip,
                _ => FallLanding::Generic,
            },
            climbable: match feet_name.as_str() {
                "minecraft:ladder" => Some(FallLocation::Ladder),
                "minecraft:vine" => Some(FallLocation::Vines),
                "minecraft:weeping_vines" | "minecraft:weeping_vines_plant" => {
                    Some(FallLocation::WeepingVines)
                }
                "minecraft:twisting_vines" | "minecraft:twisting_vines_plant" => {
                    Some(FallLocation::TwistingVines)
                }
                "minecraft:scaffolding" => Some(FallLocation::Scaffolding),
                "minecraft:cave_vines" | "minecraft:cave_vines_plant" => {
                    Some(FallLocation::OtherClimbable)
                }
                _ => None,
            },
        }
    }

    /// 插件 player tick 响应 action 中事件通道可达的子集（SystemMessage）；
    /// 其余（teleport/give/menu 等）需要会话状态，见 chat::apply_plugin_action。
    fn apply_plugin_action_reachable(
        &self,
        ctx: &SessionTickContext<'_>,
        action: qexed_plugins::api::PlayerAction,
    ) {
        match action {
            qexed_plugins::api::PlayerAction::SystemMessage {
                text,
                translate,
                with,
                overlay,
            } => {
                let content = if !translate.trim().is_empty() {
                    translatable_component(
                        translate,
                        with.into_iter().map(text_component).collect::<Vec<_>>(),
                    )
                } else {
                    text_component(text)
                };
                self.send_packet_to(
                    ctx.profile_id,
                    qexed_protocol::to_client::play::system_chat::SystemChat {
                        content,
                        overlay,
                    },
                );
            }
            other => {
                log::debug!(
                    "plugin player tick action requires session state, skipped: {other:?}"
                );
            }
        }
    }

    /// 生存 tick/移动结果落包（非死亡时同步 SetHealth）。
    fn sync_health_if_changed(
        &self,
        ctx: &SessionTickContext<'_>,
        outcome: qexed_play::survival::SurvivalDamage,
    ) {
        if !outcome.changed() || outcome.death_message().is_some() {
            return;
        }
        self.send_health(ctx);
    }

    fn send_health(&self, ctx: &SessionTickContext<'_>) {
        let packet = self
            .survival
            .lock()
            .expect("survival state poisoned")
            .health_packet();
        self.send_packet_to(ctx.profile_id, packet);
    }

    fn send_health_at(&self, profile_id: uuid::Uuid, health: f32, food: i32, saturation: f32) {
        self.send_packet_to(
            profile_id,
            SetHealth {
                health,
                food: VarInt(food),
                saturation,
            },
        );
    }

    /// 外部伤害反馈（v4 send_external_damage_feedback）：DamageEvent +
    /// HurtAnimation（+ 击退 SetEntityMotion），本人经事件通道、他人经广播。
    fn send_external_damage_feedback(
        &self,
        ctx: &SessionTickContext<'_>,
        source_entity_id: i32,
        source_position: EntityPosition,
        knockback: f32,
    ) {
        let yaw =
            qexed_play::gameplay::combat::damage_yaw_from_source(ctx.position, source_position);
        let damage_event = DamageEvent {
            entity_id: VarInt(ctx.entity_id),
            // source_type 0 = minecraft:generic（damage_type 注册表 holder 占位）。
            source_type: VarInt(0),
            source_cause_id: VarInt(source_entity_id),
            source_direct_id: VarInt(source_entity_id),
            source_position: Some(qexed_protocol::to_client::play::damage_event::Vec3d {
                x: source_position.x,
                y: source_position.y,
                z: source_position.z,
            }),
        };
        let hurt = qexed_protocol::to_client::play::hurt_animation::HurtAnimation {
            entity_id: VarInt(ctx.entity_id),
            yaw,
        };
        let mut broadcast = Vec::new();
        for packet_bytes_result in [
            qexed_player::packet_bytes(damage_event),
            qexed_player::packet_bytes(hurt),
        ] {
            match packet_bytes_result {
                Ok(bytes) => broadcast.push(bytes),
                Err(err) => {
                    log::warn!("encode damage feedback failed: {err}");
                    return;
                }
            }
        }
        if knockback > 0.0 {
            let motion = qexed_play::gameplay::combat::external_damage_knockback_packet(
                ctx.entity_id,
                ctx.position,
                source_position,
                knockback,
            );
            match qexed_player::packet_bytes(motion) {
                Ok(bytes) => broadcast.push(bytes),
                Err(err) => log::warn!("encode knockback failed: {err}"),
            }
        }
        self.players
            .send_packets_to(ctx.profile_id, broadcast.clone());
        self.players
            .broadcast_packets_except(ctx.profile_id, broadcast);
    }

    /// 死亡广播（v4 handle_player_death 的包序列子集）：SetHealth(0) + 死亡消息；
    /// 外部伤害用来源实体名变体（death.attack.mob / arrow / explosion.player）。
    fn broadcast_death(
        &self,
        ctx: &SessionTickContext<'_>,
        message: DeathMessage,
        kind: qexed_player::PlayerDamageKind,
        source_entity_id: i32,
    ) {
        let health = SetHealth {
            health: 0.0,
            food: VarInt(0),
            saturation: 0.0,
        };
        self.send_packet_to(ctx.profile_id, health);

        let death_packet = external_death_message_broadcast_packet(
            &self.players,
            &self.entities,
            kind,
            source_entity_id,
            ctx.profile_id,
        )
        .or_else(|_| death_message_broadcast_packet(&self.players, message, ctx.profile_id));
        match death_packet {
            Ok(bytes) => {
                self.players
                    .send_packets_to(ctx.profile_id, vec![bytes.clone()]);
                self.players
                    .broadcast_packets_except(ctx.profile_id, vec![bytes]);
            }
            Err(err) => log::warn!("encode death message failed: {err}"),
        }
        log::debug!(
            "player died: entity_id={}, message={}",
            ctx.entity_id,
            message.translation_key()
        );
    }

    /// 死亡掉落（v4 drop_player_inventory_on_death）：清空背包并以掉落物实体
    /// 回到死亡位置（v4 的 0.3 半径随机省略为原地 + 0.5 抬升）。
    fn drop_inventory_on_death(&self, ctx: &SessionTickContext<'_>) {
        let (slots, _changes) =
            self.lock_inventory_blocking().drain_droppable_items();
        if slots.is_empty() {
            return;
        }
        let viewers = PlayerViewerBridge::new(&self.players);
        let mut position = ctx.position;
        position.y += 0.5;
        for slot in slots {
            if let Err(err) = self.entities.drop_item_with_rendering(
                &viewers,
                ctx.profile_id,
                ctx.dimension,
                qexed_play::plugin_bridge::entities_position(position),
                slot,
                &self.rendering,
            ) {
                log::warn!("drop inventory on death failed: {err}");
            }
        }
    }

    fn send_packet_to<T: qexed_packet::Packet>(&self, profile_id: uuid::Uuid, packet: T) {
        match qexed_player::packet_bytes(packet) {
            Ok(bytes) => self.players.send_packets_to(profile_id, vec![bytes]),
            Err(err) => log::warn!("encode play packet failed: {err}"),
        }
    }
}

/// 坐标 → 方块位置（floor 语义，与 chunk_coord 一致）。
fn block_position(x: f64, y: f64, z: f64) -> qexed_packet::net_types::Position {
    qexed_packet::net_types::Position {
        x: x.floor() as i32,
        y: y.floor() as i32,
        z: z.floor() as i32,
    }
}

/// v4 play/util text_component 的本地等价（qexed_play::util 未导出）。
fn text_component(text: impl Into<String>) -> qexed_protocol::types::TextComponent {
    let mut map = std::collections::HashMap::new();
    map.insert(
        "text".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from(text.into())),
    );
    qexed_nbt::Tag::Compound(std::sync::Arc::new(map))
}

/// v4 play/util translatable_component 的本地等价。
fn translatable_component(
    key: impl Into<String>,
    with: Vec<qexed_protocol::types::TextComponent>,
) -> qexed_protocol::types::TextComponent {
    let mut map = std::collections::HashMap::new();
    map.insert(
        "translate".to_string(),
        qexed_nbt::Tag::String(std::sync::Arc::from(key.into())),
    );
    if !with.is_empty() {
        map.insert(
            "with".to_string(),
            qexed_nbt::Tag::List(
                qexed_nbt::ListHeader {
                    tag_id: qexed_nbt::tag_id::COMPOUND,
                    length: with.len() as i32,
                },
                std::sync::Arc::from(with.into_boxed_slice()),
            ),
        );
    }
    qexed_nbt::Tag::Compound(std::sync::Arc::new(map))
}

/// 外部伤害死亡消息键（survival 域折叠枚举 → 协议翻译键）。
fn external_death_message_key(kind: qexed_player::PlayerDamageKind) -> String {
    qexed_play::survival::external_damage_death_message(kind)
        .translation_key()
        .to_string()
}

/// 背包槽变更 → SetPlayerInventory 包。
fn inventory_change_packet(
    change: &qexed_play::inventory::InventorySlotChange,
) -> Option<qexed_protocol::to_client::play::set_player_inventory::SetPlayerInventory> {
    match change {
        qexed_play::inventory::InventorySlotChange::Hotbar { slot, item } => {
            Some(qexed_play::inventory::set_player_inventory_packet(*slot, item.clone()))
        }
        qexed_play::inventory::InventorySlotChange::Main { slot, item } => Some(
            qexed_play::inventory::set_player_main_inventory_packet(*slot, item.clone()),
        ),
        qexed_play::inventory::InventorySlotChange::Equipment { .. } => None,
    }
}

/// 拾取动画包（v4 item_pickup_packets）。
fn pickup_packets(
    item: &qexed_entities::DroppedItemEntity,
    collector_entity_id: i32,
    amount: i32,
) -> Result<Vec<bytes::Bytes>> {
    if amount >= item.item.item_count.0 {
        return Ok(item
            .pickup_packets(collector_entity_id)
            .map_err(qexed_play::PlayError::from)?);
    }
    let take = qexed_player::packet_bytes(TakeItemEntity {
        item_id: VarInt(item.entity_id),
        player_id: VarInt(collector_entity_id),
        amount: VarInt(amount.max(1)),
    })?;
    let mut remaining = item.clone();
    remaining.item.item_count = VarInt(item.item.item_count.0 - amount.max(0));
    let mut packets = vec![take];
    packets.extend(remaining.metadata_packets().map_err(qexed_play::PlayError::from)?);
    Ok(packets)
}
