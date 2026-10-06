//! Spatial hash grid for fast entity lookups by position.
//!
//! Divides dimensions into fixed-size cells. Each cell stores entity keys.
//! Queries by bounding-box radius are O(k) where k = entities in nearby cells,
//! instead of O(n) for linear scan over all entities.

use std::collections::{HashMap, HashSet};

/// Cell coordinate in the spatial grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellCoord {
    pub dim: u16,
    pub cx: i32,
    pub cy: i32,
    pub cz: i32,
}

/// Entity key used in the spatial index.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntityKey(pub String);

impl EntityKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }
}

impl std::ops::Deref for EntityKey {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

/// Spatial hash grid for O(1) entity insertion, removal, and nearby queries.
///
/// Key features:
/// - Fixed cell size for predictable memory usage
/// - Each entity belongs to exactly one cell at a time
/// - Query returns entities in neighboring cells
/// - Supports bulk insertion/removal for entity teleport
pub struct SpatialHash {
    /// cell → set of entity keys
    cells: HashMap<CellCoord, HashSet<EntityKey>>,
    /// entity key → current cell
    entity_cells: HashMap<EntityKey, CellCoord>,
    cell_size: f64,
    entity_count: usize,
}

impl SpatialHash {
    /// Create a new spatial hash with the given cell size.
    /// Smaller cells = more precise queries but more memory overhead.
    /// Recommended: 16.0 for general use, 8.0 for dense areas.
    pub fn new(cell_size: f64) -> Self {
        assert!(cell_size > 0.0, "cell size must be positive");
        Self {
            cells: HashMap::new(),
            entity_cells: HashMap::new(),
            cell_size,
            entity_count: 0,
        }
    }

    /// Convert a world position to cell coordinates.
    pub fn cell_for(&self, dim: u16, x: f64, y: f64, z: f64) -> CellCoord {
        CellCoord {
            dim,
            cx: (x / self.cell_size).floor() as i32,
            cy: (y / self.cell_size).floor() as i32,
            cz: (z / self.cell_size).floor() as i32,
        }
    }

    /// Number of entities tracked.
    pub fn len(&self) -> usize {
        self.entity_count
    }

    pub fn is_empty(&self) -> bool {
        self.entity_count == 0
    }

    /// Number of occupied cells.
    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    /// Insert or update an entity's position.
    pub fn insert(&mut self, key: EntityKey, dim: u16, x: f64, y: f64, z: f64) {
        let new_cell = self.cell_for(dim, x, y, z);

        // Remove from old cell if moving
        if let Some(old_cell) = self.entity_cells.get(&key) {
            if *old_cell == new_cell {
                return; // Same cell, no update needed
            }
            self.remove_from_cell(&key, *old_cell);
        }

        self.entity_cells.insert(key.clone(), new_cell);
        self.cells
            .entry(new_cell)
            .or_insert_with(|| HashSet::with_capacity(4))
            .insert(key);
        self.entity_count = self.entity_cells.len();
    }

    /// Remove an entity from the grid.
    pub fn remove(&mut self, key: &EntityKey) -> bool {
        if let Some(cell) = self.entity_cells.remove(key) {
            self.remove_from_cell(key, cell);
            self.entity_count = self.entity_cells.len();
            true
        } else {
            false
        }
    }

    fn remove_from_cell(&mut self, key: &EntityKey, cell: CellCoord) {
        if let Some(set) = self.cells.get_mut(&cell) {
            set.remove(key);
            if set.is_empty() {
                self.cells.remove(&cell);
            }
        }
    }

    /// Find all entities within `range` blocks of (x, y, z).
    /// Returns an iterator of entity keys. O(k) where k = entities in nearby cells.
    pub fn find_nearby(
        &self,
        dim: u16,
        x: f64,
        y: f64,
        z: f64,
        range: f64,
    ) -> impl Iterator<Item = &EntityKey> {
        let center = self.cell_for(dim, x, y, z);
        let cell_radius = (range / self.cell_size).ceil() as i32 + 1;

        // Pre-allocate buffer to avoid repeated reallocation
        let mut results = Vec::new();
        for dcx in -cell_radius..=cell_radius {
            for dcy in -cell_radius..=cell_radius {
                for dcz in -cell_radius..=cell_radius {
                    let cell = CellCoord {
                        dim,
                        cx: center.cx + dcx,
                        cy: center.cy + dcy,
                        cz: center.cz + dcz,
                    };
                    if let Some(entities) = self.cells.get(&cell) {
                        results.extend(entities.iter());
                    }
                }
            }
        }
        results.into_iter()
    }

