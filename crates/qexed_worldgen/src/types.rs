use crate::cache::WorldgenCache;

#[derive(Debug, Clone)]
pub struct WorldGenerator {
    pub(crate) cache: WorldgenCache,
    pub(crate) seed: i64,
}

#[derive(Debug, Clone)]
pub struct ChunkRequest<'a> {
    pub dimension: &'a str,
    pub chunk_x: i32,
    pub chunk_z: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkComparison {
    pub equal: bool,
    pub differences: Vec<String>,
}
