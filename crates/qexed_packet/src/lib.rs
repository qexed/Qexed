pub mod error;
use bytes::{Buf, BytesMut};

use crate::error::PacketError;

pub mod codec;
pub mod net_types;

pub trait Packet: std::fmt::Debug + Send + Sync + Clone + Default {
    const ID: i32;
    fn serialize(&self, w: &mut PacketWriter) -> Result<(),PacketError>;
    fn deserialize(&mut self, r: &mut PacketReader) -> Result<(),PacketError>;
}
pub trait PacketCodec: std::fmt::Debug + Send + Sync + Default {
    fn serialize(&self, w: &mut PacketWriter) -> Result<(),PacketError>;
    fn deserialize(&mut self, r: &mut PacketReader) -> Result<(),PacketError>;
}
pub struct PacketReader<'a> {
    pub buf: &'a mut dyn Buf,
}

impl<'a> PacketReader<'a> {
    pub fn new(buf: &'a mut dyn Buf) -> Self {
        Self { buf }
    }
    pub fn deserialize<T: PacketCodec>(&mut self) -> Result<T,PacketError> {
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
    pub fn serialize<T: PacketCodec>(&mut self, value: &T) -> Result<(),PacketError> {
        return value.serialize(self);
    }
}
