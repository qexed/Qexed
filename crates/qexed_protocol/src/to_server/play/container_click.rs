use std::collections::HashMap;

use qexed_packet::{PacketCodec, net_types::VarInt};

use crate::types::Slot;

#[qexed_packet_macros::packet(id = 0x12)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ContainerClick {
    pub window_id: u8,
    pub state_id: VarInt,
    pub slot: i16,
    pub button: i8,
    pub click_type: VarInt,
    pub changed_slots: ChangedSlots,
    pub carried_item: Slot,
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct ChangedSlots(pub HashMap<i16, Slot>);

impl PacketCodec for ChangedSlots {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
        VarInt(self.0.len() as i32).serialize(w)?;
        let mut entries = self.0.iter().collect::<Vec<_>>();
        entries.sort_by_key(|(slot, _)| **slot);
        for (slot, item) in entries {
            slot.serialize(w)?;
            item.serialize(w)?;
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
        let mut len = VarInt::default();
        len.deserialize(r)?;
        if len.0 < 0 {
            anyhow::bail!("negative changed slot count: {}", len.0);
        }

        self.0.clear();
        for _ in 0..len.0 {
            let mut slot = i16::default();
            slot.deserialize(r)?;
            let mut item = Slot::default();
            item.deserialize(r)?;
            self.0.insert(slot, item);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use qexed_packet::Packet;

    use super::{ChangedSlots, ContainerClick};

    #[test]
    fn container_click_round_trips_changed_slots() {
        let mut changed = HashMap::new();
        changed.insert(13, crate::types::Slot::default());
        let packet = ContainerClick {
            window_id: 1,
            state_id: qexed_packet::net_types::VarInt(7),
            slot: 13,
            button: 0,
            click_type: qexed_packet::net_types::VarInt(0),
            changed_slots: ChangedSlots(changed),
            carried_item: crate::types::Slot::default(),
        };
        let mut buf = bytes::BytesMut::new();
        let mut writer = qexed_packet::PacketWriter::new(&mut buf);
        packet.serialize(&mut writer).unwrap();

        let mut bytes = buf.freeze();
        let mut reader = qexed_packet::PacketReader::new(&mut bytes);
        let mut decoded = ContainerClick::default();
        decoded.deserialize(&mut reader).unwrap();

        assert_eq!(decoded, packet);
    }
}
