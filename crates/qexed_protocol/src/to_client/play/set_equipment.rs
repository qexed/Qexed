use qexed_packet::{Packet, PacketCodec, PacketReader, PacketWriter, net_types::VarInt};

use crate::types::Slot;

#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetEquipment {
    pub entity_id: VarInt,
    pub slots: Vec<Equipment>,
}

impl Packet for SetEquipment {
    const ID: i32 = 0x66;

    fn serialize(&self, w: &mut PacketWriter) -> anyhow::Result<()> {
        self.entity_id.serialize(w)?;
        let last = self.slots.len().saturating_sub(1);
        for (index, equipment) in self.slots.iter().enumerate() {
            let slot = if index == last {
                equipment.slot
            } else {
                equipment.slot | 0x80
            };
            slot.serialize(w)?;
            equipment.item.serialize(w)?;
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> anyhow::Result<()> {
        self.entity_id.deserialize(r)?;
        self.slots.clear();
        loop {
            let mut slot = 0_u8;
            slot.deserialize(r)?;
            let has_next = slot & 0x80 != 0;
            let mut item = Slot::default();
            item.deserialize(r)?;
            self.slots.push(Equipment {
                slot: slot & 0x7f,
                item,
            });
            if !has_next {
                break;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Equipment {
    pub slot: u8,
    pub item: Slot,
}

impl Equipment {
    pub const MAINHAND: u8 = 0;
    pub const OFFHAND: u8 = 1;
    pub const FEET: u8 = 2;
    pub const LEGS: u8 = 3;
    pub const CHEST: u8 = 4;
    pub const HEAD: u8 = 5;
    pub const BODY: u8 = 6;
    pub const SADDLE: u8 = 7;

    pub fn new(slot: u8, item: Slot) -> Self {
        Self { slot, item }
    }

    pub fn mainhand(item: Slot) -> Self {
        Self::new(Self::MAINHAND, item)
    }

    pub fn offhand(item: Slot) -> Self {
        Self::new(Self::OFFHAND, item)
    }

    pub fn feet(item: Slot) -> Self {
        Self::new(Self::FEET, item)
    }

    pub fn legs(item: Slot) -> Self {
        Self::new(Self::LEGS, item)
    }

    pub fn chest(item: Slot) -> Self {
        Self::new(Self::CHEST, item)
    }

    pub fn head(item: Slot) -> Self {
        Self::new(Self::HEAD, item)
    }
}
