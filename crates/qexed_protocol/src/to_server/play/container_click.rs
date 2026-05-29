use std::collections::HashMap;

use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x12)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ContainerClick {
    pub window_id: VarInt,
    pub state_id: VarInt,
    pub slot: i16,
    pub button: i8,
    pub click_type: VarInt,
    pub changed_slots: ChangedSlots,
    pub carried_item: HashedSlot,
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct ChangedSlots(pub HashMap<i16, HashedSlot>);

impl PacketCodec for ChangedSlots {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
        write_bounded_len(self.0.len(), 128, "changed slots", w)?;
        let mut entries = self.0.iter().collect::<Vec<_>>();
        entries.sort_by_key(|(slot, _)| **slot);
        for (slot, item) in entries {
            slot.serialize(w)?;
            item.serialize(w)?;
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
        let len = read_bounded_len(128, "changed slots", r)?;
        self.0.clear();
        for _ in 0..len {
            let mut slot = i16::default();
            slot.deserialize(r)?;
            let mut item = HashedSlot::default();
            item.deserialize(r)?;
            self.0.insert(slot, item);
        }
        Ok(())
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct HashedSlot {
    pub item: Option<HashedItem>,
}

impl PacketCodec for HashedSlot {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
        self.item.serialize(w)
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
        self.item.deserialize(r)
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct HashedItem {
    pub item_id: VarInt,
    pub item_count: VarInt,
    pub components: HashedPatchMap,
}

impl PacketCodec for HashedItem {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
        self.item_id.serialize(w)?;
        self.item_count.serialize(w)?;
        self.components.serialize(w)
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
        self.item_id.deserialize(r)?;
        self.item_count.deserialize(r)?;
        self.components.deserialize(r)
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct HashedPatchMap {
    pub added_components: HashMap<i32, i32>,
    pub removed_components: Vec<VarInt>,
}

impl PacketCodec for HashedPatchMap {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
        write_bounded_len(self.added_components.len(), 256, "hashed added components", w)?;
        let mut added = self.added_components.iter().collect::<Vec<_>>();
        added.sort_by_key(|(component, _)| **component);
        for (component, hash) in added {
            VarInt(*component).serialize(w)?;
            hash.serialize(w)?;
        }

        write_bounded_len(
            self.removed_components.len(),
            256,
            "hashed removed components",
            w,
        )?;
        for component in &self.removed_components {
            component.serialize(w)?;
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
        let added_len = read_bounded_len(256, "hashed added components", r)?;
        self.added_components.clear();
        for _ in 0..added_len {
            let mut component = VarInt::default();
            component.deserialize(r)?;
            let mut hash = i32::default();
            hash.deserialize(r)?;
            self.added_components.insert(component.0, hash);
        }

        let removed_len = read_bounded_len(256, "hashed removed components", r)?;
        self.removed_components.clear();
        for _ in 0..removed_len {
            let mut component = VarInt::default();
            component.deserialize(r)?;
            self.removed_components.push(component);
        }
        Ok(())
    }
}

fn read_bounded_len(
    max: usize,
    name: &str,
    r: &mut qexed_packet::PacketReader,
) -> anyhow::Result<usize> {
    let mut len = VarInt::default();
    len.deserialize(r)?;
    if len.0 < 0 {
        anyhow::bail!("negative {name} length: {}", len.0);
    }
    let len = len.0 as usize;
    if len > max {
        anyhow::bail!("{name} length {len} exceeds max {max}");
    }
    Ok(len)
}

fn write_bounded_len(
    len: usize,
    max: usize,
    name: &str,
    w: &mut qexed_packet::PacketWriter,
) -> anyhow::Result<()> {
    if len > max {
        anyhow::bail!("{name} length {len} exceeds max {max}");
    }
    VarInt(len as i32).serialize(w)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use qexed_packet::Packet;

    use super::{ChangedSlots, ContainerClick, HashedItem, HashedPatchMap, HashedSlot};

    #[test]
    fn container_click_round_trips_changed_slots() {
        let mut changed = HashMap::new();
        changed.insert(13, HashedSlot::default());
        let packet = ContainerClick {
            window_id: qexed_packet::net_types::VarInt(1),
            state_id: qexed_packet::net_types::VarInt(7),
            slot: 13,
            button: 0,
            click_type: qexed_packet::net_types::VarInt(0),
            changed_slots: ChangedSlots(changed),
            carried_item: HashedSlot::default(),
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

    #[test]
    fn container_click_decodes_26_1_2_hashed_item_payload() {
        let packet = ContainerClick {
            window_id: qexed_packet::net_types::VarInt(2),
            state_id: qexed_packet::net_types::VarInt(0),
            slot: 13,
            button: 0,
            click_type: qexed_packet::net_types::VarInt(0),
            changed_slots: ChangedSlots(HashMap::from([(
                13,
                HashedSlot {
                    item: Some(HashedItem {
                        item_id: qexed_packet::net_types::VarInt(1029),
                        item_count: qexed_packet::net_types::VarInt(1),
                        components: HashedPatchMap {
                            added_components: HashMap::from([(9, 0x12345678)]),
                            removed_components: vec![qexed_packet::net_types::VarInt(11)],
                        },
                    }),
                },
            )])),
            carried_item: HashedSlot::default(),
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
