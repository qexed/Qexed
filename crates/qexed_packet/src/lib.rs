use bytes::{Buf, BytesMut};
use thiserror::Error;
pub mod codec;
pub mod net_types;

pub trait Packet: std::fmt::Debug + Send + Sync + Clone + Default  {
    const ID: u32;
    fn serialize(&self, w: &mut PacketWriter) -> anyhow::Result<()>;
    fn deserialize(&mut self, r: &mut PacketReader) -> anyhow::Result<()>;
}
pub trait PacketCodec: std::fmt::Debug + Send + Sync + Default {
    fn serialize(&self, w: &mut PacketWriter) -> anyhow::Result<()>;
    fn deserialize(&mut self, r: &mut PacketReader) -> anyhow::Result<()>;
}
pub struct PacketReader<'a> {
    pub buf: Box<&'a mut (dyn Buf + Send + Sync)>,
}

impl<'a> PacketReader<'a> {
    pub fn new(buf: Box<&'a mut (dyn Buf + Send + Sync)>) -> Self {
        Self { buf }
    } 
    pub fn deserialize<T: PacketCodec>(&mut self) -> anyhow::Result<T> {
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
    pub fn serialize<T: PacketCodec>(&mut self, value: &T) -> anyhow::Result<()> {
        return value.serialize(self);
    }
}
#[derive(Error, Debug)]
pub enum DecodeError {
    #[error("Invalid VarInt")]
    InvalidVarInt,
    #[error("Invalid VarLong - too many bytes")]
    InvalidVarLong,
}