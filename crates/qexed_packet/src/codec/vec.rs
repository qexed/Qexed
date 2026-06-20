use crate::{PacketCodec, net_types::VarInt, write_varint_len};
use bytes::BufMut as _;

impl<T> PacketCodec for Vec<T>
where
    T: PacketCodec,
{
    fn serialize(&self, w: &mut crate::PacketWriter) -> anyhow::Result<()> {
        write_varint_len(self.len(), "vec", w)?;
        for prop in self {
            prop.serialize(w)?;
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> anyhow::Result<()> {
        let mut len = VarInt::default();
        len.deserialize(r)?;
        if len.0 < 0 {
            return Err(anyhow::anyhow!("negative vec length: {}", len.0));
        }

        self.clear();
        let len = len.0 as usize;
        self.try_reserve(len)?;
        for _ in 0..len {
            let mut value = T::default();
            value.deserialize(r)?;
            self.push(value);
        }
        Ok(())
    }
}

impl<const N: usize> PacketCodec for [u8; N]
where
    [u8; N]: Default,
{
    fn serialize(&self, w: &mut crate::PacketWriter) -> anyhow::Result<()> {
        w.buf.put_slice(self);
        Ok(())
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> anyhow::Result<()> {
        r.buf.copy_to_slice(self);
        Ok(())
    }
}

impl<T> PacketCodec for Box<T>
where
    T: PacketCodec + Default,
{
    fn serialize(&self, w: &mut crate::PacketWriter) -> anyhow::Result<()> {
        self.as_ref().serialize(w)
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> anyhow::Result<()> {
        let mut value = T::default();
        value.deserialize(r)?;
        *self = Box::new(value);
        Ok(())
    }
}
