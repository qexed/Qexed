use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap},
};

use qexed_packet::net_types::Position;

use crate::world::WorldManager;

const DEFAULT_MAX_NODES: usize = 4096;

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

        for next in neighbours_toward(current.position, goal) {
            if !is_walkable_cached(query.world, query.dimension, next, &mut walkable) {
                continue;
            }
            let new_cost = current.cost + 1;
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

fn neighbours_toward(position: PositionKey, goal: PositionKey) -> [PositionKey; 4] {
    let mut neighbours = [
        PositionKey {
            x: position.x + 1,
            ..position
        },
        PositionKey {
            x: position.x - 1,
            ..position
        },
        PositionKey {
            z: position.z + 1,
            ..position
        },
        PositionKey {
            z: position.z - 1,
            ..position
        },
    ];
    neighbours.sort_by_key(|position| manhattan(*position, goal));
    neighbours
}

fn manhattan(left: PositionKey, right: PositionKey) -> i32 {
    (left.x - right.x).abs() + (left.y - right.y).abs() + (left.z - right.z).abs()
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

    let feet_state = world
        .block_state_at(dimension, feet)
        .unwrap_or_else(crate::inventory::air_block_state);
    let head_state = world
        .block_state_at(dimension, &head)
        .unwrap_or_else(crate::inventory::air_block_state);
    let below_state = world
        .block_state_at(dimension, &below)
        .unwrap_or_else(crate::inventory::air_block_state);

    !crate::inventory::block_has_collision(feet_state)
        && !crate::inventory::block_has_collision(head_state)
        && crate::inventory::block_has_collision(below_state)
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
