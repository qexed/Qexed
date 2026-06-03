use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
    time::{Duration, Instant},
};

use qexed_config::app::qexed::server::{WorldOrePit, WorldOrePitBlock};
use qexed_packet::net_types::Position;
use qexed_protocol::to_client::play::add_entity::EntityPosition;

use super::WorldManager;

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

    pub fn tick(&self, world: &WorldManager, now: Instant) -> Vec<OrePitBlockUpdate> {
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
        world: &WorldManager,
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
        world: &WorldManager,
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
                .block_state_at(&self.dimension, &position)
                .unwrap_or_else(crate::inventory::air_block_state);
            let key = BlockKey::from(&position);
            if !self.can_replace(current_state, &key, state) {
                continue;
            }

            let block_state = self.blocks.choose();
            world.set_runtime_block(&self.dimension, position.clone(), block_state);
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
        (self.replace_air && crate::inventory::can_replace_block_state(current_state))
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

    fn surface_position(&self, world: &WorldManager, position: EntityPosition) -> EntityPosition {
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

fn player_space_is_clear(world: &WorldManager, dimension: &str, x: f64, y: i32, z: f64) -> bool {
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
            .block_state_at(dimension, &position)
            .unwrap_or_else(crate::inventory::air_block_state);
        !crate::inventory::block_has_collision(state)
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
            .unwrap_or_else(crate::inventory::air_block_state)
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

#[cfg(test)]
mod tests {
    use qexed_config::app::qexed::server::{WorldOrePit, WorldOrePitBlock};

    use super::OrePitManager;

    #[test]
    fn ore_pit_fills_only_replaceable_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let world = crate::world::WorldManager::with_light_mode(
            dir.path(),
            crate::world::WorldLightMode::Static,
            crate::world::WorldLightAlgorithm::Fast,
            true,
        );
        let stone = crate::world::chunk_nbt::default_block_state_id("minecraft:stone");
        let diamond = crate::world::chunk_nbt::default_block_state_id("minecraft:diamond_ore");
        world.set_runtime_block(
            "minecraft:overworld",
            qexed_packet::net_types::Position { x: 1, y: 0, z: 0 },
            stone,
        );
        let manager = OrePitManager::from_config(&[WorldOrePit {
            id: "test".to_string(),
            dimension: "minecraft:overworld".to_string(),
            min_x: 0,
            max_x: 1,
            min_y: 0,
            max_y: 0,
            min_z: 0,
            max_z: 0,
            tick_interval_ms: 500,
            initial_refill: false,
            max_blocks_per_tick: 8,
            blocks: vec![WorldOrePitBlock {
                block: "minecraft:diamond_ore".to_string(),
                weight: 1,
            }],
            ..WorldOrePit::default()
        }]);

        let updates = manager.tick(&world, std::time::Instant::now());

        assert!(updates.is_empty());
        let updates = manager.tick(
            &world,
            std::time::Instant::now() + std::time::Duration::from_secs(1),
        );

        assert_eq!(updates.len(), 1);
        assert_eq!(
            world.block_state_at(
                "minecraft:overworld",
                &qexed_packet::net_types::Position { x: 0, y: 0, z: 0 }
            ),
            Some(diamond)
        );
        assert_eq!(
            world.block_state_at(
                "minecraft:overworld",
                &qexed_packet::net_types::Position { x: 1, y: 0, z: 0 }
            ),
            Some(stone)
        );
    }

    #[test]
    fn ore_pit_refills_on_first_tick_by_default() {
        let dir = tempfile::tempdir().unwrap();
        let world = crate::world::WorldManager::with_light_mode(
            dir.path(),
            crate::world::WorldLightMode::Static,
            crate::world::WorldLightAlgorithm::Fast,
            true,
        );
        let diamond = crate::world::chunk_nbt::default_block_state_id("minecraft:diamond_ore");
        let position = qexed_packet::net_types::Position { x: 0, y: 0, z: 0 };
        let manager = OrePitManager::from_config(&[WorldOrePit {
            id: "test".to_string(),
            dimension: "minecraft:overworld".to_string(),
            min_x: 0,
            max_x: 0,
            min_y: 0,
            max_y: 0,
            min_z: 0,
            max_z: 0,
            tick_interval_ms: 300_000,
            max_blocks_per_tick: 1,
            blocks: vec![WorldOrePitBlock {
                block: "minecraft:diamond_ore".to_string(),
                weight: 1,
            }],
            ..WorldOrePit::default()
        }]);

        let updates = manager.tick(&world, std::time::Instant::now());

        assert_eq!(updates.len(), 1);
        assert_eq!(
            world.block_state_at("minecraft:overworld", &position),
            Some(diamond)
        );
    }

    #[test]
    fn ore_pit_refill_period_scans_entire_pit_across_service_ticks() {
        let dir = tempfile::tempdir().unwrap();
        let world = crate::world::WorldManager::with_light_mode(
            dir.path(),
            crate::world::WorldLightMode::Static,
            crate::world::WorldLightAlgorithm::Fast,
            true,
        );
        let diamond = crate::world::chunk_nbt::default_block_state_id("minecraft:diamond_ore");
        let manager = OrePitManager::from_config(&[WorldOrePit {
            id: "test".to_string(),
            dimension: "minecraft:overworld".to_string(),
            min_x: 0,
            max_x: 2,
            min_y: 0,
            max_y: 0,
            min_z: 0,
            max_z: 0,
            tick_interval_ms: 300_000,
            initial_refill: true,
            max_blocks_per_tick: 1,
            blocks: vec![WorldOrePitBlock {
                block: "minecraft:diamond_ore".to_string(),
                weight: 1,
            }],
            ..WorldOrePit::default()
        }]);
        let now = std::time::Instant::now();

        assert_eq!(manager.tick(&world, now).len(), 1);
        assert_eq!(
            manager
                .tick(&world, now + std::time::Duration::from_millis(50))
                .len(),
            1
        );
        assert_eq!(
            manager
                .tick(&world, now + std::time::Duration::from_millis(100))
                .len(),
            1
        );
        assert_eq!(
            manager
                .tick(&world, now + std::time::Duration::from_millis(150))
                .len(),
            0
        );

        for x in 0..=2 {
            assert_eq!(
                world.block_state_at(
                    "minecraft:overworld",
                    &qexed_packet::net_types::Position { x, y: 0, z: 0 }
                ),
                Some(diamond)
            );
        }
    }

    #[test]
    fn ore_pit_refills_generated_ore_after_break() {
        let dir = tempfile::tempdir().unwrap();
        let world = crate::world::WorldManager::with_light_mode(
            dir.path(),
            crate::world::WorldLightMode::Static,
            crate::world::WorldLightAlgorithm::Fast,
            true,
        );
        let diamond = crate::world::chunk_nbt::default_block_state_id("minecraft:diamond_ore");
        let air = crate::inventory::air_block_state();
        let position = qexed_packet::net_types::Position { x: 0, y: 0, z: 0 };
        let manager = OrePitManager::from_config(&[WorldOrePit {
            id: "test".to_string(),
            dimension: "minecraft:overworld".to_string(),
            min_x: 0,
            max_x: 0,
            min_y: 0,
            max_y: 0,
            min_z: 0,
            max_z: 0,
            tick_interval_ms: 500,
            initial_refill: false,
            max_blocks_per_tick: 1,
            blocks: vec![WorldOrePitBlock {
                block: "diamond_ore".to_string(),
                weight: 1,
            }],
            ..WorldOrePit::default()
        }]);

        let now = std::time::Instant::now();
        assert!(manager.tick(&world, now).is_empty());
        assert_eq!(
            manager
                .tick(&world, now + std::time::Duration::from_secs(1))
                .len(),
            1
        );
        world.set_runtime_block("minecraft:overworld", position.clone(), air);

        let updates = manager.tick(&world, now + std::time::Duration::from_secs(2));

        assert_eq!(updates.len(), 1);
        assert_eq!(
            world.block_state_at("minecraft:overworld", &position),
            Some(diamond)
        );
    }

    #[test]
    fn ore_pit_can_restrict_breaking_to_generated_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let world = crate::world::WorldManager::with_light_mode(
            dir.path(),
            crate::world::WorldLightMode::Static,
            crate::world::WorldLightAlgorithm::Fast,
            true,
        );
        let stone = crate::world::chunk_nbt::default_block_state_id("minecraft:stone");
        let diamond = crate::world::chunk_nbt::default_block_state_id("minecraft:diamond_ore");
        let position = qexed_packet::net_types::Position { x: 0, y: 0, z: 0 };
        let native = qexed_packet::net_types::Position { x: 1, y: 0, z: 0 };
        world.set_runtime_block("minecraft:overworld", native.clone(), stone);
        let manager = OrePitManager::from_config(&[WorldOrePit {
            id: "test".to_string(),
            dimension: "minecraft:overworld".to_string(),
            min_x: 0,
            max_x: 1,
            min_y: 0,
            max_y: 0,
            min_z: 0,
            max_z: 0,
            tick_interval_ms: 500,
            initial_refill: false,
            max_blocks_per_tick: 8,
            only_break_generated: true,
            blocks: vec![WorldOrePitBlock {
                block: "minecraft:diamond_ore".to_string(),
                weight: 1,
            }],
            ..WorldOrePit::default()
        }]);
        let now = std::time::Instant::now();

        manager.tick(&world, now);
        manager.tick(&world, now + std::time::Duration::from_secs(1));

        assert!(manager.permits_player_break("minecraft:overworld", &position, diamond));
        assert!(!manager.permits_player_break("minecraft:overworld", &native, stone));
    }

    #[test]
    fn ore_pit_evacuation_target_is_available_when_refill_is_due() {
        let dir = tempfile::tempdir().unwrap();
        let world = crate::world::WorldManager::with_light_mode(
            dir.path(),
            crate::world::WorldLightMode::Static,
            crate::world::WorldLightAlgorithm::Fast,
            true,
        );
        let manager = OrePitManager::from_config(&[WorldOrePit {
            id: "test".to_string(),
            dimension: "minecraft:overworld".to_string(),
            min_x: 0,
            max_x: 4,
            min_y: 0,
            max_y: 4,
            min_z: 0,
            max_z: 4,
            tick_interval_ms: 300_000,
            initial_refill: true,
            teleport_players_to_surface_on_refill: true,
            blocks: vec![WorldOrePitBlock {
                block: "minecraft:diamond_ore".to_string(),
                weight: 1,
            }],
            ..WorldOrePit::default()
        }]);
        let position = qexed_protocol::to_client::play::add_entity::EntityPosition {
            x: 2.5,
            y: 2.0,
            z: 2.5,
            yaw: 90.0,
            pitch: 0.0,
            on_ground: true,
        };

        let target = manager
            .evacuation_target_for_player(
                &world,
                "minecraft:overworld",
                position,
                std::time::Instant::now(),
            )
            .expect("player inside a due ore pit should be evacuated");

        assert_eq!(target.y, 5.0);
        assert_eq!(target.x, 2.5);
        assert_eq!(target.z, 2.5);
        assert_eq!(target.yaw, 90.0);
    }

    #[test]
    fn ore_pit_evacuation_target_skips_solid_surface_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let world = crate::world::WorldManager::with_light_mode(
            dir.path(),
            crate::world::WorldLightMode::Static,
            crate::world::WorldLightAlgorithm::Fast,
            true,
        );
        let stone = crate::world::chunk_nbt::default_block_state_id("minecraft:stone");
        world.set_runtime_block(
            "minecraft:overworld",
            qexed_packet::net_types::Position { x: 2, y: 5, z: 2 },
            stone,
        );
        let manager = OrePitManager::from_config(&[WorldOrePit {
            id: "test".to_string(),
            dimension: "minecraft:overworld".to_string(),
            min_x: 0,
            max_x: 4,
            min_y: 0,
            max_y: 4,
            min_z: 0,
            max_z: 4,
            tick_interval_ms: 300_000,
            initial_refill: true,
            teleport_players_to_surface_on_refill: true,
            blocks: vec![WorldOrePitBlock {
                block: "minecraft:diamond_ore".to_string(),
                weight: 1,
            }],
            ..WorldOrePit::default()
        }]);
        let position = qexed_protocol::to_client::play::add_entity::EntityPosition {
            x: 2.5,
            y: 2.0,
            z: 2.5,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        };

        let target = manager
            .evacuation_target_for_player(
                &world,
                "minecraft:overworld",
                position,
                std::time::Instant::now(),
            )
            .expect("player inside a due ore pit should be evacuated");

        assert_eq!(target.y, 6.0);
    }
}
