use std::collections::{HashMap, HashSet};

pub type ChunkPosition = (i32, i32);

#[derive(Debug, Clone)]
pub struct PlayerChunkView {
    center_chunk_x: i32,
    center_chunk_z: i32,
    radius: i32,
    sent_chunks: HashSet<ChunkPosition>,
    pending_unloads: HashMap<ChunkPosition, std::time::Instant>,
}

impl PlayerChunkView {
    pub fn new(view_distance: i32) -> Self {
        Self {
            center_chunk_x: 0,
            center_chunk_z: 0,
            radius: chunk_view_radius(view_distance),
            sent_chunks: HashSet::new(),
            pending_unloads: HashMap::new(),
        }
    }

    pub fn center_chunk_x(&self) -> i32 {
        self.center_chunk_x
    }

    pub fn center_chunk_z(&self) -> i32 {
        self.center_chunk_z
    }

    pub fn radius(&self) -> i32 {
        self.radius
    }

    pub fn sent_chunks(&self) -> &HashSet<ChunkPosition> {
        &self.sent_chunks
    }

    pub fn mark_chunk_sent(&mut self, chunk: ChunkPosition) {
        self.sent_chunks.insert(chunk);
    }

    pub fn extend_sent_chunks(&mut self, chunks: impl IntoIterator<Item = ChunkPosition>) {
        self.sent_chunks.extend(chunks);
    }

    pub fn set_center(&mut self, chunk_x: i32, chunk_z: i32) {
        self.center_chunk_x = chunk_x;
        self.center_chunk_z = chunk_z;
    }

    pub fn is_complete(&self) -> bool {
        chunk_window_positions(self.center_chunk_x, self.center_chunk_z, self.radius)
            .iter()
            .all(|chunk| self.sent_chunks.contains(chunk))
    }

    pub fn has_pending_unloads(&self) -> bool {
        !self.pending_unloads.is_empty()
    }

    pub fn mark_delayed_unloads(
        &mut self,
        full_window: &HashSet<ChunkPosition>,
        unload_at: std::time::Instant,
    ) -> Vec<ChunkPosition> {
        self.pending_unloads
            .retain(|chunk, _| self.sent_chunks.contains(chunk) && !full_window.contains(chunk));

        let mut leaving = Vec::new();
        for chunk in &self.sent_chunks {
            if full_window.contains(chunk) {
                self.pending_unloads.remove(chunk);
            } else {
                self.pending_unloads.entry(*chunk).or_insert(unload_at);
                leaving.push(*chunk);
            }
        }
        leaving.sort_unstable();
        leaving
    }

    pub fn expired_unloads(&mut self, now: std::time::Instant) -> Vec<ChunkPosition> {
        let full_window =
            chunk_window_positions(self.center_chunk_x, self.center_chunk_z, self.radius);
        let mut expired = self
            .pending_unloads
            .iter()
            .filter_map(|(chunk, unload_at)| {
                (*unload_at <= now
                    && self.sent_chunks.contains(chunk)
                    && !full_window.contains(chunk))
                .then_some(*chunk)
            })
            .collect::<Vec<_>>();
        expired.sort_unstable();
        for chunk in &expired {
            self.pending_unloads.remove(chunk);
            self.sent_chunks.remove(chunk);
        }
        expired
    }
}

pub fn chunk_view_radius(view_distance: i32) -> i32 {
    view_distance.max(1)
}

pub fn player_chunk_coordinate(position: f64) -> i32 {
    (position.floor() as i32).div_euclid(16)
}

pub fn chunk_window_positions(
    center_chunk_x: i32,
    center_chunk_z: i32,
    radius: i32,
) -> HashSet<ChunkPosition> {
    let mut chunks = HashSet::new();
    for chunk_z in center_chunk_z - radius..=center_chunk_z + radius {
        for chunk_x in center_chunk_x - radius..=center_chunk_x + radius {
            chunks.insert((chunk_x, chunk_z));
        }
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::{PlayerChunkView, chunk_window_positions, player_chunk_coordinate};
    use std::time::Duration;

    #[test]
    fn player_position_maps_to_chunk_coordinate() {
        assert_eq!(player_chunk_coordinate(0.0), 0);
        assert_eq!(player_chunk_coordinate(15.99), 0);
        assert_eq!(player_chunk_coordinate(16.0), 1);
        assert_eq!(player_chunk_coordinate(-0.01), -1);
        assert_eq!(player_chunk_coordinate(-16.0), -1);
    }

    #[test]
    fn chunk_view_delays_and_cancels_unloads() {
        let mut chunk_view = PlayerChunkView::new(1);
        chunk_view.mark_chunk_sent((-1, 0));
        chunk_view.mark_chunk_sent((0, 0));

        let unload_at = std::time::Instant::now() + Duration::from_secs(4);
        let shifted_window = chunk_window_positions(2, 0, 1);
        let leaving = chunk_view.mark_delayed_unloads(&shifted_window, unload_at);

        assert_eq!(leaving, vec![(-1, 0), (0, 0)]);
        assert!(chunk_view.sent_chunks().contains(&(-1, 0)));
        assert!(chunk_view.has_pending_unloads());
        assert!(
            chunk_view
                .expired_unloads(unload_at - Duration::from_millis(1))
                .is_empty()
        );

        let original_window = chunk_window_positions(0, 0, 1);
        chunk_view.mark_delayed_unloads(&original_window, unload_at);

        assert!(!chunk_view.has_pending_unloads());
        assert!(chunk_view.sent_chunks().contains(&(-1, 0)));
    }
}
