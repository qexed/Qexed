/// qexed_anvil 错误类型。
#[derive(Debug, thiserror::Error)]
pub enum AnvilError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("cannot open region file {path}: {source}")]
    OpenFailed {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("region file header too short: {path} has {len} bytes")]
    HeaderTooShort { path: String, len: usize },

    #[error("chunk offset inside region header")]
    InvalidOffset,

    #[error("chunk data out of region file bounds")]
    DataOutOfBounds,

    #[error("external .mcc chunk streams are not supported")]
    ExternalStreamUnsupported,

    #[error("unsupported chunk compression type: {0}")]
    UnsupportedCompression(u8),

    #[error("chunk payload too large for sector-based storage")]
    ChunkTooLarge,

    // ---- chunk NBT ----

    #[error("chunk NBT error: {0}")]
    Nbt(#[from] qexed_nbt::NbtError),

    #[error("chunk root tag is not a compound")]
    NotACompound,

    #[error("missing expected field: {0}")]
    MissingField(String),

    #[error("section y={0} is not a compound")]
    SectionNotCompound(i32),

    #[error("block index {index} out of section bounds")]
    IndexOutOfBounds { index: usize },

    #[error("invalid paletted container data length: got {got}, expected {expected}")]
    PaletteDataLength { got: usize, expected: usize },

    #[error("palette index {index} out of range for palette size {palette_len}")]
    PaletteIndexOutOfRange { index: usize, palette_len: usize },

    #[error("negative paletted value: {0}")]
    NegativePaletteValue(i64),

    #[error("paletted value {value} exceeds {bits} bits")]
    PaletteValueOverflow { value: u64, bits: usize },

    #[error("invalid block state count: got {got}, expected {expected}")]
    BlockStateCount { got: usize, expected: usize },

    #[error("missing paletted container data for palette size {0}")]
    MissingPaletteData(usize),
}
