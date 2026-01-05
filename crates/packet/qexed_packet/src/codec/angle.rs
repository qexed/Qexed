// 实现PacketCodec（根据你提供的代码修正）
use crate::{PacketCodec, net_types::Angle};
impl PacketCodec for Angle {
    fn serialize(&self, w: &mut crate::PacketWriter) -> anyhow::Result<()> {
        self.0.serialize(w)  // 直接序列化内部的u8值
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> anyhow::Result<()> {
        self.0.deserialize(r)?;
        Ok(())
    }
}
