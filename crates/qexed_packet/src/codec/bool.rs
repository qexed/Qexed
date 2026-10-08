// bool 类型处理
use crate::{PacketCodec, error::PacketError};
// 1:True,0:False
impl PacketCodec for bool {
    fn serialize(&self, w: &mut crate::PacketWriter) -> Result<(), PacketError> {
        (*self as u8).serialize(w)
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> Result<(), PacketError> {
        let mut v = 0u8;
        v.deserialize(r)?;
        *self = v != 0;
        Ok(())
    }
}