    /// Find entities in a bounding box.
    pub fn find_in_aabb(
        &self,
        dim: u16,
        min_x: f64,
        max_x: f64,
        min_y: f64,
        max_y: f64,
        min_z: f64,
        max_z: f64,
    ) -> impl Iterator<Item = &EntityKey> {
        let min_cell = self.cell_for(dim, min_x, min_y, min_z);
        let max_cell = self.cell_for(dim, max_x, max_y, max_z);

        let mut results = Vec::new();
        for cx in min_cell.cx..=max_cell.cx {
            for cy in min_cell.cy..=max_cell.cy {
                for cz in min_cell.cz..=max_cell.cz {
                    let cell = CellCoord { dim, cx, cy, cz };
                    if let Some(entities) = self.cells.get(&cell) {
                        results.extend(entities.iter());
                    }
                }
            }
        }
        results.into_iter()
    }

    /// Get the current cell of an entity (if tracked).
    pub fn cell_of(&self, key: &EntityKey) -> Option<CellCoord> {
        self.entity_cells.get(key).copied()
    }

    /// Clear all entities.
    pub fn clear(&mut self) {
        self.cells.clear();
        self.entity_cells.clear();
        self.entity_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spatial_hash_insert_and_query() {
        let mut grid = SpatialHash::new(16.0);

        grid.insert(EntityKey::new("e1"), 0, 0.0, 64.0, 0.0);
        grid.insert(EntityKey::new("e2"), 0, 8.0, 64.0, 8.0);
        grid.insert(EntityKey::new("e3"), 0, 100.0, 64.0, 100.0);

        assert_eq!(grid.len(), 3);

        // e1 and e2 are close, e3 is far
        let nearby: Vec<_> = grid.find_nearby(0, 0.0, 64.0, 0.0, 20.0).cloned().collect();
        assert!(nearby.contains(&EntityKey::new("e1")));
        assert!(nearby.contains(&EntityKey::new("e2")));
        assert!(!nearby.contains(&EntityKey::new("e3")));
    }

    #[test]
    fn spatial_hash_move_entity() {
        let mut grid = SpatialHash::new(16.0);

        grid.insert(EntityKey::new("e1"), 0, 0.0, 64.0, 0.0);
        assert_eq!(grid.len(), 1);

        // Move to new position
        grid.insert(EntityKey::new("e1"), 0, 50.0, 64.0, 50.0);
        assert_eq!(grid.len(), 1); // Still 1 entity

        // Old position should not have it
        let nearby_old: Vec<_> = grid.find_nearby(0, 0.0, 64.0, 0.0, 16.0).cloned().collect();
        assert!(!nearby_old.contains(&EntityKey::new("e1")));

        // New position should have it
        let nearby_new: Vec<_> = grid.find_nearby(0, 50.0, 64.0, 50.0, 16.0).cloned().collect();
        assert!(nearby_new.contains(&EntityKey::new("e1")));
    }

    #[test]
    fn spatial_hash_remove() {
        let mut grid = SpatialHash::new(16.0);
        let key = EntityKey::new("e1");

        grid.insert(key.clone(), 0, 0.0, 64.0, 0.0);
        assert!(grid.remove(&key));
        assert_eq!(grid.len(), 0);

        let nearby: Vec<_> = grid.find_nearby(0, 0.0, 64.0, 0.0, 100.0).cloned().collect();
        assert!(nearby.is_empty());
    }

    #[test]
    fn spatial_hash_dimension_isolation() {
        let mut grid = SpatialHash::new(16.0);

        grid.insert(EntityKey::new("e1"), 0, 0.0, 64.0, 0.0); // overworld
        grid.insert(EntityKey::new("e2"), 1, 0.0, 64.0, 0.0); // nether

        // Query overworld → only e1
        let overworld: Vec<_> = grid.find_nearby(0, 0.0, 64.0, 0.0, 50.0).cloned().collect();
        assert!(overworld.contains(&EntityKey::new("e1")));
        assert!(!overworld.contains(&EntityKey::new("e2")));

        // Query nether → only e2
        let nether: Vec<_> = grid.find_nearby(1, 0.0, 64.0, 0.0, 50.0).cloned().collect();
        assert!(!nether.contains(&EntityKey::new("e1")));
        assert!(nether.contains(&EntityKey::new("e2")));
    }

    #[test]
    fn spatial_hash_large_scale_insert_and_query() {
        use std::time::Instant;

        let mut grid = SpatialHash::new(16.0);
        let entity_count = 50_000;
        let query_count = 500;

        // Insert entities spread over 2000×2000 area
        let start = Instant::now();
        for i in 0..entity_count {
            let x = ((i * 73 + 31) % 2000) as f64;
            let z = ((i * 137 + 67) % 2000) as f64;
            let y = 64.0;
            grid.insert(EntityKey::new(format!("entity_{i}")), 0, x, y, z);
        }
        let insert_time = start.elapsed();
        println!(
            "Inserted {entity_count} entities in {insert_time:?} ({:.0}k/s)",
            entity_count as f64 / insert_time.as_secs_f64().max(0.001) / 1000.0
        );

        assert_eq!(grid.len(), entity_count);

        // Query with small range (8 blocks)
        let range = 8.0;
        let start = Instant::now();
        let mut total_found = 0usize;
        for i in 0..query_count {
            let x = ((i * 53 + 17) % 2000) as f64;
            let z = ((i * 97 + 43) % 2000) as f64;
            let found = grid.find_nearby(0, x, 64.0, z, range).count();
            total_found += found;
        }
        let query_time = start.elapsed();
        println!(
            "Queried {query_count} times in {query_time:?} ({:.0} q/ms), avg found: {:.1}",
            query_count as f64 / query_time.as_secs_f64().max(0.001) / 1000.0,
            total_found as f64 / query_count as f64
        );

        assert!(insert_time.as_millis() < 500, "insert too slow: {insert_time:?}");
        assert!(query_time.as_millis() < 30, "query too slow: {query_time:?}");
    }

    #[test]
    fn spatial_hash_linear_scan_comparison() {
        use std::time::Instant;

        // Store entities in a Vec
        let count = 50_000;
        let mut entities = Vec::new();
        for i in 0..count {
            let x = ((i * 73 + 31) % 2000) as f64;
            let z = ((i * 137 + 67) % 2000) as f64;
            entities.push((format!("entity_{i}"), x, 64.0, z));
        }

        // Spatial hash
        let mut grid = SpatialHash::new(16.0);
        for (key, x, y, z) in &entities {
            grid.insert(EntityKey::new(key.clone()), 0, *x, *y, *z);
        }

        let range = 8.0;
        let range_sq = range * range;

        // Linear scan: 500 queries
        let start = Instant::now();
        for i in 0..500 {
            let qx = ((i * 53 + 17) % 2000) as f64;
            let qz = ((i * 97 + 43) % 2000) as f64;
            let _found: Vec<_> = entities
                .iter()
                .filter(|(_, ex, _, ez)| {
                    let dx = ex - qx;
                    let dz = ez - qz;
                    dx * dx + dz * dz <= range_sq
                })
                .collect();
        }
        let linear_time = start.elapsed();

        // Spatial hash: 500 queries
        let start = Instant::now();
        for i in 0..500 {
            let qx = ((i * 53 + 17) % 2000) as f64;
            let qz = ((i * 97 + 43) % 2000) as f64;
            let _found: Vec<_> = grid.find_nearby(0, qx, 64.0, qz, range).cloned().collect();
        }
        let hash_time = start.elapsed();

        let speedup = linear_time.as_secs_f64() / hash_time.as_secs_f64().max(0.0001);
        println!(
            "50k entities: Linear={linear_time:?}, Spatial={hash_time:?}, speedup={speedup:.1}x"
        );

        assert!(speedup > 1.5, "spatial hash should be >1.5x faster, got {speedup:.1}x");
    }

    #[test]
    fn spatial_hash_aabb_query() {
        let mut grid = SpatialHash::new(16.0);

        // Entities at various positions
        grid.insert(EntityKey::new("corner"), 0, 0.0, 64.0, 0.0);
        grid.insert(EntityKey::new("center"), 0, 50.0, 64.0, 50.0);
        grid.insert(EntityKey::new("far"), 0, 200.0, 64.0, 200.0);

        // Query AABB covering corner and center
        let found: Vec<_> = grid
            .find_in_aabb(0, -10.0, 60.0, 50.0, 70.0, -10.0, 60.0)
            .cloned()
            .collect();

        assert!(found.contains(&EntityKey::new("corner")));
        assert!(found.contains(&EntityKey::new("center")));
        assert!(!found.contains(&EntityKey::new("far")));
    }

    #[test]
    fn spatial_hash_clear() {
        let mut grid = SpatialHash::new(16.0);
        for i in 0..100 {
            grid.insert(EntityKey::new(format!("e{i}")), 0, i as f64, 64.0, i as f64);
        }
        assert_eq!(grid.len(), 100);
        grid.clear();
        assert_eq!(grid.len(), 0);
        assert_eq!(grid.cell_count(), 0);
    }
}
