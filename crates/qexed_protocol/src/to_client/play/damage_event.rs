use bytes::{Buf as _, BufMut as _};
use qexed_packet::{Packet, PacketCodec, PacketReader, PacketWriter, net_types::VarInt};

#[derive(Debug, Default, PartialEq, Clone)]
pub struct DamageEvent {
    pub entity_id: VarInt,
    // TODO: net.minecraft.core.Holder<DamageType> — 注册表 holder id，用 VarInt 占位
    pub source_type: VarInt,
    pub source_cause_id: VarInt,
    pub source_direct_id: VarInt,
    // TODO: Optional<Vec3> — 线上格式为布尔存在标记 + 3 个 double
    pub source_position: Option<Vec3d>,
}

impl Packet for DamageEvent {
    const ID: i32 = 0x18;

    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        self.entity_id.serialize(w)?;
        self.source_type.serialize(w)?;
        self.source_cause_id.serialize(w)?;
        self.source_direct_id.serialize(w)?;
        match &self.source_position {
            Some(position) => {
                true.serialize(w)?;
                position.serialize(w)
            }
            None => false.serialize(w),
        }
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        self.entity_id.deserialize(r)?;
        self.source_type.deserialize(r)?;
        self.source_cause_id.deserialize(r)?;
        self.source_direct_id.deserialize(r)?;
        let mut has_position = false;
        has_position.deserialize(r)?;
        self.source_position = if has_position {
            let mut position = Vec3d::default();
            position.deserialize(r)?;
            Some(position)
        } else {
            None
        };
        Ok(())
    }
}

/// net.minecraft.world.phys.Vec3（STREAM_CODEC：3 个 double）。
#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub struct Vec3d {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl PacketCodec for Vec3d {
    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        w.buf.put_f64(self.x);
        w.buf.put_f64(self.y);
        w.buf.put_f64(self.z);
        Ok(())
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        self.x = r.buf.get_f64();
        self.y = r.buf.get_f64();
        self.z = r.buf.get_f64();
        Ok(())
    }
}
