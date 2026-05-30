use std::collections::{HashMap, VecDeque};

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
    if !is_walkable(query.world, query.dimension, &query.start)
        || !is_walkable(query.world, query.dimension, &query.goal)
    {
        return None;
    }

    let mut frontier = VecDeque::new();
    let mut came_from = HashMap::<PositionKey, Option<PositionKey>>::new();
    let start = PositionKey::from(&query.start);
    let goal = PositionKey::from(&query.goal);
    frontier.push_back(start);
    came_from.insert(start, None);

    while let Some(current) = frontier.pop_front() {
        if current == goal {
            return Some(reconstruct_path(start, goal, came_from));
        }
        if came_from.len() >= max_nodes {
            return None;
        }

        for next in neighbours(current) {
            if came_from.contains_key(&next) {
                continue;
            }
            let next_position = next.position();
            if !is_walkable(query.world, query.dimension, &next_position) {
                continue;
            }
            frontier.push_back(next);
            came_from.insert(next, Some(current));
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

fn neighbours(position: PositionKey) -> [PositionKey; 4] {
    [
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
    ]
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
