use crate::PacketCodec;
use bytes::BufMut as _;

impl<T> PacketCodec for Option<T>
where
    T: PacketCodec,
{
    fn serialize(&self, w: &mut crate::PacketWriter) -> anyhow::Result<()> {
        if let Some(t) = self {
            w.buf.put_u8(1);
            t.serialize(w)?;
        } else {
            w.buf.put_u8(0);
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> anyhow::Result<()> {
        if r.buf.get_u8() != 0 {
            let mut value = T::default();
            value.deserialize(r)?;
            *self = Some(value);
        } else {
            *self = None;
        }
        Ok(())
    }
}
