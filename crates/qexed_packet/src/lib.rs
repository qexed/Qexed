use bytes::{Buf, BytesMut};
use thiserror::Error;

pub mod codec;
pub mod net_types;
mod packet_doc;
pub use packet_doc::{PacketDoc, PacketDocSubmit, collect_packet_docs};

#[derive(Error, Debug)]
pub enum PacketError {
    #[error("Invalid VarInt")]
    InvalidVarInt,
    #[error("Invalid VarLong - too many bytes")]
    InvalidVarLong,
    #[error("Incomplete packet")]
    IncompletePacket,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("nbt error: {0}")]
    Nbt(#[from] qexed_nbt::NbtError),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("utf8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error("{0}")]
    Message(String),
}

impl From<DecodeError> for PacketError {
    fn from(value: DecodeError) -> Self {
        match value {
            DecodeError::InvalidVarInt => Self::InvalidVarInt,
            DecodeError::InvalidVarLong => Self::InvalidVarLong,
            DecodeError::IncompletePacket => Self::IncompletePacket,
        }
    }
}

impl PacketError {
    pub fn msg(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}

pub type Result<T> = std::result::Result<T, PacketError>;

pub trait Packet: std::fmt::Debug + Send + Sync + Clone + Default {
    const ID: i32;
    fn serialize(&self, w: &mut PacketWriter) -> crate::Result<()>;
    fn deserialize(&mut self, r: &mut PacketReader) -> crate::Result<()>;
}

pub trait PacketCodec: std::fmt::Debug + Send + Sync + Default {
    fn serialize(&self, w: &mut PacketWriter) -> crate::Result<()>;
    fn deserialize(&mut self, r: &mut PacketReader) -> crate::Result<()>;
}

pub struct PacketReader<'a> {
    pub buf: &'a mut dyn Buf,
}

impl<'a> PacketReader<'a> {
    pub fn new(buf: &'a mut dyn Buf) -> Self {
        Self { buf }
    }
    pub fn deserialize<T: PacketCodec>(&mut self) -> crate::Result<T> {
        let mut t: T = Default::default();
        t.deserialize(self)?;
        Ok(t)
    }
}

pub struct PacketWriter<'a> {
    pub buf: &'a mut BytesMut,
}

impl<'a> PacketWriter<'a> {
    pub fn new(buf: &'a mut BytesMut) -> Self {
        Self { buf }
    }
    pub fn serialize<T: PacketCodec>(&mut self, value: &T) -> crate::Result<()> {
        value.serialize(self)
    }
}

#[derive(Error, Debug)]
pub enum DecodeError {
    #[error("Invalid VarInt")]
    InvalidVarInt,
    #[error("Invalid VarLong - too many bytes")]
    InvalidVarLong,
    #[error("Incomplete packet")]
    IncompletePacket,
}
