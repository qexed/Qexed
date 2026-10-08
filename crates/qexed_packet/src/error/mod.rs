#[derive(Debug, thiserror::Error)]
pub enum PacketError {
    // ========================================================================
    // 外部错误自动转换（#[from]）
    // ========================================================================
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("config error: {0}")]
    ConfigError(#[from] qexed_config::error::ConfigError),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("invalid UTF-8: {0}")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),

    // ========================================================================
    // VarInt / VarLong 编解码
    // ========================================================================
    #[error("incomplete packet: not enough bytes remaining")]
    IncompletePacket,

    #[error("invalid VarInt")]
    InvalidVarInt,

    #[error("invalid VarLong")]
    InvalidVarLong,

    #[error("optional varint value overflows")]
    OptionalVarIntOverflow,

    // ========================================================================
    // 通用长度校验
    // ========================================================================
    /// 负数长度：`negative {name} length: {value}`
    #[error("negative {name} length: {value}")]
    NegativeLength { name: &'static str, value: i32 },

    /// 长度超过剩余可读字节：`{name} length {len} exceeds remaining {remaining}`
    #[error("{name} length {len} exceeds remaining {remaining}")]
    LengthExceedsRemaining {
        name: &'static str,
        len: usize,
        remaining: usize,
    },

    /// 长度超过 VarInt 可表示的最大值：`{name} length {len} exceeds VarInt max`
    #[error("{name} length {len} exceeds VarInt max")]
    LengthExceedsVarIntMax { name: &'static str, len: usize },

    /// 长度超过 i32 可表示的最大值：`{name} length {len} exceeds i32 max`
    #[error("{name} length {len} exceeds i32 max")]
    LengthExceedsI32Max { name: &'static str, len: usize },

    /// 集合长度超过业务上限：`{name} length {count} exceeds max {max}`
    #[error("{name} length {count} exceeds max {max}")]
    LengthExceedsMax {
        name: &'static str,
        count: usize,
        max: usize,
    },

    // ========================================================================
    // 长度前缀负载
    // ========================================================================
    #[error("length-prefixed payload size {size} exceeds max {max}")]
    LengthPrefixedSizeExceedsMax { size: usize, max: usize },

    #[error("length-prefixed payload has {count} trailing bytes")]
    LengthPrefixedTrailingBytes { count: usize },

    // ========================================================================
    // 枚举判别值 / 必填字段 / 定长负载
    // ========================================================================

    /// 不支持的枚举判别值或类型 id：`unsupported {what}: {value}`
    #[error("unsupported {what}: {value}")]
    UnsupportedValue { what: &'static str, value: i32 },

    /// 序列化时缺少必填数据：`{what} is required`
    #[error("{what} is required")]
    MissingRequired { what: &'static str },

    /// 定长负载长度不符：`{what} length must be {expected}, got {got}`
    #[error("{what} length must be {expected}, got {got}")]
    InvalidLength {
        what: &'static str,
        expected: usize,
        got: usize,
    },

    // ========================================================================
    // NBT
    // ========================================================================
    #[error("missing NBT tag id")]
    MissingNbtTagId,

    #[error("NBT list header length {header_length} does not match item count {items_len}")]
    NbtListHeaderMismatch {
        header_length: i32,
        items_len: usize,
    },

    #[error("unknown NBT tag id: 0x{0:02X}")]
    UnknownNbtTagId(u8),

    #[error("NBT string is too long: {0}")]
    NbtStringTooLong(usize),
}