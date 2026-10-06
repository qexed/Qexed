//! 矿坑（ore pit）定期填充：v4 `world/ore_pits.rs` 的 v6 迁移。
//!
//! 适配：
//! - 配置类型（WorldOrePit/WorldOrePitBlock）来自 `crate::config`（v6 无
//!   qexed_config::app::qexed::*）；
//! - v4 直接依赖 `super::WorldManager`（world-core 任务的 manager.rs）与
//!   `crate::inventory`（play crate，未迁移）。这里把所需能力收敛成
//!   [`OrePitWorld`] trait：world-core 的 WorldManager 落地后直接 impl；
//!   block 语义判定（可替换/碰撞）接 chunk_nbt 的方块语义注册表
//!   （blocks.json 运行时加载，OnceLock 缓存）。
//! - EntityPosition 改用 v6 `add_entity::EntityPosition`（v4 同名同构）。

use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
    time::{Duration, Instant},
};

use qexed_packet::net_types::Position;
use qexed_protocol::types::EntityPosition;

use crate::config::{WorldOrePit, WorldOrePitBlock};

/// 矿坑所需的世界能力（v4 WorldManager 投影）。
///
/// 由 `WorldManager`（manager.rs）实现。
pub trait OrePitWorld {
    /// 查询方块状态；未知/未加载返回 None（按空气处理）。
    fn ore_pit_block_state_at(&self, dimension: &str, position: &Position) -> Option<i32>;

    /// 运行时写入方块（不落盘，v4 set_runtime_block）。
    fn ore_pit_set_runtime_block(&self, dimension: &str, position: &Position, block_state: i32);
}

/// 空气方块状态 id（v4 crate::inventory::air_block_state）。
fn air_block_state() -> i32 {
    0
}

/// 方块是否可被矿坑填充替换（v4 block_item_registry().replaceable_block_states）。
///
/// 数据源：chunk_nbt 方块语义注册表（blocks.json，OnceLock 缓存）。
fn can_replace_block_state(block_state: i32) -> bool {
    crate::world::chunk_nbt::can_replace_block_state(block_state)
}

/// 方块是否有碰撞箱（撤离目标找地面用；v4 collision_block_states 注册表）。
fn block_has_collision(block_state: i32) -> bool {
    crate::world::chunk_nbt::block_has_collision(block_state)
}

#[derive(Debug)]
pub struct OrePitManager {
    pits: Vec<RuntimeOrePit>,
    state: Mutex<OrePitState>,
}

impl Default for OrePitManager {
    fn default() -> Self {
        Self::from_config(&[])
    }
}

#[derive(Debug)]
struct RuntimeOrePit {
    id: String,
    dimension: String,
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
    min_z: i32,
    max_z: i32,
    tick_interval: Duration,
    initial_refill: bool,
    max_blocks_per_tick: usize,
    teleport_players_to_surface_on_refill: bool,
    replace_air: bool,
    replace_generated: bool,
    only_break_generated: bool,
    blocks: WeightedBlocks,
}

#[derive(Debug, Default)]
struct OrePitState {
    pits: HashMap<String, RuntimeOrePitState>,
}

#[derive(Debug)]
struct RuntimeOrePitState {
    last_tick: Option<Instant>,
    cursor: usize,
    scanned: usize,
    refill_in_progress: bool,
    generated_positions: HashSet<BlockKey>,
}

impl RuntimeOrePitState {
    fn new(pit: &RuntimeOrePit, now: Instant) -> Self {
        Self {
            last_tick: (!pit.initial_refill).then_some(now),
            cursor: 0,
            scanned: 0,
            refill_in_progress: false,
            generated_positions: HashSet::new(),
        }
    }

    fn start_refill(&mut self, now: Instant) {
        self.last_tick = Some(now);
        self.cursor = 0;
        self.scanned = 0;
        self.refill_in_progress = true;
    }

    fn finish_refill(&mut self) {
        self.cursor = 0;
        self.scanned = 0;
        self.refill_in_progress = false;
    }
}

#[derive(Debug, Clone)]
struct WeightedBlocks {
    states: Vec<WeightedBlock>,
    total_weight: u32,
}

#[derive(Debug, Clone, Copy)]
struct WeightedBlock {
    block_state: i32,
    weight: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct BlockKey {
    x: i32,
    y: i32,
    z: i32,
}

#[derive(Debug, Clone)]
pub struct OrePitBlockUpdate {
    pub dimension: String,
    pub position: Position,
    pub block_state: i32,
}

impl OrePitManager {
    pub fn from_config(pits: &[WorldOrePit]) -> Self {
        Self {
            pits: pits.iter().filter_map(RuntimeOrePit::from_config).collect(),
            state: Mutex::new(OrePitState::default()),
        }
    }

