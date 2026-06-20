use bytes::{Buf, BufMut as _, BytesMut};
use thiserror::Error;
pub mod codec;
pub mod net_types;

pub trait Packet: std::fmt::Debug + Send + Sync + Clone + Default {
    const ID: i32;
    fn serialize(&self, w: &mut PacketWriter) -> anyhow::Result<()>;
    fn deserialize(&mut self, r: &mut PacketReader) -> anyhow::Result<()>;
}
pub trait PacketCodec: std::fmt::Debug + Send + Sync + Default {
    fn serialize(&self, w: &mut PacketWriter) -> anyhow::Result<()>;
    fn deserialize(&mut self, r: &mut PacketReader) -> anyhow::Result<()>;
}
pub struct PacketReader<'a> {
    pub buf: &'a mut dyn Buf,
}

impl<'a> PacketReader<'a> {
    pub fn new(buf: &'a mut dyn Buf) -> Self {
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

pub(crate) fn write_varint_len(
    len: usize,
    name: &str,
    writer: &mut PacketWriter,
) -> anyhow::Result<()> {
    if len > i32::MAX as usize {
        return Err(anyhow::anyhow!("{name} length {len} exceeds VarInt max"));
    }

    write_varint_value(len as i32, writer);
    Ok(())
}

pub(crate) fn write_varint_value(value: i32, writer: &mut PacketWriter) {
    let mut val = value as u32;
    loop {
        let mut temp = (val & 0x7F) as u8;
        val >>= 7;
        if val != 0 {
            temp |= 0x80;
        }
        writer.buf.put_u8(temp);
        if val == 0 {
            break;
        }
    }
}

pub(crate) fn write_varlong_value(value: i64, writer: &mut PacketWriter) {
    let mut val = value as u64;
    loop {
        let mut temp = (val & 0x7F) as u8;
        val >>= 7;
        if val != 0 {
            temp |= 0x80;
        }
        writer.buf.put_u8(temp);
        if val == 0 {
            break;
        }
    }
}

pub(crate) fn read_exact_vec(buf: &mut dyn Buf, len: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(len);
    append_exact_vec(buf, len, &mut bytes);
    bytes
}

pub(crate) fn read_exact_vec_into(
    buf: &mut dyn Buf,
    len: usize,
    bytes: &mut Vec<u8>,
) -> anyhow::Result<()> {
    bytes.clear();
    bytes.try_reserve(len)?;
    append_exact_vec(buf, len, bytes);
    Ok(())
}

fn append_exact_vec(buf: &mut dyn Buf, len: usize, bytes: &mut Vec<u8>) {
    let mut remaining = len;
    while remaining > 0 {
        let chunk = buf.chunk();
        let take = chunk.len().min(remaining);
        assert!(take > 0, "buffer underflow while reading {len} bytes");
        bytes.extend_from_slice(&chunk[..take]);
        buf.advance(take);
        remaining -= take;
    }
}
