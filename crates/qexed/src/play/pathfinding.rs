use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap},
};

use qexed_packet::net_types::Position;

use crate::world::WorldManager;

const DEFAULT_MAX_NODES: usize = 4096;
const MAX_STEP_UP_BLOCKS: i32 = 1;
const MAX_DROP_BLOCKS: i32 = 3;
const STEPABLE_COLLISION_HEIGHT: f64 = 0.6;

#[derive(Debug, Clone)]
pub struct PathQuery<'a> {
    pub world: &'a WorldManager,
    pub dimension: &'a str,
    pub start: Position,
    pub goal: Position,
    pub max_nodes: usize,
}

pub fn find_path(query: PathQuery<'_>) -> Option<Vec<Position>> {
    let max_nodes = query.max_nodes.max(1).min(DEFAULT_MAX_NODES);
    let mut walkable = HashMap::new();
    let start = PositionKey::from(&query.start);
    let goal = PositionKey::from(&query.goal);
    if !is_walkable_cached(query.world, query.dimension, start, &mut walkable)
        || !is_walkable_cached(query.world, query.dimension, goal, &mut walkable)
    {
        return None;
    }

    let mut frontier = BinaryHeap::new();
    let mut came_from = HashMap::<PositionKey, Option<PositionKey>>::new();
    let mut cost_so_far = HashMap::<PositionKey, i32>::new();
    let mut sequence = 0u64;
    frontier.push(PathNode {
        position: start,
        cost: 0,
        estimate: manhattan(start, goal),
        sequence,
    });
    came_from.insert(start, None);
    cost_so_far.insert(start, 0);

    while let Some(current) = frontier.pop() {
        if current.position == goal {
            return Some(reconstruct_path(start, goal, came_from));
        }
        if cost_so_far
            .get(&current.position)
            .is_some_and(|best| current.cost > *best)
        {
            continue;
        }

        for next in neighbours_toward(
            query.world,
            query.dimension,
            current.position,
            goal,
            &mut walkable,
        ) {
            if !is_walkable_cached(query.world, query.dimension, next, &mut walkable) {
                continue;
            }
            let new_cost = current.cost + movement_cost(current.position, next);
            if cost_so_far
                .get(&next)
                .is_some_and(|existing| new_cost >= *existing)
            {
                continue;
            }
            if !came_from.contains_key(&next) && came_from.len() >= max_nodes {
                continue;
            }
            came_from.insert(next, Some(current.position));
            cost_so_far.insert(next, new_cost);
            sequence = sequence.wrapping_add(1);
            frontier.push(PathNode {
                position: next,
                cost: new_cost,
                estimate: new_cost + manhattan(next, goal),
                sequence,
            });
        }
    }

    None
}

fn reconstruct_path(
    start: PositionKey,
    goal: PositionKey,
    came_from: HashMap<PositionKey, Option<PositionKey>>,
) -> Vec<Position> {
    let mut current = goal;
    let mut path = vec![current.position()];
    while current != start {
        let Some(Some(previous)) = came_from.get(&current).copied() else {
            break;
        };
        current = previous;
        path.push(current.position());
    }
    path.reverse();
    path
}

fn neighbours_toward(
    world: &WorldManager,
    dimension: &str,
    position: PositionKey,
    goal: PositionKey,
    walkable: &mut HashMap<PositionKey, bool>,
) -> Vec<PositionKey> {
    let mut neighbours = Vec::with_capacity(20);
    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        for dy in candidate_y_offsets(position, goal) {
            let candidate = PositionKey {
                x: position.x + dx,
                y: position.y + dy,
                z: position.z + dz,
            };
            if is_walkable_cached(world, dimension, candidate, walkable) {
                neighbours.push(candidate);
                break;
            }
        }
    }
    neighbours.sort_by_key(|position| manhattan(*position, goal));
    neighbours
}

fn manhattan(left: PositionKey, right: PositionKey) -> i32 {
    (left.x - right.x).abs() + (left.y - right.y).abs() + (left.z - right.z).abs()
}

fn movement_cost(from: PositionKey, to: PositionKey) -> i32 {
    10 + (to.y - from.y).abs() * 4
}

fn candidate_y_offsets(position: PositionKey, goal: PositionKey) -> Vec<i32> {
    let mut offsets = Vec::with_capacity(usize::try_from(2 + MAX_DROP_BLOCKS).unwrap_or(6));
    let preferred_vertical = (goal.y - position.y).clamp(-MAX_DROP_BLOCKS, MAX_STEP_UP_BLOCKS);
    offsets.push(preferred_vertical);
    for offset in 0..=MAX_STEP_UP_BLOCKS {
        push_unique_offset(&mut offsets, offset);
    }
    for offset in 1..=MAX_DROP_BLOCKS {
        push_unique_offset(&mut offsets, -offset);
    }
    offsets
}

