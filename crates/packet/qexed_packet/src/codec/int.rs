// Int 类型的 解析

use crate::PacketCodec;
use bytes::BufMut as _;
impl PacketCodec for u8 {
    fn serialize(&self, w: &mut crate::PacketWriter) -> anyhow::Result<()> {
        Ok(w.buf.put_u8(*self))
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> anyhow::Result<()> {
        Ok(*self = r.buf.get_u8())
    }
}
impl PacketCodec for u16 {
    fn serialize(&self, w: &mut crate::PacketWriter) -> anyhow::Result<()> {
        Ok(w.buf.put_u16(*self))
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> anyhow::Result<()> {
        Ok(*self = r.buf.get_u16())
    }
}
impl PacketCodec for u32 {
    fn serialize(&self, w: &mut crate::PacketWriter) -> anyhow::Result<()> {
        Ok(w.buf.put_u32(*self))
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> anyhow::Result<()> {
        Ok(*self = r.buf.get_u32())
    }
}
impl PacketCodec for u64 {
    fn serialize(&self, w: &mut crate::PacketWriter) -> anyhow::Result<()> {
        Ok(w.buf.put_u64(*self))
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> anyhow::Result<()> {
        Ok(*self = r.buf.get_u64())
    }
}

impl PacketCodec for i8 {
    fn serialize(&self, w: &mut crate::PacketWriter) -> anyhow::Result<()> {
        Ok(w.buf.put_i8(*self))
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> anyhow::Result<()> {
        Ok(*self = r.buf.get_i8())
    }
}
impl PacketCodec for i16 {
    fn serialize(&self, w: &mut crate::PacketWriter) -> anyhow::Result<()> {
        Ok(w.buf.put_i16(*self))
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> anyhow::Result<()> {
        Ok(*self = r.buf.get_i16())
    }
}
impl PacketCodec for i32 {
    fn serialize(&self, w: &mut crate::PacketWriter) -> anyhow::Result<()> {
        Ok(w.buf.put_i32(*self))
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> anyhow::Result<()> {
        Ok(*self = r.buf.get_i32())
    }
}
impl PacketCodec for i64 {
    fn serialize(&self, w: &mut crate::PacketWriter) -> anyhow::Result<()> {
        Ok(w.buf.put_i64(*self))
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> anyhow::Result<()> {
        Ok(*self = r.buf.get_i64())
    }
}
