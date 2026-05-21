use crate::{PacketCodec, net_types::VarInt};
impl <T>PacketCodec for Vec<T> where T: PacketCodec, {
    fn serialize(&self, w: &mut crate::PacketWriter) -> anyhow::Result<()> {
        VarInt(self.len() as i32).serialize(w)?;
        for prop in self{
            prop.serialize(w)?;
        }
        Ok(())
    }
    // 我们信任提供的是空vec,要不然你调用这个干什么？？？
    fn deserialize(&mut self, r: &mut crate::PacketReader) -> anyhow::Result<()> {
        let mut len = VarInt(0);
        len.deserialize(r)?;
        for _ in 0..len.0{
            let mut value:T = Default::default();
            value.deserialize(r)?;
            self.push(value);
        }
        Ok(())
    }
}

impl<const N: usize>PacketCodec for [u8; N] where [u8; N]: Default {
    fn serialize(&self, w: &mut crate::PacketWriter) -> anyhow::Result<()> {
        for i in 0..N {
            self[i].serialize(w)?
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> anyhow::Result<()> {
        for i in 0..N {
            self[i].deserialize(r)?;
        }
        Ok(())
    }
}