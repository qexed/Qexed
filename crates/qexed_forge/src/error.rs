/// qexed_forge 错误。
#[derive(Debug, thiserror::Error)]
pub enum ForgeError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("packet codec error: {0}")]
    Packet(#[from] qexed_packet::error::PacketError),

    #[error("unexpected handshake discriminator: {0}")]
    UnexpectedDiscriminator(u8),

    #[error("unknown forge channel: {0}")]
    UnknownChannel(String),

    #[error("mod list rejected: {0}")]
    ModListRejected(String),
}
