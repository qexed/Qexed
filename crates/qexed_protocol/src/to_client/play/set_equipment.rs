use bytes::BufMut as _;
use qexed_packet::{
    net_types::VarInt,
    PacketCodec, PacketReader, PacketWriter,
};
use crate::types::Slot;

/// 26.3 SetEquipment：entity_id + 逐项 (slot 字节 | Slot)。
/// slot 字节低 7 位槽位号；0x80 继续位（非末项置位，末项清零）。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetEquipment {
    pub entity: VarInt,
    pub slots: Vec<EquipmentEntry>,
}

/// 单件装备：槽位（0=主手 1=副手 2..5=盔甲 6=鞍）与物品。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct EquipmentEntry {
    pub slot: i32,
    pub item: Slot,
}

impl PacketCodec for EquipmentEntry {
    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        self.item.serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        self.item.deserialize(r)
    }
}

impl qexed_packet::Packet for SetEquipment {
    const ID: i32 = 0x68;

    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        self.entity.serialize(w)?;
        let last = self.slots.len().saturating_sub(1);
        for (index, entry) in self.slots.iter().enumerate() {
            let mut b = (entry.slot & 0x7f) as u8;
            if index != last {
                b |= 0x80;
            }
            w.buf.put_u8(b);
            entry.item.serialize(w)?;
        }
        if self.slots.is_empty() {
            w.buf.put_u8(0);
            Slot::default().serialize(w)?;
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        self.entity.deserialize(r)?;
        self.slots.clear();
        loop {
            use bytes::Buf as _;
            let b = r.buf.get_u8();
            let slot = (b & 0x7f) as i32;
            let mut item = Slot::default();
            item.deserialize(r)?;
            self.slots.push(EquipmentEntry { slot, item });
            if b & 0x80 == 0 {
                break;
            }
        }
        Ok(())
    }
}