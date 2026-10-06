use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use qexed_packet::net_types::Position as BlockPosition;
use qexed_protocol::types::EntityPosition;

#[derive(Debug, Clone, PartialEq)]
pub(in crate::gameplay) struct RedstoneBlockChange {
    pub(in crate::gameplay) dimension: String,
    pub(in crate::gameplay) position: BlockPosition,
    pub(in crate::gameplay) block_state: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::gameplay) struct RedstoneInteraction {
    pub(in crate::gameplay) block_state: i32,
    pub(in crate::gameplay) button_release_ms: Option<u64>,
}

#[derive(Debug, Default)]
pub(in crate::gameplay) struct RedstoneRuntime {
    button_releases: Vec<ScheduledRedstoneUpdate>,
    scheduled_updates: Vec<ScheduledBlockStateUpdate>,
    tnt_explosions: Vec<ScheduledTntExplosion>,
    ready_scheduled_updates: HashSet<ScheduledBlockStateKey>,
    powered_pressure_plates: HashSet<RedstoneBlockKey>,
    powered_tripwires: HashSet<RedstoneBlockKey>,
    daylight_due: Option<Instant>,
}

#[derive(Debug, Clone)]
struct ScheduledRedstoneUpdate {
    due: Instant,
    key: RedstoneBlockKey,
}

#[derive(Debug, Clone)]
struct ScheduledBlockStateUpdate {
    due: Instant,
    key: RedstoneBlockKey,
    block_state: i32,
}