    pub fn tick(&self, world: &dyn OrePitWorld, now: Instant) -> Vec<OrePitBlockUpdate> {
        if self.pits.is_empty() {
            return Vec::new();
        }

        let mut state = self.state.lock().expect("ore pit state poisoned");
        let mut updates = Vec::new();
        for pit in &self.pits {
            let pit_state = state
                .pits
                .entry(pit.id.clone())
                .or_insert_with(|| RuntimeOrePitState::new(pit, now));
            if !pit_state.refill_in_progress {
                if let Some(last_tick) = pit_state.last_tick
                    && now.duration_since(last_tick) < pit.tick_interval
                {
                    continue;
                }
                pit_state.start_refill(now);
            }
            if pit.tick(world, pit_state, &mut updates) {
                pit_state.finish_refill();
            }
        }
        updates
    }

    pub fn evacuation_target_for_player(
        &self,
        world: &dyn OrePitWorld,
        dimension: &str,
        position: EntityPosition,
        now: Instant,
    ) -> Option<EntityPosition> {
        if self.pits.is_empty() {
            return None;
        }

        let block_position = Position {
            x: position.x.floor() as i32,
            y: position.y.floor() as i32,
            z: position.z.floor() as i32,
        };
        let mut state = self.state.lock().expect("ore pit state poisoned");
        self.pits
            .iter()
            .filter(|pit| pit.teleport_players_to_surface_on_refill)
            .filter(|pit| pit.contains(dimension, &block_position))
            .find_map(|pit| {
                let pit_state = state
                    .pits
                    .entry(pit.id.clone())
                    .or_insert_with(|| RuntimeOrePitState::new(pit, now));
                pit.refill_due(pit_state, now)
                    .then(|| pit.surface_position(world, position))
            })
    }

    pub fn permits_player_break(
        &self,
        dimension: &str,
        position: &Position,
        current_state: i32,
    ) -> bool {
        let key = BlockKey::from(position);
        let state = self.state.lock().expect("ore pit state poisoned");
        let restricted_pits = self
            .pits
            .iter()
            .filter(|pit| pit.only_break_generated)
            .filter(|pit| pit.contains(dimension, position))
            .collect::<Vec<_>>();
        if restricted_pits.is_empty() {
            return true;
        }
        restricted_pits.iter().all(|pit| {
            state
                .pits
                .get(&pit.id)
                .is_some_and(|pit_state| pit_state.generated_positions.contains(&key))
                && pit.blocks.contains_state(current_state)
        })
    }
}

impl RuntimeOrePit {
    fn from_config(config: &WorldOrePit) -> Option<Self> {
        if !config.enable {
            return None;
        }
        let dimension = config.dimension.trim();
        if dimension.is_empty() {
            return None;
        }
        let blocks = WeightedBlocks::from_config(&config.blocks)?;
        Some(Self {
            id: ore_pit_id(config),
            dimension: dimension.to_string(),
            min_x: config.min_x.min(config.max_x),
            max_x: config.min_x.max(config.max_x),
            min_y: config.min_y.min(config.max_y),
            max_y: config.min_y.max(config.max_y),
            min_z: config.min_z.min(config.max_z),
            max_z: config.min_z.max(config.max_z),
            tick_interval: Duration::from_millis(config.tick_interval_ms.max(1)),
            initial_refill: config.initial_refill,
            max_blocks_per_tick: config.max_blocks_per_tick.max(1),
            teleport_players_to_surface_on_refill: config.teleport_players_to_surface_on_refill,
            replace_air: config.replace_air,
            replace_generated: config.replace_generated,
            only_break_generated: config.only_break_generated,
            blocks,
        })
    }

    fn tick(
        &self,
        world: &dyn OrePitWorld,
        state: &mut RuntimeOrePitState,
        updates: &mut Vec<OrePitBlockUpdate>,
    ) -> bool {
        let volume = self.volume();
        if volume == 0 {
            return true;
        }

        let mut changed = 0usize;
        while state.scanned < volume && changed < self.max_blocks_per_tick {
            let offset = state.cursor % volume;
            state.cursor = (state.cursor + 1) % volume;
            state.scanned += 1;

            let position = self.position_at(offset);
            let current_state = world
                .ore_pit_block_state_at(&self.dimension, &position)
                .unwrap_or_else(air_block_state);
            let key = BlockKey::from(&position);
            if !self.can_replace(current_state, &key, state) {
                continue;
            }

            let block_state = self.blocks.choose();
            world.ore_pit_set_runtime_block(&self.dimension, &position, block_state);
            state.generated_positions.insert(key);
            updates.push(OrePitBlockUpdate {
                dimension: self.dimension.clone(),
                position,
                block_state,
            });
            changed += 1;
        }
        state.scanned >= volume
    }

    fn can_replace(&self, current_state: i32, key: &BlockKey, state: &RuntimeOrePitState) -> bool {
        (self.replace_air && can_replace_block_state(current_state))
            || (self.replace_generated
                && state.generated_positions.contains(key)
                && self.blocks.contains_state(current_state))
    }

