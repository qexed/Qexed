// bool 类型处理
use crate::PacketCodec;
// 1:True,0:False
impl PacketCodec for bool {
    fn serialize(&self, w: &mut crate::PacketWriter) -> anyhow::Result<()> {
        (*self as u8).serialize(w)
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> anyhow::Result<()> {
        let mut v = 0u8;
        v.deserialize(r)?;
        *self = v != 0;
        Ok(())
    }
}