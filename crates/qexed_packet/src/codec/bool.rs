// bool 类型处理
use crate::PacketCodec;
use bytes::BufMut as _;
// 1:True,0:False
impl PacketCodec for bool {
    fn serialize(&self, w: &mut crate::PacketWriter) -> anyhow::Result<()> {
        w.buf.put_u8(*self as u8);
        Ok(())
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> anyhow::Result<()> {
        *self = r.buf.get_u8() != 0;
        Ok(())
    }
}
