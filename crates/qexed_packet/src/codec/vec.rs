use crate::{PacketCodec, net_types::VarInt};

impl<T> PacketCodec for Vec<T>
where
    T: PacketCodec,
{
    fn serialize(&self, w: &mut crate::PacketWriter) -> anyhow::Result<()> {
        if self.len() > i32::MAX as usize {
            return Err(anyhow::anyhow!(
                "vec length {} exceeds VarInt max",
                self.len()
            ));
        }

        VarInt(self.len() as i32).serialize(w)?;
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
        self.reserve(len.0 as usize);
        for _ in 0..len.0 {
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
        for value in self {
            value.serialize(w)?
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> anyhow::Result<()> {
        for value in self {
            value.deserialize(r)?;
        }
        Ok(())
    }
}