#[derive(Debug, Clone)]
struct ScheduledTntExplosion {
    due: Instant,
    key: RedstoneBlockKey,
    radius: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ScheduledBlockStateKey {
    key: RedstoneBlockKey,
    block_state: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RedstoneBlockKey {
    dimension: String,
    x: i32,
    y: i32,
    z: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PosKey {
    x: i32,
    y: i32,
    z: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Direction {
    dx: i32,
    dy: i32,
    dz: i32,
}

#[derive(Debug, Clone)]
struct TripwireHookScan {
    position: BlockPosition,
    state: i32,
    wires: Vec<(BlockPosition, i32)>,
}

const DIRECTIONS: [Direction; 6] = [
    Direction {
        dx: 1,
        dy: 0,
        dz: 0,
    },
    Direction {
        dx: -1,
        dy: 0,
        dz: 0,
    },
    Direction {
        dx: 0,
        dy: 1,
        dz: 0,
    },
    Direction {
        dx: 0,
        dy: -1,
        dz: 0,
    },
    Direction {
        dx: 0,
        dy: 0,
        dz: 1,
    },
    Direction {
        dx: 0,
        dy: 0,
        dz: -1,
    },
];

const HORIZONTAL_DIRECTIONS: [(Direction, &str); 4] = [
    (
        Direction {
            dx: 1,
            dy: 0,
            dz: 0,
        },
        "east",
    ),
    (
        Direction {
            dx: -1,
            dy: 0,
            dz: 0,
        },
        "west",
    ),
    (
        Direction {
            dx: 0,
            dy: 0,
            dz: 1,
        },
        "south",
    ),
    (
        Direction {
            dx: 0,
            dy: 0,
            dz: -1,
        },
        "north",
    ),
];

const TRIPWIRE_MAX_SCAN: i32 = 42;

impl Direction {
    fn opposite(self) -> Self {
        Self {
            dx: -self.dx,
            dy: -self.dy,
            dz: -self.dz,
        }
    }
}

impl RedstoneRuntime {
    pub(in crate::gameplay) fn new() -> Self {
        Self::default()
    }

    pub(in crate::gameplay) fn schedule_button_release(
        &mut self,
        dimension: impl Into<String>,
        position: BlockPosition,
        delay_ms: u64,
    ) {
        self.button_releases.push(ScheduledRedstoneUpdate {
            due: Instant::now() + Duration::from_millis(delay_ms.max(50)),
            key: RedstoneBlockKey::new(dimension, &position),
        });
    }

    pub(in crate::gameplay) fn schedule_tnt_explosion(
        &mut self,
        dimension: impl Into<String>,
        position: BlockPosition,
        delay_ms: u64,
    ) {
        let key = RedstoneBlockKey::new(dimension, &position);
        self.tnt_explosions.retain(|pending| pending.key != key);
        self.tnt_explosions.push(ScheduledTntExplosion {
            due: Instant::now() + Duration::from_millis(delay_ms.max(50)),
            key,
            radius: 4.0,
        });
    }

    pub(in crate::gameplay) fn should_delay_redstone_update(
        &mut self,
        current_state: i32,
        update: &RedstoneBlockChange,
    ) -> bool {
        let key = RedstoneBlockKey::new(update.dimension.clone(), &update.position);
        if self
            .ready_scheduled_updates
            .remove(&ScheduledBlockStateKey {
                key: key.clone(),
                block_state: update.block_state,
            })
        {
            return false;
        }

        let current_name = block_name(current_state);
        let update_name = block_name(update.block_state);
        let delay_ticks = if current_name == "minecraft:repeater"
            && update_name == "minecraft:repeater"
            && !property_bool(current_state, "locked")
            && property_bool(current_state, "powered")
                != property_bool(update.block_state, "powered")
        {
            property_value(current_state, "delay")
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(1)
                .clamp(1, 4)
        } else if current_name == "minecraft:redstone_lamp"
            && update_name == "minecraft:redstone_lamp"
            && property_bool(current_state, "lit")
            && !property_bool(update.block_state, "lit")
        {
            2
        } else {
            return false;
        };

        self.scheduled_updates.retain(|pending| pending.key != key);
        self.scheduled_updates.push(ScheduledBlockStateUpdate {
            due: Instant::now() + Duration::from_millis(delay_ticks * 50),
            key,
            block_state: update.block_state,
        });
        true
    }

    pub(in crate::gameplay) fn tick(
        &mut self,
        world: &dyn crate::world_access::WorldBlockSource,
        world_rules: &qexed_world::world::WorldRulesManager,
        players: &qexed_player::PlayerManager,
        gameplay: &crate::config::GameplayConfig,
    ) -> Vec<RedstoneBlockChange> {
        let mut changes = Vec::new();
        self.collect_due_scheduled_updates(world, &mut changes);
        self.collect_due_tnt_explosions(world, &mut changes);
        self.collect_due_button_releases(world, &mut changes);
        self.collect_pressure_plate_updates(world, players, &mut changes);
        self.collect_tripwire_updates(world, players, &mut changes);
        if self.should_tick_daylight_detectors() {
            self.collect_daylight_detector_updates(
                world,
                world_rules,
                players,
                gameplay,
                &mut changes,
            );
        }
        changes
    }

    pub(in crate::gameplay) fn observer_updates_after_block_changes(
        &mut self,
        world: &dyn crate::world_access::WorldBlockSource,
        dimension: &str,
        positions: &[BlockPosition],
    ) -> Vec<RedstoneBlockChange> {
        let mut changes = Vec::new();
        let mut seen = HashSet::new();
        for changed in positions {
            for facing in DIRECTIONS {
                let observer_position = offset(changed, -facing.dx, -facing.dy, -facing.dz);
                let key = RedstoneBlockKey::new(dimension.to_string(), &observer_position);
                if !seen.insert(key.clone()) {
                    continue;
                }
                let Some(state) = world.block_state_at(dimension, &observer_position) else {
                    continue;
                };
                if !is_observer(&block_name(state))
                    || facing_direction(state) != Some(facing)
                    || property_bool(state, "powered")
                {
                    continue;
                }
                let Some(powered) = block_state_with_property(state, "powered", "true") else {
                    continue;
                };
                let Some(unpowered) = block_state_with_property(powered, "powered", "false") else {
                    continue;
                };
                changes.push(RedstoneBlockChange {
                    dimension: dimension.to_string(),
                    position: observer_position,
                    block_state: powered,
                });
                self.scheduled_updates.push(ScheduledBlockStateUpdate {
                    due: Instant::now() + Duration::from_millis(100),
                    key,
                    block_state: unpowered,
                });
            }
        }
        changes
    }

    fn should_tick_daylight_detectors(&mut self) -> bool {
        let now = Instant::now();
        if self.daylight_due.is_some_and(|due| due > now) {
            return false;
        }
        self.daylight_due = Some(now + Duration::from_secs(1));
        true
    }

    fn collect_due_button_releases(
        &mut self,
        world: &dyn crate::world_access::WorldBlockSource,
        changes: &mut Vec<RedstoneBlockChange>,
    ) {
        let now = Instant::now();
        let mut pending = Vec::new();
        for release in self.button_releases.drain(..) {
            if release.due > now {
                pending.push(release);
                continue;
            }
            let position = release.key.position();
            let Some(state) = world.block_state_at(&release.key.dimension, &position) else {
                continue;
            };
            let name = block_name(state);
            if is_button(&name)
                && property_bool(state, "powered")
                && let Some(next) = block_state_with_property(state, "powered", "false")
            {
                changes.push(RedstoneBlockChange {
                    dimension: release.key.dimension,
                    position,
                    block_state: next,
                });
            }
        }
        self.button_releases = pending;
    }

    fn collect_due_scheduled_updates(
        &mut self,
        world: &dyn crate::world_access::WorldBlockSource,
        changes: &mut Vec<RedstoneBlockChange>,
    ) {
        let now = Instant::now();
        let mut pending = Vec::new();
        for update in self.scheduled_updates.drain(..) {
            if update.due > now {
                pending.push(update);
                continue;
            }
            let position = update.key.position();
            if world
                .block_state_at(&update.key.dimension, &position)
                .is_some_and(|current| current == update.block_state)
            {
                continue;
            }
            self.ready_scheduled_updates.insert(ScheduledBlockStateKey {
                key: update.key.clone(),
                block_state: update.block_state,
            });
            changes.push(RedstoneBlockChange {
                dimension: update.key.dimension,
                position,
                block_state: update.block_state,
            });
        }
        self.scheduled_updates = pending;
    }

    fn collect_due_tnt_explosions(
        &mut self,
        world: &dyn crate::world_access::WorldBlockSource,
        changes: &mut Vec<RedstoneBlockChange>,
    ) {
        let now = Instant::now();
        let mut pending = Vec::new();
        let mut changed = HashSet::new();
        for explosion in self.tnt_explosions.drain(..) {
            if explosion.due > now {
                pending.push(explosion);
                continue;
            }
            for change in tnt_explosion_air_blocks(world, &explosion) {
                if changed.insert(RedstoneBlockKey::new(
                    change.dimension.clone(),
                    &change.position,
                )) {
                    changes.push(change);
                }
            }
        }
        self.tnt_explosions = pending;
    }

    fn collect_pressure_plate_updates(
        &mut self,
        world: &dyn crate::world_access::WorldBlockSource,
        players: &qexed_player::PlayerManager,
        changes: &mut Vec<RedstoneBlockChange>,
    ) {
        let mut active = HashMap::<RedstoneBlockKey, (BlockPosition, i32, u32)>::new();
        for player in players.list_except(uuid::Uuid::nil()) {
            let Some((position, state)) =
                pressure_plate_under_player(world, &player.dimension, player.position)
            else {
                continue;
            };
            let key = RedstoneBlockKey::new(player.dimension.clone(), &position);
            active
                .entry(key)
                .and_modify(|(_, _, count)| *count = count.saturating_add(1))
                .or_insert((position, state, 1));
        }

        for (key, (position, state, count)) in &active {
            if let Some(powered) = pressure_plate_powered_state(*state, *count) {
                changes.push(RedstoneBlockChange {
                    dimension: key.dimension.clone(),
                    position: position.clone(),
                    block_state: powered,
                });
            }
        }

        for key in self.powered_pressure_plates.drain() {
            if active.contains_key(&key) {
                continue;
            }
            let position = key.position();
            let Some(state) = world.block_state_at(&key.dimension, &position) else {
                continue;
            };
            if let Some(unpowered) = pressure_plate_powered_state(state, 0) {
                changes.push(RedstoneBlockChange {
                    dimension: key.dimension,
                    position,
                    block_state: unpowered,
                });
            }
        }
        self.powered_pressure_plates = active.into_keys().collect();
    }

    fn collect_tripwire_updates(
        &mut self,
        world: &dyn crate::world_access::WorldBlockSource,
        players: &qexed_player::PlayerManager,
        changes: &mut Vec<RedstoneBlockChange>,
    ) {
        let mut active = HashSet::new();
        for player in players.list_except(uuid::Uuid::nil()) {
            let Some((position, state)) =
                tripwire_at_player(world, &player.dimension, player.position)
            else {
                continue;
            };
            if property_bool(state, "disarmed") {
                continue;
            }
            let key = RedstoneBlockKey::new(player.dimension.clone(), &position);
            active.insert(key);
        }

        let previous = self.powered_tripwires.drain().collect::<HashSet<_>>();
        let touched = active
            .iter()
            .chain(previous.iter())
            .cloned()
            .collect::<HashSet<_>>();
        let mut line_changes = HashMap::new();
        for key in &touched {
            let position = key.position();
            let synced_line = collect_tripwire_line_updates(
                world,
                &key.dimension,
                &position,
                &active,
                &mut line_changes,
            );
            if synced_line {
                continue;
            }
            let Some(state) = world.block_state_at(&key.dimension, &position) else {
                continue;
            };
            if !is_tripwire(&block_name(state)) {
                continue;
            }
            let desired_powered = active.contains(key);
            if let Some(next) =
                block_state_with_property(state, "powered", bool_value(desired_powered))
            {
                insert_redstone_change(
                    &mut line_changes,
                    RedstoneBlockChange {
                        dimension: key.dimension.clone(),
                        position,
                        block_state: next,
                    },
                );
            }
        }

        changes.extend(line_changes.into_values());
        self.powered_tripwires = active;
    }

    fn collect_daylight_detector_updates(
        &self,
        world: &dyn crate::world_access::WorldBlockSource,
        world_rules: &qexed_world::world::WorldRulesManager,
        players: &qexed_player::PlayerManager,
        gameplay: &crate::config::GameplayConfig,
        changes: &mut Vec<RedstoneBlockChange>,
    ) {
        let radius = i32::try_from(gameplay.redstone_max_distance.clamp(1, 8)).unwrap_or(8);
        let vertical_radius = 4;
        let mut seen = HashSet::new();
        for player in players.list_except(uuid::Uuid::nil()) {
            let base = BlockPosition {
                x: player.position.x.floor() as i32,
                y: player.position.y.floor() as i32,
                z: player.position.z.floor() as i32,
            };
            for x in base.x.saturating_sub(radius)..=base.x.saturating_add(radius) {
                for y in
                    base.y.saturating_sub(vertical_radius)..=base.y.saturating_add(vertical_radius)
                {
                    for z in base.z.saturating_sub(radius)..=base.z.saturating_add(radius) {
                        let position = BlockPosition { x, y, z };
                        let key = RedstoneBlockKey::new(player.dimension.clone(), &position);
                        if !seen.insert(key.clone()) {
                            continue;
                        }
                        let Some(state) = world.block_state_at(&key.dimension, &position) else {
                            continue;
                        };
                        if !is_daylight_detector(&block_name(state)) {
                            continue;
                        }
                        let power = daylight_detector_power(
                            world_rules.current_time(&key.dimension),
                            property_bool(state, "inverted"),
                        );
                        let Some(next) = set_power_level(state, power) else {
                            continue;
                        };
                        if next != state {
                            changes.push(RedstoneBlockChange {
                                dimension: key.dimension,
                                position,
                                block_state: next,
                            });
                        }
                    }
                }
            }
        }
    }
}

pub(in crate::gameplay) fn interaction_for_block_state(
    block_state: i32,
    block_name: &str,
) -> Option<RedstoneInteraction> {
    if is_lever(block_name) {
        let powered = property_bool(block_state, "powered");
        return block_state_with_property(
            block_state,
            "powered",
            if powered { "false" } else { "true" },
        )
        .map(|block_state| RedstoneInteraction {
            block_state,
            button_release_ms: None,
        });
    }

    if is_button(block_name) {
        if property_bool(block_state, "powered") {
            return None;
        }
        return block_state_with_property(block_state, "powered", "true").map(|block_state| {
            RedstoneInteraction {
                block_state,
                button_release_ms: Some(button_release_ms(block_name)),
            }
        });
    }

    None
}

pub(in crate::gameplay) fn updates_after_block_changes(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    origins: &[BlockPosition],
    max_distance: u32,
) -> Vec<RedstoneBlockChange> {
    if origins.is_empty() {
        return Vec::new();
    }
    let candidates = collect_redstone_candidates(world, dimension, origins, max_distance);
    if candidates.is_empty() {
        return Vec::new();
    }

    let mut overlay = HashMap::<PosKey, i32>::new();
    for position in &candidates {
        if let Some(state) = world.block_state_at(dimension, position) {
            overlay.insert(PosKey::from(position), state);
        }
    }

    let max_iterations = max_distance.max(1).min(64) as usize;
    for _ in 0..max_iterations {
        let mut changed = false;
        let mut next_overlay = overlay.clone();
        for position in &candidates {
            let key = PosKey::from(position);
            let Some(state) = overlay.get(&key).copied() else {
                continue;
            };
            let name = block_name(state);
            let Some(next_state) =
                next_redstone_state(world, dimension, &overlay, position, state, &name)
            else {
                continue;
            };
            if next_state != state {
                next_overlay.insert(key, next_state);
                changed = true;
            }
        }
        overlay = next_overlay;
        if !changed {
            break;
        }
    }

    candidates
        .into_iter()
        .filter_map(|position| {
            let key = PosKey::from(&position);
            let next = overlay.get(&key).copied()?;
            let current = world.block_state_at(dimension, &position)?;
            (next != current).then(|| RedstoneBlockChange {
                dimension: dimension.to_string(),
                position,
                block_state: next,
            })
        })
        .collect()
}

fn collect_redstone_candidates(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    origins: &[BlockPosition],
    max_distance: u32,
) -> Vec<BlockPosition> {
    let radius = i32::try_from(max_distance.max(1).min(64)).unwrap_or(32);
    let vertical_radius = 3;
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for origin in origins {
        for x in origin.x.saturating_sub(radius)..=origin.x.saturating_add(radius) {
            for y in
                origin.y.saturating_sub(vertical_radius)..=origin.y.saturating_add(vertical_radius)
            {
                for z in origin.z.saturating_sub(radius)..=origin.z.saturating_add(radius) {
                    if (x - origin.x).abs() + (z - origin.z).abs() > radius + 2 {
                        continue;
                    }
                    let position = BlockPosition { x, y, z };
                    let key = PosKey::from(&position);
                    if !seen.insert(key) {
                        continue;
                    }
                    let Some(state) = world.block_state_at(dimension, &position) else {
                        continue;
                    };
                    let name = block_name(state);
                    if is_redstone_reactive(state, &name) {
                        out.push(position);
                    }
                }
            }
        }
    }
    out
}

fn next_redstone_state(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    overlay: &HashMap<PosKey, i32>,
    position: &BlockPosition,
    block_state: i32,
    block_name: &str,
) -> Option<i32> {
    match block_name {
        "minecraft:redstone_wire" => {
            let power = redstone_wire_power(world, dimension, overlay, position);
            redstone_wire_state(world, dimension, overlay, position, block_state, power)
        }
        "minecraft:redstone_lamp" => {
            let lit = neighbor_power(world, dimension, overlay, position) > 0;
            block_state_with_property(block_state, "lit", bool_value(lit))
        }
        "minecraft:redstone_torch" | "minecraft:redstone_wall_torch" => {
            let lit = !torch_attached_block_powered(
                world,
                dimension,
                overlay,
                position,
                block_state,
                block_name,
            );
            block_state_with_property(block_state, "lit", bool_value(lit))
        }
        name if is_repeater(name) => {
            repeater_state(world, dimension, overlay, position, block_state)
        }
        name if is_comparator(name) => {
            comparator_state(world, dimension, overlay, position, block_state)
        }
        "minecraft:tnt" => (neighbor_power(world, dimension, overlay, position) > 0)
            .then(crate::inventory::air_block_state),
        name if is_redstone_driven_device(name, block_state) => {
            let powered =
                driven_device_input_power(world, dimension, overlay, position, block_state, name)
                    > 0;
            redstone_driven_device_state(block_state, name, powered)
        }
        _ => None,
    }
}

fn redstone_wire_state(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    overlay: &HashMap<PosKey, i32>,
    position: &BlockPosition,
    block_state: i32,
    power: u8,
) -> Option<i32> {
    let mut next = set_power_level(block_state, power)?;
    for (direction, property) in HORIZONTAL_DIRECTIONS {
        let connection = redstone_wire_connection(world, dimension, overlay, position, direction);
        next = block_state_with_property(next, property, connection).unwrap_or(next);
    }
    Some(next)
}

fn redstone_wire_connection(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    overlay: &HashMap<PosKey, i32>,
    position: &BlockPosition,
    direction: Direction,
) -> &'static str {
    let side = offset(position, direction.dx, 0, direction.dz);
    if redstone_wire_connects_to(world, dimension, overlay, &side, direction) {
        return "side";
    }

    let upper = offset(&side, 0, 1, 0);
    if redstone_wire_connects_to(world, dimension, overlay, &upper, direction) {
        return "up";
    }

    let lower = offset(&side, 0, -1, 0);
    if redstone_wire_connects_to(world, dimension, overlay, &lower, direction) {
        return "side";
    }

    "none"
}

fn redstone_wire_connects_to(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    overlay: &HashMap<PosKey, i32>,
    position: &BlockPosition,
    direction_to_neighbor: Direction,
) -> bool {
    let Some(state) = state_at(world, dimension, overlay, position) else {
        return false;
    };
    let name = block_name(state);
    match name.as_str() {
        "minecraft:redstone_wire" | "minecraft:redstone_block" => true,
        "minecraft:redstone_torch" | "minecraft:redstone_wall_torch" => {
            redstone_torch_outputs_to(state, &name, direction_to_neighbor.opposite())
        }
        name if is_repeater(name) || is_comparator(name) => {
            facing_direction(state).is_some_and(|facing| {
                facing == direction_to_neighbor || facing == direction_to_neighbor.opposite()
            })
        }
        name if is_observer(name) => facing_direction(state).is_some_and(|facing| {
            facing == direction_to_neighbor || facing == direction_to_neighbor.opposite()
        }),
        name if is_powered_control(name) => true,
        name if is_redstone_driven_device(name, state) => true,
        _ => false,
    }
}

fn redstone_wire_power(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    overlay: &HashMap<PosKey, i32>,
    position: &BlockPosition,
) -> u8 {
    let mut power = 0u8;
    for direction in DIRECTIONS {
        let source = offset(position, -direction.dx, -direction.dy, -direction.dz);
        power = power.max(redstone_wire_source_power(
            world, dimension, overlay, &source, direction,
        ));
    }
    for (direction, _) in HORIZONTAL_DIRECTIONS {
        let upper = offset(position, -direction.dx, 1, -direction.dz);
        power = power.max(redstone_wire_source_power(
            world, dimension, overlay, &upper, direction,
        ));
        let lower = offset(position, -direction.dx, -1, -direction.dz);
        power = power.max(redstone_wire_source_power(
            world, dimension, overlay, &lower, direction,
        ));
    }
    power.min(15)
}

fn redstone_wire_source_power(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    overlay: &HashMap<PosKey, i32>,
    source: &BlockPosition,
    direction_to_target: Direction,
) -> u8 {
    let candidate = source_power_to(world, dimension, overlay, source, direction_to_target);
    if state_at(world, dimension, overlay, source)
        .is_some_and(|state| block_name(state) == "minecraft:redstone_wire")
    {
        candidate.saturating_sub(1)
    } else {
        candidate
    }
}

fn neighbor_power(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    overlay: &HashMap<PosKey, i32>,
    position: &BlockPosition,
) -> u8 {
    DIRECTIONS
        .iter()
        .map(|direction| {
            let source = offset(position, -direction.dx, -direction.dy, -direction.dz);
            source_power_to(world, dimension, overlay, &source, *direction)
        })
        .max()
        .unwrap_or(0)
}

fn directional_input_power(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    overlay: &HashMap<PosKey, i32>,
    position: &BlockPosition,
    block_state: i32,
) -> u8 {
    let facing = facing_direction(block_state).unwrap_or(Direction {
        dx: 0,
        dy: 0,
        dz: -1,
    });
    let source = offset(position, -facing.dx, -facing.dy, -facing.dz);
    source_power_to(world, dimension, overlay, &source, facing)
}

fn repeater_state(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    overlay: &HashMap<PosKey, i32>,
    position: &BlockPosition,
    block_state: i32,
) -> Option<i32> {
    let locked = repeater_locked(world, dimension, overlay, position, block_state);
    let powered = if locked {
        property_bool(block_state, "powered")
    } else {
        directional_input_power(world, dimension, overlay, position, block_state) > 0
    };
    let mut next = block_state_with_property(block_state, "powered", bool_value(powered))?;
    if has_property(next, "locked") {
        next = block_state_with_property(next, "locked", bool_value(locked)).unwrap_or(next);
    }
    Some(next)
}

fn comparator_state(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    overlay: &HashMap<PosKey, i32>,
    position: &BlockPosition,
    block_state: i32,
) -> Option<i32> {
    let powered = comparator_output_power(world, dimension, overlay, position, block_state) > 0;
    block_state_with_property(block_state, "powered", bool_value(powered))
}

fn repeater_locked(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    overlay: &HashMap<PosKey, i32>,
    position: &BlockPosition,
    block_state: i32,
) -> bool {
    let Some(facing) = facing_direction(block_state) else {
        return false;
    };
    perpendicular_directions(facing)
        .into_iter()
        .any(|direction| {
            let source = offset(position, -direction.dx, -direction.dy, -direction.dz);
            side_lock_source_powered(world, dimension, overlay, &source, direction)
        })
}

fn side_lock_source_powered(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    overlay: &HashMap<PosKey, i32>,
    source: &BlockPosition,
    direction_to_target: Direction,
) -> bool {
    let Some(state) = state_at(world, dimension, overlay, source) else {
        return false;
    };
    let name = block_name(state);
    (is_repeater(&name) || is_comparator(&name))
        && source_direct_power_to(
            world,
            dimension,
            overlay,
            source,
            state,
            &name,
            direction_to_target,
            &mut HashSet::new(),
        ) > 0
}

fn perpendicular_directions(facing: Direction) -> [Direction; 2] {
    if facing.dx != 0 {
        [
            Direction {
                dx: 0,
                dy: 0,
                dz: 1,
            },
            Direction {
                dx: 0,
                dy: 0,
                dz: -1,
            },
        ]
    } else {
        [
            Direction {
                dx: 1,
                dy: 0,
                dz: 0,
            },
            Direction {
                dx: -1,
                dy: 0,
                dz: 0,
            },
        ]
    }
}

fn driven_device_input_power(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    overlay: &HashMap<PosKey, i32>,
    position: &BlockPosition,
    block_state: i32,
    block_name: &str,
) -> u8 {
    let mut power = neighbor_power(world, dimension, overlay, position);
    if is_door_device(block_name) {
        let other_half = if property_value(block_state, "half").as_deref() == Some("upper") {
            offset(position, 0, -1, 0)
        } else {
            offset(position, 0, 1, 0)
        };
        power = power.max(neighbor_power(world, dimension, overlay, &other_half));
    }
    power
}

fn torch_attached_block_powered(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    overlay: &HashMap<PosKey, i32>,
    position: &BlockPosition,
    block_state: i32,
    block_name: &str,
) -> bool {
    let attached = if block_name == "minecraft:redstone_wall_torch" {
        let facing = facing_direction(block_state).unwrap_or(Direction {
            dx: 0,
            dy: 0,
            dz: -1,
        });
        offset(position, -facing.dx, -facing.dy, -facing.dz)
    } else {
        offset(position, 0, -1, 0)
    };
    neighbor_power(world, dimension, overlay, &attached) > 0
}

fn source_power_to(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    overlay: &HashMap<PosKey, i32>,
    source: &BlockPosition,
    direction_to_target: Direction,
) -> u8 {
    source_power_to_inner(
        world,
        dimension,
        overlay,
        source,
        direction_to_target,
        &mut HashSet::new(),
    )
}

fn source_power_to_inner(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    overlay: &HashMap<PosKey, i32>,
    source: &BlockPosition,
    direction_to_target: Direction,
    active_comparators: &mut HashSet<PosKey>,
) -> u8 {
    let Some(state) = state_at(world, dimension, overlay, source) else {
        return 0;
    };
    let name = block_name(state);
    let direct = source_direct_power_to(
        world,
        dimension,
        overlay,
        source,
        state,
        &name,
        direction_to_target,
        active_comparators,
    );
    if direct > 0 {
        return direct;
    }
    if is_redstone_conductor(state, &name) {
        return conducted_power_to(
            world,
            dimension,
            overlay,
            source,
            direction_to_target,
            active_comparators,
        );
    }
    0
}

#[allow(clippy::too_many_arguments)]
fn source_direct_power_to(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    overlay: &HashMap<PosKey, i32>,
    source: &BlockPosition,
    state: i32,
    name: &str,
    direction_to_target: Direction,
    active_comparators: &mut HashSet<PosKey>,
) -> u8 {
    if is_comparator(name) {
        return if property_bool(state, "powered")
            && facing_direction(state).is_some_and(|facing| facing == direction_to_target)
        {
            comparator_output_power_inner(
                world,
                dimension,
                overlay,
                source,
                state,
                active_comparators,
            )
        } else {
            0
        };
    }

    direct_source_power_to(state, name, direction_to_target)
}

fn direct_source_power_to(state: i32, name: &str, direction_to_target: Direction) -> u8 {
    match name {
        "minecraft:redstone_block" => 15,
        "minecraft:redstone_wire" => power_level(state),
        "minecraft:redstone_torch" | "minecraft:redstone_wall_torch" => {
            if property_bool(state, "lit")
                && redstone_torch_outputs_to(state, name, direction_to_target)
            {
                15
            } else {
                0
            }
        }
        name if is_repeater(name) => {
            if property_bool(state, "powered")
                && facing_direction(state).is_some_and(|facing| facing == direction_to_target)
            {
                15
            } else {
                0
            }
        }
        name if is_observer(name) => {
            if property_bool(state, "powered")
                && facing_direction(state)
                    .is_some_and(|facing| direction_to_target == facing.opposite())
            {
                15
            } else {
                0
            }
        }
        name if is_powered_control(name) => powered_control_level(state),
        _ => 0,
    }
}

fn conducted_power_to(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    overlay: &HashMap<PosKey, i32>,
    source: &BlockPosition,
    direction_to_target: Direction,
    active_comparators: &mut HashSet<PosKey>,
) -> u8 {
    let mut power = 0;
    let target_input_direction = direction_to_target.opposite();
    for input_direction in DIRECTIONS {
        if input_direction == target_input_direction {
            continue;
        }
        let input_position = offset(
            source,
            -input_direction.dx,
            -input_direction.dy,
            -input_direction.dz,
        );
        let Some(input_state) = state_at(world, dimension, overlay, &input_position) else {
            continue;
        };
        let input_name = block_name(input_state);
        power = power.max(source_direct_power_to(
            world,
            dimension,
            overlay,
            &input_position,
            input_state,
            &input_name,
            input_direction,
            active_comparators,
        ));
    }
    power
}

fn comparator_output_power(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    overlay: &HashMap<PosKey, i32>,
    position: &BlockPosition,
    block_state: i32,
) -> u8 {
    comparator_output_power_inner(
        world,
        dimension,
        overlay,
        position,
        block_state,
        &mut HashSet::new(),
    )
}

fn comparator_output_power_inner(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    overlay: &HashMap<PosKey, i32>,
    position: &BlockPosition,
    block_state: i32,
    active_comparators: &mut HashSet<PosKey>,
) -> u8 {
    let key = PosKey::from(position);
    if !active_comparators.insert(key) {
        return if property_bool(block_state, "powered") {
            15
        } else {
            0
        };
    }

    let rear = comparator_rear_input_power(
        world,
        dimension,
        overlay,
        position,
        block_state,
        active_comparators,
    );
    let side = comparator_side_input_power(
        world,
        dimension,
        overlay,
        position,
        block_state,
        active_comparators,
    );
    let output = if property_value(block_state, "mode").as_deref() == Some("subtract") {
        rear.saturating_sub(side)
    } else if rear >= side {
        rear
    } else {
        0
    };
    active_comparators.remove(&key);
    output.min(15)
}

fn comparator_rear_input_power(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    overlay: &HashMap<PosKey, i32>,
    position: &BlockPosition,
    block_state: i32,
    active_comparators: &mut HashSet<PosKey>,
) -> u8 {
    let facing = facing_direction(block_state).unwrap_or(Direction {
        dx: 0,
        dy: 0,
        dz: -1,
    });
    let rear = offset(position, -facing.dx, -facing.dy, -facing.dz);
    let redstone_power =
        source_power_to_inner(world, dimension, overlay, &rear, facing, active_comparators);
    let readable_power = state_at(world, dimension, overlay, &rear)
        .map(comparator_readable_block_power)
        .unwrap_or(0);
    redstone_power.max(readable_power)
}

fn comparator_side_input_power(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    overlay: &HashMap<PosKey, i32>,
    position: &BlockPosition,
    block_state: i32,
    active_comparators: &mut HashSet<PosKey>,
) -> u8 {
    let facing = facing_direction(block_state).unwrap_or(Direction {
        dx: 0,
        dy: 0,
        dz: -1,
    });
    perpendicular_directions(facing)
        .into_iter()
        .map(|direction| {
            let source = offset(position, -direction.dx, -direction.dy, -direction.dz);
            source_power_to_inner(
                world,
                dimension,
                overlay,
                &source,
                direction,
                active_comparators,
            )
        })
        .max()
        .unwrap_or(0)
}

fn comparator_readable_block_power(block_state: i32) -> u8 {
    match block_name(block_state).as_str() {
        "minecraft:composter" => property_level(block_state, "level", 8),
        "minecraft:water_cauldron" | "minecraft:powder_snow_cauldron" => {
            property_level(block_state, "level", 3)
        }
        "minecraft:lava_cauldron" => 3,
        "minecraft:cake" => 14u8.saturating_sub(property_level(block_state, "bites", 6) * 2),
        "minecraft:respawn_anchor" => property_level(block_state, "charges", 4) * 15 / 4,
        "minecraft:beehive" | "minecraft:bee_nest" => property_level(block_state, "honey_level", 5),
        _ => 0,
    }
}

fn tnt_explosion_air_blocks(
    world: &dyn crate::world_access::WorldBlockSource,
    explosion: &ScheduledTntExplosion,
) -> Vec<RedstoneBlockChange> {
    let center = explosion.key.position();
    let radius = explosion.radius.clamp(0.1, 16.0);
    let min_x = (f64::from(center.x) + 0.5 - radius).floor() as i32;
    let max_x = (f64::from(center.x) + 0.5 + radius).floor() as i32;
    let min_y = (f64::from(center.y) + 0.5 - radius).floor() as i32;
    let max_y = (f64::from(center.y) + 0.5 + radius).floor() as i32;
    let min_z = (f64::from(center.z) + 0.5 - radius).floor() as i32;
    let max_z = (f64::from(center.z) + 0.5 + radius).floor() as i32;
    let air = crate::inventory::air_block_state();
    let mut changes = Vec::new();
    for x in min_x..=max_x {
        for y in min_y..=max_y {
            for z in min_z..=max_z {
                let dx = f64::from(x) + 0.5 - (f64::from(center.x) + 0.5);
                let dy = f64::from(y) + 0.5 - (f64::from(center.y) + 0.5);
                let dz = f64::from(z) + 0.5 - (f64::from(center.z) + 0.5);
                if dx * dx + dy * dy + dz * dz > radius * radius {
                    continue;
                }
                let position = BlockPosition { x, y, z };
                let current = world
                    .block_state_at(&explosion.key.dimension, &position)
                    .unwrap_or(air);
                if tnt_explosion_can_break_block_state(current) {
                    changes.push(RedstoneBlockChange {
                        dimension: explosion.key.dimension.clone(),
                        position,
                        block_state: air,
                    });
                }
            }
        }
    }
    changes
}

fn tnt_explosion_can_break_block_state(block_state: i32) -> bool {
    if crate::inventory::is_air_block_state(block_state) {
        return false;
    }
    !matches!(
        block_name(block_state).as_str(),
        "minecraft:bedrock"
            | "minecraft:barrier"
            | "minecraft:command_block"
            | "minecraft:chain_command_block"
            | "minecraft:repeating_command_block"
            | "minecraft:end_portal"
            | "minecraft:end_portal_frame"
            | "minecraft:structure_block"
            | "minecraft:jigsaw"
    )
}

fn redstone_torch_outputs_to(
    block_state: i32,
    block_name: &str,
    direction_to_target: Direction,
) -> bool {
    if block_name == "minecraft:redstone_wall_torch" {
        return facing_direction(block_state)
            .is_none_or(|facing| direction_to_target != facing.opposite());
    }
    direction_to_target
        != Direction {
            dx: 0,
            dy: -1,
            dz: 0,
        }
}

fn pressure_plate_under_player(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    position: EntityPosition,
) -> Option<(BlockPosition, i32)> {
    let surface = BlockPosition {
        x: position.x.floor() as i32,
        y: position.y.floor() as i32,
        z: position.z.floor() as i32,
    };
    if let Some(state) = world.block_state_at(dimension, &surface)
        && is_pressure_plate(&block_name(state))
    {
        return Some((surface, state));
    }
    let base = BlockPosition {
        x: position.x.floor() as i32,
        y: (position.y - 0.0001).floor() as i32,
        z: position.z.floor() as i32,
    };
    let state = world.block_state_at(dimension, &base)?;
    is_pressure_plate(&block_name(state)).then_some((base, state))
}

fn tripwire_at_player(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    position: EntityPosition,
) -> Option<(BlockPosition, i32)> {
    let surface = BlockPosition {
        x: position.x.floor() as i32,
        y: position.y.floor() as i32,
        z: position.z.floor() as i32,
    };
    if let Some(state) = world.block_state_at(dimension, &surface)
        && is_tripwire(&block_name(state))
    {
        return Some((surface, state));
    }
    let base = BlockPosition {
        x: position.x.floor() as i32,
        y: (position.y - 0.0001).floor() as i32,
        z: position.z.floor() as i32,
    };
    let state = world.block_state_at(dimension, &base)?;
    is_tripwire(&block_name(state)).then_some((base, state))
}

fn collect_tripwire_line_updates(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    position: &BlockPosition,
    active: &HashSet<RedstoneBlockKey>,
    changes: &mut HashMap<RedstoneBlockKey, RedstoneBlockChange>,
) -> bool {
    let Some(origin_state) = world.block_state_at(dimension, position) else {
        return false;
    };
    if !is_tripwire(&block_name(origin_state)) {
        return false;
    }

    let mut synced = false;
    for (first, second) in tripwire_axis_pairs() {
        let Some(first_hook) = find_tripwire_hook(world, dimension, position, first) else {
            continue;
        };
        let Some(second_hook) = find_tripwire_hook(world, dimension, position, second) else {
            continue;
        };
        synced = true;
        collect_tripwire_pair_updates(
            dimension,
            position,
            origin_state,
            first_hook,
            second_hook,
            active,
            changes,
        );
    }
    synced
}

fn tripwire_axis_pairs() -> [(Direction, Direction); 2] {
    [
        (
            Direction {
                dx: 1,
                dy: 0,
                dz: 0,
            },
            Direction {
                dx: -1,
                dy: 0,
                dz: 0,
            },
        ),
        (
            Direction {
                dx: 0,
                dy: 0,
                dz: 1,
            },
            Direction {
                dx: 0,
                dy: 0,
                dz: -1,
            },
        ),
    ]
}

fn find_tripwire_hook(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    origin: &BlockPosition,
    direction: Direction,
) -> Option<TripwireHookScan> {
    let mut wires = Vec::new();
    for distance in 1..=TRIPWIRE_MAX_SCAN {
        let position = offset(
            origin,
            direction.dx * distance,
            direction.dy * distance,
            direction.dz * distance,
        );
        let state = world.block_state_at(dimension, &position)?;
        let name = block_name(state);
        if is_tripwire(&name) {
            wires.push((position, state));
            continue;
        }
        if is_tripwire_hook(&name)
            && facing_direction(state).is_some_and(|facing| facing == direction.opposite())
        {
            return Some(TripwireHookScan {
                position,
                state,
                wires,
            });
        }
        return None;
    }
    None
}

fn collect_tripwire_pair_updates(
    dimension: &str,
    origin_position: &BlockPosition,
    origin_state: i32,
    first_hook: TripwireHookScan,
    second_hook: TripwireHookScan,
    active: &HashSet<RedstoneBlockKey>,
    changes: &mut HashMap<RedstoneBlockKey, RedstoneBlockChange>,
) {
    let mut wires = first_hook.wires.clone();
    wires.push((origin_position.clone(), origin_state));
    wires.extend(second_hook.wires.clone());
    let powered = wires
        .iter()
        .any(|(position, _)| active.contains(&RedstoneBlockKey::new(dimension, position)));

    for (position, state) in wires {
        let next = tripwire_wire_state(state, true, powered);
        if next != state {
            insert_redstone_change(
                changes,
                RedstoneBlockChange {
                    dimension: dimension.to_string(),
                    position,
                    block_state: next,
                },
            );
        }
    }
    for hook in [first_hook, second_hook] {
        let next = tripwire_hook_state(hook.state, true, powered);
        if next != hook.state {
            insert_redstone_change(
                changes,
                RedstoneBlockChange {
                    dimension: dimension.to_string(),
                    position: hook.position,
                    block_state: next,
                },
            );
        }
    }
}

fn tripwire_wire_state(block_state: i32, attached: bool, powered: bool) -> i32 {
    let mut next = block_state;
    if has_property(next, "attached") {
        next = block_state_with_property(next, "attached", bool_value(attached)).unwrap_or(next);
    }
    if has_property(next, "powered") {
        next = block_state_with_property(next, "powered", bool_value(powered)).unwrap_or(next);
    }
    next
}

fn tripwire_hook_state(block_state: i32, attached: bool, powered: bool) -> i32 {
    let mut next = block_state;
    if has_property(next, "attached") {
        next = block_state_with_property(next, "attached", bool_value(attached)).unwrap_or(next);
    }
    if has_property(next, "powered") {
        next = block_state_with_property(next, "powered", bool_value(powered)).unwrap_or(next);
    }
    next
}

fn insert_redstone_change(
    changes: &mut HashMap<RedstoneBlockKey, RedstoneBlockChange>,
    change: RedstoneBlockChange,
) {
    changes.insert(
        RedstoneBlockKey::new(change.dimension.clone(), &change.position),
        change,
    );
}

fn pressure_plate_powered_state(block_state: i32, occupant_count: u32) -> Option<i32> {
    if has_property(block_state, "power") {
        let name = block_name(block_state);
        set_power_level(
            block_state,
            weighted_pressure_plate_power(&name, occupant_count),
        )
    } else {
        block_state_with_property(block_state, "powered", bool_value(occupant_count > 0))
    }
}

fn weighted_pressure_plate_power(block_name: &str, occupant_count: u32) -> u8 {
    if occupant_count == 0 {
        return 0;
    }
    if block_name == "minecraft:heavy_weighted_pressure_plate" {
        return occupant_count.div_ceil(10).min(15) as u8;
    }
    occupant_count.min(15) as u8
}

fn powered_control_level(block_state: i32) -> u8 {
    if has_property(block_state, "power") {
        power_level(block_state)
    } else if property_bool(block_state, "powered") {
        15
    } else {
        0
    }
}

fn daylight_detector_power(time: i64, inverted: bool) -> u8 {
    let day_time = time.rem_euclid(24_000) as f64;
    let daylight = if day_time <= 12_000.0 {
        (std::f64::consts::PI * day_time / 12_000.0).sin()
    } else {
        0.0
    };
    let power = (daylight * 15.0).round().clamp(0.0, 15.0) as u8;
    if inverted { 15 - power } else { power }
}

fn block_state_with_property(block_state: i32, key: &str, value: &str) -> Option<i32> {
    let entry = crate::world_access::block_state_entry(block_state);
    let mut properties = entry.properties;
    let mut found = false;
    for (property_key, property_value) in &mut properties {
        if property_key == key {
            *property_value = value.to_string();
            found = true;
            break;
        }
    }
    found.then(|| crate::world_access::block_state(&entry.name, &properties))
}

fn set_power_level(block_state: i32, power: u8) -> Option<i32> {
    block_state_with_property(block_state, "power", &power.min(15).to_string())
}

fn power_level(block_state: i32) -> u8 {
    property_value(block_state, "power")
        .and_then(|value| value.parse::<u8>().ok())
        .unwrap_or(0)
        .min(15)
}

fn property_level(block_state: i32, key: &str, max: u8) -> u8 {
    property_value(block_state, key)
        .and_then(|value| value.parse::<u8>().ok())
        .unwrap_or(0)
        .min(max)
}

fn property_bool(block_state: i32, key: &str) -> bool {
    property_value(block_state, key).is_some_and(|value| value == "true")
}

fn property_value(block_state: i32, key: &str) -> Option<String> {
    crate::world_access::block_state_entry(block_state)
        .properties
        .into_iter()
        .find_map(|(property_key, value)| (property_key == key).then_some(value))
}

fn has_property(block_state: i32, key: &str) -> bool {
    property_value(block_state, key).is_some()
}

fn facing_direction(block_state: i32) -> Option<Direction> {
    match property_value(block_state, "facing")?.as_str() {
        "east" => Some(Direction {
            dx: 1,
            dy: 0,
            dz: 0,
        }),
        "west" => Some(Direction {
            dx: -1,
            dy: 0,
            dz: 0,
        }),
        "south" => Some(Direction {
            dx: 0,
            dy: 0,
            dz: 1,
        }),
        "north" => Some(Direction {
            dx: 0,
            dy: 0,
            dz: -1,
        }),
        "up" => Some(Direction {
            dx: 0,
            dy: 1,
            dz: 0,
        }),
        "down" => Some(Direction {
            dx: 0,
            dy: -1,
            dz: 0,
        }),
        _ => None,
    }
}

fn state_at(
    world: &dyn crate::world_access::WorldBlockSource,
    dimension: &str,
    overlay: &HashMap<PosKey, i32>,
    position: &BlockPosition,
) -> Option<i32> {
    overlay
        .get(&PosKey::from(position))
        .copied()
        .or_else(|| world.block_state_at(dimension, position))
}

fn block_name(block_state: i32) -> String {
    crate::inventory::block_name_for_state(block_state)
        .unwrap_or_else(|| crate::world_access::block_state_entry(block_state).name)
}

fn is_redstone_conductor(block_state: i32, block_name: &str) -> bool {
    !matches!(
        block_name,
        "minecraft:redstone_wire"
            | "minecraft:redstone_lamp"
            | "minecraft:redstone_torch"
            | "minecraft:redstone_wall_torch"
            | "minecraft:repeater"
            | "minecraft:comparator"
            | "minecraft:redstone_block"
            | "minecraft:observer"
    ) && !is_powered_control(block_name)
        && !is_redstone_driven_device(block_name, block_state)
        && !crate::inventory::can_replace_block_state(block_state)
        && crate::inventory::block_has_collision(block_state)
}

fn redstone_driven_device_state(block_state: i32, block_name: &str, powered: bool) -> Option<i32> {
    if is_copper_bulb(block_name) {
        return copper_bulb_state(block_state, powered);
    }

    let mut next = block_state;
    let mut handled = false;
    if is_openable_redstone_device(block_name) {
        if has_property(next, "open") {
            next = block_state_with_property(next, "open", bool_value(powered)).unwrap_or(next);
            handled = true;
        }
        if has_property(next, "powered") {
            next = block_state_with_property(next, "powered", bool_value(powered)).unwrap_or(next);
            handled = true;
        }
    } else if is_redstone_rail(block_name) {
        if has_property(next, "powered") {
            next = block_state_with_property(next, "powered", bool_value(powered)).unwrap_or(next);
            handled = true;
        }
    } else if is_triggered_redstone_device(block_name) {
        if has_property(next, "triggered") {
            next =
                block_state_with_property(next, "triggered", bool_value(powered)).unwrap_or(next);
            handled = true;
        }
    } else if is_piston_device(block_name) && has_property(next, "extended") {
        next = block_state_with_property(next, "extended", bool_value(powered)).unwrap_or(next);
        handled = true;
    }

    handled.then_some(next)
}

fn copper_bulb_state(block_state: i32, powered: bool) -> Option<i32> {
    if !has_property(block_state, "powered") || !has_property(block_state, "lit") {
        return None;
    }

    let was_powered = property_bool(block_state, "powered");
    let was_lit = property_bool(block_state, "lit");
    let mut next = block_state_with_property(block_state, "powered", bool_value(powered))
        .unwrap_or(block_state);
    if powered && !was_powered {
        next = block_state_with_property(next, "lit", bool_value(!was_lit)).unwrap_or(next);
    }
    Some(next)
}

fn is_redstone_reactive(block_state: i32, block_name: &str) -> bool {
    matches!(
        block_name,
        "minecraft:redstone_wire"
            | "minecraft:redstone_lamp"
            | "minecraft:redstone_torch"
            | "minecraft:redstone_wall_torch"
            | "minecraft:repeater"
            | "minecraft:comparator"
            | "minecraft:tnt"
    ) || is_redstone_driven_device(block_name, block_state)
}

fn is_redstone_driven_device(block_name: &str, block_state: i32) -> bool {
    (is_openable_redstone_device(block_name) && has_property(block_state, "open"))
        || (is_redstone_rail(block_name) && has_property(block_state, "powered"))
        || (is_copper_bulb(block_name)
            && has_property(block_state, "powered")
            && has_property(block_state, "lit"))
        || (is_triggered_redstone_device(block_name) && has_property(block_state, "triggered"))
        || (is_piston_device(block_name) && has_property(block_state, "extended"))
}

fn is_openable_redstone_device(block_name: &str) -> bool {
    is_door_device(block_name)
        || block_name.ends_with("_trapdoor")
        || block_name.ends_with("_fence_gate")
}

fn is_door_device(block_name: &str) -> bool {
    block_name.ends_with("_door")
}

fn is_redstone_rail(block_name: &str) -> bool {
    matches!(
        block_name,
        "minecraft:powered_rail" | "minecraft:detector_rail" | "minecraft:activator_rail"
    )
}

fn is_copper_bulb(block_name: &str) -> bool {
    block_name == "minecraft:copper_bulb" || block_name.ends_with("_copper_bulb")
}

fn is_triggered_redstone_device(block_name: &str) -> bool {
    matches!(
        block_name,
        "minecraft:dispenser" | "minecraft:dropper" | "minecraft:crafter"
    )
}

fn is_piston_device(block_name: &str) -> bool {
    matches!(block_name, "minecraft:piston" | "minecraft:sticky_piston")
}

fn is_powered_control(block_name: &str) -> bool {
    is_lever(block_name)
        || is_button(block_name)
        || is_pressure_plate(block_name)
        || is_tripwire(block_name)
        || is_tripwire_hook(block_name)
        || is_daylight_detector(block_name)
        || is_target_block(block_name)
        || is_sculk_sensor(block_name)
        || is_lightning_rod(block_name)
}

fn is_lever(block_name: &str) -> bool {
    block_name == "minecraft:lever"
}

fn is_button(block_name: &str) -> bool {
    block_name.ends_with("_button")
}

fn is_pressure_plate(block_name: &str) -> bool {
    block_name.ends_with("_pressure_plate")
}

fn is_tripwire(block_name: &str) -> bool {
    block_name == "minecraft:tripwire"
}

fn is_tripwire_hook(block_name: &str) -> bool {
    block_name == "minecraft:tripwire_hook"
}

fn is_daylight_detector(block_name: &str) -> bool {
    block_name == "minecraft:daylight_detector"
}

fn is_target_block(block_name: &str) -> bool {
    block_name == "minecraft:target"
}

fn is_sculk_sensor(block_name: &str) -> bool {
    matches!(
        block_name,
        "minecraft:sculk_sensor" | "minecraft:calibrated_sculk_sensor"
    )
}

fn is_lightning_rod(block_name: &str) -> bool {
    block_name == "minecraft:lightning_rod"
}

fn is_repeater(block_name: &str) -> bool {
    block_name == "minecraft:repeater"
}

fn is_comparator(block_name: &str) -> bool {
    block_name == "minecraft:comparator"
}

fn is_observer(block_name: &str) -> bool {
    block_name == "minecraft:observer"
}

fn button_release_ms(block_name: &str) -> u64 {
    if block_name == "minecraft:stone_button"
        || block_name == "minecraft:polished_blackstone_button"
    {
        1000
    } else {
        1500
    }
}

fn bool_value(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn offset(position: &BlockPosition, dx: i32, dy: i32, dz: i32) -> BlockPosition {
    BlockPosition {
        x: position.x.saturating_add(dx),
        y: position.y.saturating_add(dy),
        z: position.z.saturating_add(dz),
    }
}

impl RedstoneBlockKey {
    fn new(dimension: impl Into<String>, position: &BlockPosition) -> Self {
        Self {
            dimension: dimension.into(),
            x: position.x,
            y: position.y,
            z: position.z,
        }
    }

    fn position(&self) -> BlockPosition {
        BlockPosition {
            x: self.x,
            y: self.y,
            z: self.z,
        }
    }
}

impl From<&BlockPosition> for PosKey {
    fn from(value: &BlockPosition) -> Self {
        Self {
            x: value.x,
            y: value.y,
            z: value.z,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIMENSION: &str = "minecraft:overworld";

    fn test_world() -> crate::world_access::InMemoryWorld {
        crate::world_access::InMemoryWorld::default()
    }

    fn pos(x: i32, y: i32, z: i32) -> BlockPosition {
        BlockPosition { x, y, z }
    }

    fn state(name: &str, properties: &[(&str, &str)]) -> i32 {
        crate::world_access::block_state(
            name,
            &properties
                .iter()
                .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
                .collect::<Vec<_>>(),
        )
    }

    fn wire_state(power: u8) -> i32 {
        let power = power.min(15).to_string();
        state(
            "minecraft:redstone_wire",
            &[
                ("east", "none"),
                ("north", "none"),
                ("power", power.as_str()),
                ("south", "none"),
                ("west", "none"),
            ],
        )
    }

    fn test_players_at(position: EntityPosition) -> qexed_player::PlayerManager {
        let players = qexed_player::PlayerManager::new(std::sync::Arc::new(
            qexed_protocol::types::EntityIdAllocator::new(1),
        ));
        let _session = players.join(
            qexed_packet::net_types::GameProfile {
                uuid: uuid::Uuid::new_v4(),
                username: "Tester".to_string(),
                properties: Vec::new(),
            },
            position,
            DIMENSION.to_string(),
            Vec::new(),
            "zh_cn".to_string(),
        );
        players
    }

    fn entity_pos(x: f64, y: f64, z: f64) -> EntityPosition {
        EntityPosition {
            x,
            y,
            z,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        }
    }

    fn test_world_rules() -> qexed_world::world::WorldRulesManager {
        qexed_world::world::WorldRulesManager::from_world_config(
            &qexed_world::config::WorldConfig::default(),
        )
        .expect("world rules")
    }

    fn update_for(updates: &[RedstoneBlockChange], position: BlockPosition) -> RedstoneBlockChange {
        updates
            .iter()
            .find(|update| update.position == position)
            .cloned()
            .expect("redstone update")
    }

    #[test]
    fn redstone_block_powers_wire_with_decay() {
        let world = test_world();
        let source = pos(0, 64, 0);
        let first_wire = pos(1, 64, 0);
        let second_wire = pos(2, 64, 0);
        world.place(
            DIMENSION,
            source.clone(),
            crate::world_access::default_block_state_id("minecraft:redstone_block"),
        );
        world.place(
            DIMENSION,
            first_wire.clone(),
            crate::world_access::default_block_state_id("minecraft:redstone_wire"),
        );
        world.place(
            DIMENSION,
            second_wire.clone(),
            crate::world_access::default_block_state_id("minecraft:redstone_wire"),
        );

        let updates = updates_after_block_changes(&world, DIMENSION, &[source], 8);
        let first_wire_state = update_for(&updates, first_wire).block_state;

        assert_eq!(power_level(first_wire_state), 15);
        assert_eq!(
            property_value(first_wire_state, "west").as_deref(),
            Some("side")
        );
        assert_eq!(
            property_value(first_wire_state, "east").as_deref(),
            Some("side")
        );
        assert_eq!(
            power_level(update_for(&updates, second_wire).block_state),
            14
        );
    }

    #[test]
    fn redstone_wire_propagates_one_block_down() {
        let world = test_world();
        let source = pos(0, 64, 0);
        let upper_wire = pos(1, 64, 0);
        let lower_wire = pos(2, 63, 0);
        world.place(
            DIMENSION,
            source.clone(),
            crate::world_access::default_block_state_id("minecraft:redstone_block"),
        );
        world.place(
            DIMENSION,
            upper_wire.clone(),
            crate::world_access::default_block_state_id("minecraft:redstone_wire"),
        );
        world.place(
            DIMENSION,
            lower_wire.clone(),
            crate::world_access::default_block_state_id("minecraft:redstone_wire"),
        );

        let updates = updates_after_block_changes(&world, DIMENSION, &[source], 8);
        let upper = update_for(&updates, upper_wire).block_state;
        let lower = update_for(&updates, lower_wire).block_state;

        assert_eq!(property_value(upper, "east").as_deref(), Some("side"));
        assert_eq!(power_level(lower), 14);
    }

    #[test]
    fn redstone_block_lights_lamp() {
        let world = test_world();
        let source = pos(0, 64, 0);
        let lamp = pos(1, 64, 0);
        world.place(
            DIMENSION,
            source.clone(),
            crate::world_access::default_block_state_id("minecraft:redstone_block"),
        );
        world.place(
            DIMENSION,
            lamp.clone(),
            crate::world_access::default_block_state_id("minecraft:redstone_lamp"),
        );

        let updates = updates_after_block_changes(&world, DIMENSION, &[source], 8);

        assert!(property_bool(update_for(&updates, lamp).block_state, "lit"));
    }

    #[test]
    fn target_block_power_feeds_redstone_wire() {
        let world = test_world();
        let target = pos(0, 64, 0);
        let wire = pos(1, 64, 0);
        world.place(
            DIMENSION,
            target.clone(),
            state("minecraft:target", &[("power", "7")]),
        );
        world.place(
            DIMENSION,
            wire.clone(),
            crate::world_access::default_block_state_id("minecraft:redstone_wire"),
        );

        let updates = updates_after_block_changes(&world, DIMENSION, &[target], 8);

        assert_eq!(power_level(update_for(&updates, wire).block_state), 7);
    }

    #[test]
    fn powered_tnt_turns_to_air_for_priming() {
        let world = test_world();
        let source = pos(0, 64, 0);
        let tnt = pos(1, 64, 0);
        world.place(
            DIMENSION,
            source.clone(),
            crate::world_access::default_block_state_id("minecraft:redstone_block"),
        );
        world.place(
            DIMENSION,
            tnt.clone(),
            crate::world_access::default_block_state_id("minecraft:tnt"),
        );

        let updates = updates_after_block_changes(&world, DIMENSION, &[source], 8);

        assert_eq!(
            update_for(&updates, tnt).block_state,
            crate::inventory::air_block_state()
        );
    }

    #[test]
    fn scheduled_tnt_explosion_breaks_allowed_blocks() {
        let world = test_world();
        let rules = test_world_rules();
        let players = test_players_at(entity_pos(8.0, 64.0, 8.0));
        let center = pos(0, 64, 0);
        let stone = pos(1, 64, 0);
        let bedrock = pos(2, 64, 0);
        world.place(
            DIMENSION,
            stone.clone(),
            crate::world_access::default_block_state_id("minecraft:stone"),
        );
        world.place(
            DIMENSION,
            bedrock.clone(),
            crate::world_access::default_block_state_id("minecraft:bedrock"),
        );
        let mut runtime = RedstoneRuntime::new();
        runtime.schedule_tnt_explosion(DIMENSION.to_string(), center, 10);

        std::thread::sleep(Duration::from_millis(70));
        let updates = runtime.tick(
            &world,
            &rules,
            &players,
            &crate::config::GameplayConfig::default(),
        );

        assert_eq!(
            update_for(&updates, stone).block_state,
            crate::inventory::air_block_state()
        );
        assert!(updates.iter().all(|update| update.position != bedrock));
    }

    #[test]
    fn solid_block_conducts_power_to_lamp() {
        let world = test_world();
        let source = pos(0, 64, 0);
        let stone = pos(1, 64, 0);
        let lamp = pos(2, 64, 0);
        world.place(
            DIMENSION,
            source.clone(),
            crate::world_access::default_block_state_id("minecraft:redstone_block"),
        );
        world.place(
            DIMENSION,
            stone,
            crate::world_access::default_block_state_id("minecraft:stone"),
        );
        world.place(
            DIMENSION,
            lamp.clone(),
            crate::world_access::default_block_state_id("minecraft:redstone_lamp"),
        );

        let updates = updates_after_block_changes(&world, DIMENSION, &[source], 8);

        assert!(property_bool(update_for(&updates, lamp).block_state, "lit"));
    }

    #[test]
    fn daylight_detector_power_follows_day_time() {
        assert_eq!(daylight_detector_power(6_000, false), 15);
        assert_eq!(daylight_detector_power(18_000, false), 0);
        assert_eq!(daylight_detector_power(18_000, true), 15);
    }

    #[test]
    fn daylight_detector_runtime_updates_power() {
        let world = test_world();
        let rules = test_world_rules();
        rules.set_time_value(DIMENSION, 6_000).expect("set time");
        let detector = pos(0, 64, 0);
        world.place(
            DIMENSION,
            detector.clone(),
            crate::world_access::default_block_state_id("minecraft:daylight_detector"),
        );
        let players = test_players_at(entity_pos(0.5, 64.0, 0.5));
        let mut runtime = RedstoneRuntime::new();

        let updates = runtime.tick(
            &world,
            &rules,
            &players,
            &crate::config::GameplayConfig::default(),
        );

        assert_eq!(power_level(update_for(&updates, detector).block_state), 15);
    }

    #[test]
    fn weighted_pressure_plate_uses_player_count_power() {
        let world = test_world();
        let rules = test_world_rules();
        let plate = pos(0, 64, 0);
        world.place(
            DIMENSION,
            plate.clone(),
            state("minecraft:light_weighted_pressure_plate", &[("power", "0")]),
        );
        let players = test_players_at(entity_pos(0.5, 64.0, 0.5));
        let mut runtime = RedstoneRuntime::new();

        let updates = runtime.tick(
            &world,
            &rules,
            &players,
            &crate::config::GameplayConfig::default(),
        );

        assert_eq!(power_level(update_for(&updates, plate).block_state), 1);
    }

    #[test]
    fn heavy_weighted_pressure_plate_uses_ten_entity_steps() {
        assert_eq!(
            weighted_pressure_plate_power("minecraft:heavy_weighted_pressure_plate", 1),
            1
        );
        assert_eq!(
            weighted_pressure_plate_power("minecraft:heavy_weighted_pressure_plate", 10),
            1
        );
        assert_eq!(
            weighted_pressure_plate_power("minecraft:heavy_weighted_pressure_plate", 11),
            2
        );
        assert_eq!(
            weighted_pressure_plate_power("minecraft:heavy_weighted_pressure_plate", 200),
            15
        );
    }

    #[test]
    fn tripwire_runtime_powers_when_player_intersects() {
        let world = test_world();
        let rules = test_world_rules();
        let tripwire = pos(0, 64, 0);
        world.place(
            DIMENSION,
            tripwire.clone(),
            state(
                "minecraft:tripwire",
                &[
                    ("attached", "false"),
                    ("disarmed", "false"),
                    ("east", "false"),
                    ("north", "false"),
                    ("powered", "false"),
                    ("south", "false"),
                    ("west", "false"),
                ],
            ),
        );
        let players = test_players_at(entity_pos(0.5, 64.0, 0.5));
        let mut runtime = RedstoneRuntime::new();

        let updates = runtime.tick(
            &world,
            &rules,
            &players,
            &crate::config::GameplayConfig::default(),
        );

        assert!(property_bool(
            update_for(&updates, tripwire).block_state,
            "powered"
        ));
    }

    #[test]
    fn tripwire_runtime_syncs_attached_hooks() {
        let world = test_world();
        let rules = test_world_rules();
        let west_hook = pos(-1, 64, 0);
        let tripwire = pos(0, 64, 0);
        let east_hook = pos(1, 64, 0);
        world.place(
            DIMENSION,
            west_hook.clone(),
            state(
                "minecraft:tripwire_hook",
                &[
                    ("attached", "false"),
                    ("facing", "east"),
                    ("powered", "false"),
                ],
            ),
        );
        world.place(
            DIMENSION,
            tripwire.clone(),
            state(
                "minecraft:tripwire",
                &[
                    ("attached", "false"),
                    ("disarmed", "false"),
                    ("east", "true"),
                    ("north", "false"),
                    ("powered", "false"),
                    ("south", "false"),
                    ("west", "true"),
                ],
            ),
        );
        world.place(
            DIMENSION,
            east_hook.clone(),
            state(
                "minecraft:tripwire_hook",
                &[
                    ("attached", "false"),
                    ("facing", "west"),
                    ("powered", "false"),
                ],
            ),
        );
        let mut runtime = RedstoneRuntime::new();
        let players = test_players_at(entity_pos(0.5, 64.0, 0.5));

        let updates = runtime.tick(
            &world,
            &rules,
            &players,
            &crate::config::GameplayConfig::default(),
        );

        for position in [&west_hook, &tripwire, &east_hook] {
            let next = update_for(&updates, position.clone()).block_state;
            assert!(property_bool(next, "attached"));
            assert!(property_bool(next, "powered"));
        }
        for update in updates {
            world.place(DIMENSION, update.position, update.block_state);
        }

        let far_players = test_players_at(entity_pos(8.0, 64.0, 8.0));
        let updates = runtime.tick(
            &world,
            &rules,
            &far_players,
            &crate::config::GameplayConfig::default(),
        );

        for position in [&west_hook, &tripwire, &east_hook] {
            let next = update_for(&updates, position.clone()).block_state;
            assert!(property_bool(next, "attached"));
            assert!(!property_bool(next, "powered"));
        }
    }

    #[test]
    fn observer_pulses_after_observed_block_change() {
        let world = test_world();
        let rules = test_world_rules();
        let players = test_players_at(entity_pos(8.0, 64.0, 8.0));
        let observer = pos(0, 64, 0);
        let observed = pos(1, 64, 0);
        world.place(
            DIMENSION,
            observer.clone(),
            state(
                "minecraft:observer",
                &[("facing", "east"), ("powered", "false")],
            ),
        );
        let mut runtime = RedstoneRuntime::new();

        let pulse = runtime.observer_updates_after_block_changes(&world, DIMENSION, &[observed]);
        let powered = update_for(&pulse, observer.clone()).block_state;
        assert!(property_bool(powered, "powered"));

        world.place(DIMENSION, observer.clone(), powered);
        std::thread::sleep(Duration::from_millis(120));
        let updates = runtime.tick(
            &world,
            &rules,
            &players,
            &crate::config::GameplayConfig::default(),
        );

        assert!(!property_bool(
            update_for(&updates, observer).block_state,
            "powered"
        ));
    }

    #[test]
    fn repeater_power_change_waits_for_configured_delay_once() {
        let world = test_world();
        let rules = test_world_rules();
        let players = test_players_at(entity_pos(8.0, 64.0, 8.0));
        let repeater = pos(0, 64, 0);
        let current = state(
            "minecraft:repeater",
            &[
                ("delay", "2"),
                ("facing", "north"),
                ("locked", "false"),
                ("powered", "false"),
            ],
        );
        let powered = state(
            "minecraft:repeater",
            &[
                ("delay", "2"),
                ("facing", "north"),
                ("locked", "false"),
                ("powered", "true"),
            ],
        );
        world.place(DIMENSION, repeater.clone(), current);
        let mut runtime = RedstoneRuntime::new();
        let update = RedstoneBlockChange {
            dimension: DIMENSION.to_string(),
            position: repeater.clone(),
            block_state: powered,
        };

        assert!(runtime.should_delay_redstone_update(current, &update));
        assert!(
            runtime
                .tick(
                    &world,
                    &rules,
                    &players,
                    &crate::config::GameplayConfig::default(),
                )
                .iter()
                .all(|change| change.position != repeater)
        );

        std::thread::sleep(Duration::from_millis(120));
        let due_updates = runtime.tick(
            &world,
            &rules,
            &players,
            &crate::config::GameplayConfig::default(),
        );
        let due_update = update_for(&due_updates, repeater);

        assert_eq!(due_update.block_state, powered);
        assert!(!runtime.should_delay_redstone_update(current, &due_update));
    }

    #[test]
    fn redstone_lamp_turn_off_waits_two_ticks_once() {
        let world = test_world();
        let rules = test_world_rules();
        let players = test_players_at(entity_pos(8.0, 64.0, 8.0));
        let lamp = pos(0, 64, 0);
        let current = state("minecraft:redstone_lamp", &[("lit", "true")]);
        let unlit = state("minecraft:redstone_lamp", &[("lit", "false")]);
        world.place(DIMENSION, lamp.clone(), current);
        let mut runtime = RedstoneRuntime::new();
        let update = RedstoneBlockChange {
            dimension: DIMENSION.to_string(),
            position: lamp.clone(),
            block_state: unlit,
        };

        assert!(runtime.should_delay_redstone_update(current, &update));
        assert!(
            runtime
                .tick(
                    &world,
                    &rules,
                    &players,
                    &crate::config::GameplayConfig::default(),
                )
                .iter()
                .all(|change| change.position != lamp)
        );

        std::thread::sleep(Duration::from_millis(120));
        let due_updates = runtime.tick(
            &world,
            &rules,
            &players,
            &crate::config::GameplayConfig::default(),
        );
        let due_update = update_for(&due_updates, lamp);

        assert_eq!(due_update.block_state, unlit);
        assert!(!runtime.should_delay_redstone_update(current, &due_update));
    }

    #[test]
    fn lever_interaction_toggles_powered_property() {
        let lever = crate::world_access::default_block_state_id("minecraft:lever");

        let interaction =
            interaction_for_block_state(lever, "minecraft:lever").expect("lever interaction");

        assert!(property_bool(interaction.block_state, "powered"));
        assert_eq!(interaction.button_release_ms, None);
    }

    #[test]
    fn button_interaction_powers_temporarily() {
        let button = crate::world_access::default_block_state_id("minecraft:stone_button");

        let interaction =
            interaction_for_block_state(button, "minecraft:stone_button").expect("button press");

        assert!(property_bool(interaction.block_state, "powered"));
        assert_eq!(interaction.button_release_ms, Some(1000));
    }

    #[test]
    fn powered_side_repeater_locks_repeater() {
        let world = test_world();
        let source = pos(-2, 64, 0);
        let side_repeater = pos(-1, 64, 0);
        let target_repeater = pos(0, 64, 0);
        world.place(
            DIMENSION,
            source.clone(),
            crate::world_access::default_block_state_id("minecraft:redstone_block"),
        );
        world.place(
            DIMENSION,
            side_repeater,
            state(
                "minecraft:repeater",
                &[
                    ("delay", "1"),
                    ("facing", "east"),
                    ("locked", "false"),
                    ("powered", "true"),
                ],
            ),
        );
        world.place(
            DIMENSION,
            target_repeater.clone(),
            state(
                "minecraft:repeater",
                &[
                    ("delay", "1"),
                    ("facing", "north"),
                    ("locked", "false"),
                    ("powered", "false"),
                ],
            ),
        );

        let updates = updates_after_block_changes(&world, DIMENSION, &[source], 8);
        let next = update_for(&updates, target_repeater).block_state;

        assert!(property_bool(next, "locked"));
        assert!(!property_bool(next, "powered"));
    }

    #[test]
    fn comparator_compare_mode_turns_off_when_side_input_is_stronger() {
        let world = test_world();
        let rear = pos(-1, 64, 0);
        let side = pos(0, 64, -1);
        let comparator = pos(0, 64, 0);
        let output_wire = pos(1, 64, 0);
        world.place(DIMENSION, rear.clone(), wire_state(5));
        world.place(DIMENSION, side.clone(), wire_state(8));
        world.place(
            DIMENSION,
            comparator.clone(),
            state(
                "minecraft:comparator",
                &[("facing", "east"), ("mode", "compare"), ("powered", "true")],
            ),
        );
        world.place(DIMENSION, output_wire.clone(), wire_state(15));

        let updates = updates_after_block_changes(&world, DIMENSION, &[rear, side], 8);

        assert!(!property_bool(
            update_for(&updates, comparator).block_state,
            "powered"
        ));
        assert_eq!(
            power_level(update_for(&updates, output_wire).block_state),
            0
        );
    }

    #[test]
    fn comparator_subtract_mode_outputs_rear_minus_side_strength() {
        let world = test_world();
        let rear = pos(-1, 64, 0);
        let side = pos(0, 64, -1);
        let comparator = pos(0, 64, 0);
        let output_wire = pos(1, 64, 0);
        world.place(
            DIMENSION,
            rear.clone(),
            state(
                "minecraft:daylight_detector",
                &[("inverted", "false"), ("power", "10")],
            ),
        );
        world.place(
            DIMENSION,
            side.clone(),
            state(
                "minecraft:daylight_detector",
                &[("inverted", "false"), ("power", "4")],
            ),
        );
        world.place(
            DIMENSION,
            comparator.clone(),
            state(
                "minecraft:comparator",
                &[
                    ("facing", "east"),
                    ("mode", "subtract"),
                    ("powered", "false"),
                ],
            ),
        );
        world.place(DIMENSION, output_wire.clone(), wire_state(0));

        let updates = updates_after_block_changes(&world, DIMENSION, &[rear, side], 8);

        assert!(property_bool(
            update_for(&updates, comparator).block_state,
            "powered"
        ));
        assert_eq!(
            power_level(update_for(&updates, output_wire).block_state),
            6
        );
    }

    #[test]
    fn comparator_reads_state_based_block_strength() {
        let world = test_world();
        let composter = pos(-1, 64, 0);
        let comparator = pos(0, 64, 0);
        let output_wire = pos(1, 64, 0);
        world.place(
            DIMENSION,
            composter.clone(),
            state("minecraft:composter", &[("level", "6")]),
        );
        world.place(
            DIMENSION,
            comparator.clone(),
            state(
                "minecraft:comparator",
                &[
                    ("facing", "east"),
                    ("mode", "compare"),
                    ("powered", "false"),
                ],
            ),
        );
        world.place(DIMENSION, output_wire.clone(), wire_state(0));

        let updates = updates_after_block_changes(&world, DIMENSION, &[composter], 8);

        assert!(property_bool(
            update_for(&updates, comparator).block_state,
            "powered"
        ));
        assert_eq!(
            power_level(update_for(&updates, output_wire).block_state),
            6
        );
    }

    #[test]
    fn powered_door_updates_open_and_powered_state() {
        let world = test_world();
        let source = pos(0, 64, 0);
        let door = pos(1, 64, 0);
        let upper_door = pos(1, 65, 0);
        let door_state = state(
            "minecraft:oak_door",
            &[
                ("facing", "north"),
                ("half", "lower"),
                ("hinge", "left"),
                ("open", "false"),
                ("powered", "false"),
            ],
        );
        let upper_door_state = state(
            "minecraft:oak_door",
            &[
                ("facing", "north"),
                ("half", "upper"),
                ("hinge", "left"),
                ("open", "false"),
                ("powered", "false"),
            ],
        );
        world.place(
            DIMENSION,
            source.clone(),
            crate::world_access::default_block_state_id("minecraft:redstone_block"),
        );
        world.place(DIMENSION, door.clone(), door_state);
        world.place(DIMENSION, upper_door.clone(), upper_door_state);

        let updates = updates_after_block_changes(&world, DIMENSION, &[source], 8);
        let next = update_for(&updates, door).block_state;
        let next_upper = update_for(&updates, upper_door).block_state;

        assert!(property_bool(next, "open"));
        assert!(property_bool(next, "powered"));
        assert!(property_bool(next_upper, "open"));
        assert!(property_bool(next_upper, "powered"));
    }

    #[test]
    fn powered_crafter_updates_triggered_state() {
        let world = test_world();
        let source = pos(0, 64, 0);
        let crafter = pos(1, 64, 0);
        world.place(
            DIMENSION,
            source.clone(),
            crate::world_access::default_block_state_id("minecraft:redstone_block"),
        );
        world.place(
            DIMENSION,
            crafter.clone(),
            state(
                "minecraft:crafter",
                &[
                    ("crafting", "false"),
                    ("orientation", "down_east"),
                    ("triggered", "false"),
                ],
            ),
        );

        let updates = updates_after_block_changes(&world, DIMENSION, &[source], 8);

        assert!(property_bool(
            update_for(&updates, crafter).block_state,
            "triggered"
        ));
    }
}