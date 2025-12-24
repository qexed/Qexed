use crate::PacketCodec;
impl <T>PacketCodec for Option<T> where T: PacketCodec, {
    fn serialize(&self, w: &mut crate::PacketWriter) -> anyhow::Result<()> {
        if let Some(t) = self{
            true.serialize(w)?;
            t.serialize(w)?;
        } else{
            false.serialize(w)?;
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> anyhow::Result<()> {
        let mut is_have = false;
        is_have.deserialize(r)?;
        if is_have{
            // 这行怎么写？我想让里面的T调用 .deserialize(r)?;
            let mut v: T = Default::default();
            v.deserialize(r)?;
            *self = Some(v);
        }
        Ok(())
    }
}