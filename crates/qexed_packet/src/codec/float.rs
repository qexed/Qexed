use crate::{PacketCodec, error::PacketError};
use bytes::BufMut as _;
impl PacketCodec for f32 {
    fn serialize(&self, w: &mut crate::PacketWriter) -> Result<(), PacketError> {
        Ok(w.buf.put_f32(*self))
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> Result<(), PacketError> {
        Ok(*self = r.buf.get_f32())
    }
}
impl PacketCodec for f64 {
    fn serialize(&self, w: &mut crate::PacketWriter) -> Result<(), PacketError> {
        Ok(w.buf.put_f64(*self))
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> Result<(), PacketError> {
        Ok(*self = r.buf.get_f64())
    }
}
