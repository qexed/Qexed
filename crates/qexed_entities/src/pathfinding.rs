//! A* 寻路（v4 play/pathfinding 迁移）。
//!
//! 通过 `WorldAccess` trait 查询方块状态，碰撞形状经 manager 的 BLOCK_SHAPES 全局源解析。
//! TODO(play): qexed_play 域落地后若寻路归 play 域，再从此处上移。

use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap},
};

use qexed_packet::net_types::Position;

use crate::context::{BlockCollisionShape, WorldAccess};

const DEFAULT_MAX_NODES: usize = 4096;
const MAX_STEP_UP_BLOCKS: i32 = 1;
const MAX_DROP_BLOCKS: i32 = 3;
const STEPABLE_COLLISION_HEIGHT: f64 = 0.6;

#[derive(Clone)]
pub struct PathQuery<'a> {
    pub world: &'a dyn WorldAccess,
    pub dimension: &'a str,
    pub start: Position,
    pub goal: Position,
    pub max_nodes: usize,
    pub cached_only: bool,
}

pub fn find_path(query: PathQuery<'_>) -> Option<Vec<Position>> {
    let max_nodes = query.max_nodes.max(1).min(DEFAULT_MAX_NODES);
    let mut walkable = HashMap::with_capacity(max_nodes);
    let start = PositionKey::from(&query.start);
    let goal = PositionKey::from(&query.goal);
    if !is_walkable_cached(
        query.world,
        query.dimension,
        start,
        query.cached_only,
        &mut walkable,
    ) || !is_walkable_cached(
        query.world,
        query.dimension,
        goal,
        query.cached_only,
        &mut walkable,
    ) {
        return None;
    }

    let mut frontier = BinaryHeap::new();
    let mut came_from = HashMap::<PositionKey, Option<PositionKey>>::with_capacity(max_nodes);
    let mut cost_so_far = HashMap::<PositionKey, i32>::with_capacity(max_nodes);
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
            query.cached_only,
            &mut walkable,
        ) {
            if !is_walkable_cached(
                query.world,
                query.dimension,
                next,
                query.cached_only,
                &mut walkable,
            ) {
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
    world: &dyn WorldAccess,
    dimension: &str,
    position: PositionKey,
    goal: PositionKey,
    cached_only: bool,
    walkable: &mut HashMap<PositionKey, bool>,
) -> Vec<PositionKey> {
    let mut neighbours = Vec::with_capacity(20);
    let (offsets, offset_len) = candidate_y_offsets(position, goal);
    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        for dy in offsets[..offset_len].iter().copied() {
            let candidate = PositionKey {
                x: position.x + dx,
                y: position.y + dy,
                z: position.z + dz,
            };
            if is_walkable_cached(world, dimension, candidate, cached_only, walkable) {
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

fn candidate_y_offsets(position: PositionKey, goal: PositionKey) -> ([i32; 5], usize) {
    let mut offsets = [0; 5];
    let mut len = 0usize;
    let preferred_vertical = (goal.y - position.y).clamp(-MAX_DROP_BLOCKS, MAX_STEP_UP_BLOCKS);
    push_unique_offset(&mut offsets, &mut len, preferred_vertical);
    for offset in 0..=MAX_STEP_UP_BLOCKS {
        push_unique_offset(&mut offsets, &mut len, offset);
    }
    for offset in 1..=MAX_DROP_BLOCKS {
        push_unique_offset(&mut offsets, &mut len, -offset);
    }
    (offsets, len)
}

fn push_unique_offset(offsets: &mut [i32; 5], len: &mut usize, offset: i32) {
    if offsets[..*len].contains(&offset) || *len >= offsets.len() {
        return;
    }
    offsets[*len] = offset;
    *len += 1;
}

fn is_walkable_cached(
    world: &dyn WorldAccess,
    dimension: &str,
    position: PositionKey,
    cached_only: bool,
    cache: &mut HashMap<PositionKey, bool>,
) -> bool {
    if let Some(walkable) = cache.get(&position) {
        return *walkable;
    }
    let walkable = is_walkable(world, dimension, &position.position(), cached_only);
    cache.insert(position, walkable);
    walkable
}

fn is_walkable(world: &dyn WorldAccess, dimension: &str, feet: &Position, cached_only: bool) -> bool {
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

    let feet_shape = collision_shape_at(world, dimension, feet, cached_only);
    let head_shape = collision_shape_at(world, dimension, &head, cached_only);
    let below_shape = collision_shape_at(world, dimension, &below, cached_only);

    let feet_clear = feet_shape.is_none_or(|shape| shape.max_y <= STEPABLE_COLLISION_HEIGHT);
    let head_clear = head_shape.is_none();
    let has_support = below_shape.is_some() || feet_shape.is_some_and(|shape| shape.max_y > 0.0);

    feet_clear && head_clear && has_support
}

fn collision_shape_at(
    world: &dyn WorldAccess,
    dimension: &str,
    position: &Position,
    cached_only: bool,
) -> Option<BlockCollisionShape> {
    let _ = cached_only;
    world
        .block_state_at(dimension, position)
        .and_then(crate::manager::block_collision_shape_for_state)
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