    fn refill_due(&self, state: &RuntimeOrePitState, now: Instant) -> bool {
        state.refill_in_progress
            || match state.last_tick {
                Some(last_tick) => now.duration_since(last_tick) >= self.tick_interval,
                None => true,
            }
    }

    fn surface_position(
        &self,
        world: &dyn OrePitWorld,
        position: EntityPosition,
    ) -> EntityPosition {
        let x = position
            .x
            .clamp(self.min_x as f64 + 0.5, self.max_x as f64 + 0.5);
        let z = position
            .z
            .clamp(self.min_z as f64 + 0.5, self.max_z as f64 + 0.5);
        let start_y = self.max_y.saturating_add(1);
        let y = (start_y..=start_y.saturating_add(16))
            .find(|y| player_space_is_clear(world, &self.dimension, x, *y, z))
            .unwrap_or(start_y);
        EntityPosition {
            x,
            y: y as f64,
            z,
            yaw: position.yaw,
            pitch: position.pitch,
            on_ground: false,
        }
    }

    fn volume(&self) -> usize {
        let x = i64::from(self.max_x - self.min_x + 1);
        let y = i64::from(self.max_y - self.min_y + 1);
        let z = i64::from(self.max_z - self.min_z + 1);
        usize::try_from(x.saturating_mul(y).saturating_mul(z)).unwrap_or(0)
    }

    fn position_at(&self, offset: usize) -> Position {
        let width_x = usize::try_from(self.max_x - self.min_x + 1).unwrap_or(1);
        let width_z = usize::try_from(self.max_z - self.min_z + 1).unwrap_or(1);
        let layer = width_x.saturating_mul(width_z).max(1);
        let y_offset = offset / layer;
        let layer_offset = offset % layer;
        let z_offset = layer_offset / width_x;
        let x_offset = layer_offset % width_x;
        Position {
            x: self.min_x + i32::try_from(x_offset).unwrap_or(0),
            y: self.min_y + i32::try_from(y_offset).unwrap_or(0),
            z: self.min_z + i32::try_from(z_offset).unwrap_or(0),
        }
    }

    fn contains(&self, dimension: &str, position: &Position) -> bool {
        self.dimension == dimension
            && (self.min_x..=self.max_x).contains(&position.x)
            && (self.min_y..=self.max_y).contains(&position.y)
            && (self.min_z..=self.max_z).contains(&position.z)
    }
}

fn player_space_is_clear(
    world: &dyn OrePitWorld,
    dimension: &str,
    x: f64,
    y: i32,
    z: f64,
) -> bool {
    let body = Position {
        x: x.floor() as i32,
        y,
        z: z.floor() as i32,
    };
    let head = Position {
        y: y.saturating_add(1),
        ..body.clone()
    };
    [body, head].into_iter().all(|position| {
        let state = world
            .ore_pit_block_state_at(dimension, &position)
            .unwrap_or_else(air_block_state);
        !block_has_collision(state)
    })
}

impl WeightedBlocks {
    fn from_config(blocks: &[WorldOrePitBlock]) -> Option<Self> {
        let states = blocks
            .iter()
            .filter_map(|block| {
                let weight = block.weight;
                if weight == 0 {
                    return None;
                }
                block_state_for_name(&block.block).map(|block_state| WeightedBlock {
                    block_state,
                    weight,
                })
            })
            .collect::<Vec<_>>();
        let total_weight = states.iter().map(|block| block.weight).sum();
        (total_weight > 0).then_some(Self {
            states,
            total_weight,
        })
    }

    fn choose(&self) -> i32 {
        let mut remaining = rand::random::<u32>() % self.total_weight;
        for block in &self.states {
            if remaining < block.weight {
                return block.block_state;
            }
            remaining -= block.weight;
        }
        self.states
            .last()
            .map(|block| block.block_state)
            .unwrap_or_else(air_block_state)
    }

    fn contains_state(&self, block_state: i32) -> bool {
        self.states
            .iter()
            .any(|weighted| weighted.block_state == block_state)
    }
}

impl From<&Position> for BlockKey {
    fn from(position: &Position) -> Self {
        Self {
            x: position.x,
            y: position.y,
            z: position.z,
        }
    }
}

fn ore_pit_id(config: &WorldOrePit) -> String {
    let id = config.id.trim();
    if !id.is_empty() {
        return id.to_string();
    }
    format!(
        "{}:{}:{}:{}:{}:{}:{}",
        config.dimension,
        config.min_x,
        config.max_x,
        config.min_y,
        config.max_y,
        config.min_z,
        config.max_z
    )
}

fn block_state_for_name(name: &str) -> Option<i32> {
    let name = normalize_block_name(name);
    crate::world::chunk_nbt::default_block_state_id_if_known(&name)
}

fn normalize_block_name(name: &str) -> String {
    let name = name.trim();
    if name.contains(':') {
        name.to_string()
    } else {
        format!("minecraft:{name}")
    }
}
