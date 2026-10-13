//! 区块流送错误类型。

#[derive(Debug, thiserror::Error)]
pub enum WorldError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("tcp write error: {0}")]
    TcpWrite(#[from] qexed_tcp_connect::error::TcpConnectError),

    #[error("registry error: {0}")]
    Registry(#[from] qexed_registry::error::RegistryError),

    #[error("mojang data error: {0}")]
    MojangData(#[from] qexed_mojang_data::error::MojangDataError),

    #[error("chunk tcp source error: {0}")]
    ChunkTcp(#[from] qexed_chunk_by_tcp::ChunkClientError),

    #[error("packet error: {0}")]
    Packet(#[from] qexed_packet::error::PacketError),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, WorldError>;
