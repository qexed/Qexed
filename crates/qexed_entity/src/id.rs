use std::sync::atomic::{AtomicI32, Ordering};

#[derive(Debug)]
pub struct EntityIdAllocator {
    next_entity_id: AtomicI32,
}

impl Default for EntityIdAllocator {
    fn default() -> Self {
        Self::new(1)
    }
}

impl EntityIdAllocator {
    pub fn new(first_entity_id: i32) -> Self {
        Self {
            next_entity_id: AtomicI32::new(first_entity_id.max(1)),
        }
    }

    pub fn next(&self) -> i32 {
        self.next_entity_id.fetch_add(1, Ordering::Relaxed)
    }
}
