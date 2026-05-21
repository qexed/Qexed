use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProtocolDecodeError {
    #[error("Packet ID mismatch: expected {expected}, got {got}")]
    PacketIdMismatch { expected: u32, got: u32 },
}