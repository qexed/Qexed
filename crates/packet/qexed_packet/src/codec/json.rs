use crate::{PacketCodec};
impl PacketCodec for serde_json::Value {
    fn serialize(&self, w: &mut crate::PacketWriter) -> anyhow::Result<()> {
        return self.to_string().serialize(w)
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> anyhow::Result<()> {
        let mut data :String=Default::default();
        data.deserialize(r)?;
        *self = serde_json::json!(data);
        Ok(())
    }
}