fn push_unique_offset(offsets: &mut Vec<i32>, offset: i32) {
    if !offsets.contains(&offset) {
        offsets.push(offset);
    }
}

fn is_walkable_cached(
    world: &WorldManager,
    dimension: &str,
    position: PositionKey,
    cache: &mut HashMap<PositionKey, bool>,
) -> bool {
    if let Some(walkable) = cache.get(&position) {
        return *walkable;
    }
    let walkable = is_walkable(world, dimension, &position.position());
    cache.insert(position, walkable);
    walkable
}

fn is_walkable(world: &WorldManager, dimension: &str, feet: &Position) -> bool {
    let head = Position {
        x: feet.x,
        y: feet.y + 1,
        z: feet.z,
    };
    let below = Position {
        x: feet.x,
        y: feet.y - 1,
        z: feet.z,
    };

    let feet_shape = collision_shape_at(world, dimension, feet);
    let head_shape = collision_shape_at(world, dimension, &head);
    let below_shape = collision_shape_at(world, dimension, &below);

    let feet_clear = feet_shape.is_none_or(|shape| shape.max_y <= STEPABLE_COLLISION_HEIGHT);
    let head_clear = head_shape.is_none();
    let has_support = below_shape.is_some() || feet_shape.is_some_and(|shape| shape.max_y > 0.0);

    feet_clear && head_clear && has_support
}

fn collision_shape_at(
    world: &WorldManager,
    dimension: &str,
    position: &Position,
) -> Option<crate::inventory::BlockCollisionShape> {
    world
        .block_state_at(dimension, position)
        .and_then(crate::inventory::block_collision_shape)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PositionKey {
    x: i32,
    y: i32,
    z: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PathNode {
    position: PositionKey,
    cost: i32,
    estimate: i32,
    sequence: u64,
}

impl Ord for PathNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .estimate
            .cmp(&self.estimate)
            .then_with(|| other.cost.cmp(&self.cost))
            .then_with(|| other.sequence.cmp(&self.sequence))
    }
}

impl PartialOrd for PathNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PositionKey {
    fn position(self) -> Position {
        Position {
            x: self.x,
            y: self.y,
            z: self.z,
        }
    }
}

impl From<&Position> for PositionKey {
    fn from(position: &Position) -> Self {
        Self {
            x: position.x,
            y: position.y,
            z: position.z,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PathQuery, find_path};
    use qexed_packet::net_types::Position;

    fn stone_block_state() -> i32 {
        crate::inventory::placed_block_state_for_item(&qexed_protocol::types::Slot {
            item_count: qexed_packet::net_types::VarInt(1),
            item_id: Some(qexed_packet::net_types::VarInt(1)),
            ..Default::default()
        })
        .expect("stone block state")
    }

    fn empty_world() -> crate::world::WorldManager {
        crate::world::WorldManager::new(tempfile::tempdir().expect("temp world dir").keep())
    }

    #[test]
    fn pathfinding_routes_around_blocking_wall() {
        let world = empty_world();
        for x in 0..=4 {
            for z in -2..=2 {
                world.place_block(
                    "minecraft:overworld",
                    Position { x, y: 63, z },
                    stone_block_state(),
                );
            }
        }
        for z in -1..=1 {
            for y in 64..=65 {
                world.place_block(
                    "minecraft:overworld",
                    Position { x: 1, y, z },
                    stone_block_state(),
                );
            }
        }

        let path = find_path(PathQuery {
            world: &world,
            dimension: "minecraft:overworld",
            start: Position { x: 0, y: 64, z: 0 },
            goal: Position { x: 4, y: 64, z: 0 },
            max_nodes: 256,
        })
        .expect("path");

        assert!(path.iter().any(|position| position.z.abs() > 1));
        assert_eq!(path.last(), Some(&Position { x: 4, y: 64, z: 0 }));
    }

    #[test]
    fn pathfinding_steps_up_one_block() {
        let world = empty_world();
        world.place_block(
            "minecraft:overworld",
            Position { x: 0, y: 63, z: 0 },
            stone_block_state(),
        );
        world.place_block(
            "minecraft:overworld",
            Position { x: 1, y: 63, z: 0 },
            stone_block_state(),
        );
        world.place_block(
            "minecraft:overworld",
            Position { x: 1, y: 64, z: 0 },
            stone_block_state(),
        );

        let path = find_path(PathQuery {
            world: &world,
            dimension: "minecraft:overworld",
            start: Position { x: 0, y: 64, z: 0 },
            goal: Position { x: 1, y: 65, z: 0 },
            max_nodes: 64,
        })
        .expect("path");

        assert_eq!(path.last(), Some(&Position { x: 1, y: 65, z: 0 }));
    }
}
