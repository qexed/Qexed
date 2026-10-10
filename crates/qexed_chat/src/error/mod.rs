#[derive(Debug, thiserror::Error)]
pub enum ChatError {
    // ── 外部 crate 源错误 ──
    #[error("crypto error: {0}")]
    Crypto(#[from] openssl::error::ErrorStack),

    // ── SecureChatSession::new / verify_message ──
    #[error("missing signed chat message signature")]
    MissingSignature,
    #[error("out-of-order signed chat timestamp")]
    OutOfOrderTimestamp,
    #[error("invalid signed chat message signature")]
    InvalidSignature,
    #[error("signed chat message index overflow")]
    IndexOverflow,

    // ── last-seen validator ──
    #[error("negative signed chat last-seen offset: {0}")]
    NegativeOffset(i32),
    #[error("last-seen offset {offset} exceeds max {max}")]
    OffsetExceedsWindow { offset: usize, max: usize },
    #[error("last-seen window missing entry")]
    MissingWindowEntry,
    #[error("last-seen update acknowledged unknown message at index {0}")]
    AcknowledgedUnknown(usize),
    #[error("last-seen update ignored previously acknowledged message at index {0}")]
    IgnoredAcknowledged(usize),
    #[error("last-seen checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: u8, actual: u8 },
}