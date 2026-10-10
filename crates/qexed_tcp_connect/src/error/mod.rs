use std::io;

use qexed_packet::error::PacketError;

#[derive(Debug, thiserror::Error)]
pub enum TcpConnectError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("Config Error: {0}")]
    ConfigError(#[from] qexed_config::error::ConfigError),
    #[error("Addr Parse Error: {0}")]
    AddrParseError(#[from] std::net::AddrParseError),

    // ---------------- packet read ----------------
    #[error("packet read varint parse error: {0}")]
    PacketReadVarIntParseError(#[from] PacketReadVarIntParseError),
    #[error("connection closed with incomplete packet")]
    ConnectionClosedWithIncompletePacket,
    #[error("invalid packet length: {0}")]
    InvalidPacketLength(i32),
    #[error("packet too large: size = {size}, max = {max}")]
    PacketTooLarge { size: usize, max: usize },
    #[error("invalid compressed data length: {0}")]
    InvalidCompressedDataLength(i32),
    #[error("compressed data length too small: size = {size}, threshold = {threshold}")]
    CompressedDataLengthTooSmall { size: i32, threshold: i32 },
    #[error("decompression error: {0}")]
    DecompressionError(io::Error),
    #[error("decompression size mismatch: expected = {expected}, actual = {actual}")]
    DecompressionSizeMismatch { expected: usize, actual: usize },

    // ---------------- packet write ----------------
    #[error("compression error: {0}")]
    CompressionError(io::Error),
    #[error("packet encode error: {0}")]
    PacketEncodeError(#[from] PacketError),

    // ---------------- shared crypto ----------------
    #[error("crypto error: {0}")]
    CryptoError(String),
    #[error("invalid encryption key length: {length}")]
    InvalidEncryptionKeyLength { length: usize },
}

#[derive(Debug, thiserror::Error)]
pub enum PacketReadVarIntParseError {
    #[error("incomplete varint")]
    IncompleteError,
    #[error("varint too large")]
    TooLargeError,
    #[error("varint read error: {0}")]
    ReadError(#[from] io::Error),
}