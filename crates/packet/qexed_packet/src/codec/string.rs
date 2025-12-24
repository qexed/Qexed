use bytes::BufMut as _;
use crate::{PacketCodec, net_types::VarInt};
impl PacketCodec for String {
    fn serialize(&self, w: &mut crate::PacketWriter) -> anyhow::Result<()> {
        VarInt(self.len() as i32).serialize(w)?;
        w.buf.put_slice(self.as_bytes());
        Ok(())
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> anyhow::Result<()> {
        let mut len:VarInt = Default::default();
        len.deserialize(r)?;
        let bytes = r.buf.copy_to_bytes(len.0  as usize);
        *self =String::from_utf8_lossy(&bytes).to_string();
        Ok(())
    }
}