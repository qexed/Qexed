mod region;
mod chunk;



pub mod error {
    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum RegionError {
        #[error("IO error: {0}")]
        Io(#[from] std::io::Error),
        #[error("NBT error: {0}")]
        InvalidChunkCoordinates(i32, i32),
        #[error("Chunk not found at {0}, {1}")]
        ChunkNotFound(i32, i32),
        #[error("Chunk data corrupted")]
        ChunkDataCorrupted,
        #[error("Unsupported compression type: {0}")]
        UnsupportedCompression(u8),
        #[error("NBT serialization error: {0}")]
        NbtSerialization(String),
    }
}