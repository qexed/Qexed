use qexed_packet::net_types::{Position, VarInt};
use qexed_protocol::{
    to_client::play::{
        block_update::BlockUpdate,
        set_equipment::{Equipment, SetEquipment},
        set_player_inventory::SetPlayerInventory,
    },
    types::Slot,
};

use crate::player_data::{StoredEquipment, StoredInventory, StoredSlot};

const STONE_ITEM_ID: i32 = 1;
pub const STONE_BLOCK_STATE_ID: i32 = 7760;
const HOTBAR_SIZE: usize = 9;

#[derive(Debug, Clone)]
pub enum InventorySlotChange {
    Hotbar { slot: usize, item: Slot },
    Equipment { slot: u8, item: Slot },
}

#[derive(Debug, Clone)]
pub struct PlayerInventory {
    hotbar: Vec<Slot>,
    equipment: Vec<Equipment>,
    selected: usize,
}

impl Default for PlayerInventory {
    fn default() -> Self {
        let mut hotbar = vec![empty_slot(); HOTBAR_SIZE];
        hotbar[0] = simple_item(STONE_ITEM_ID, 64);
        Self {
            hotbar,
            equipment: vec![
                Equipment::mainhand(simple_item(STONE_ITEM_ID, 64)),
                Equipment::offhand(empty_slot()),
                Equipment::feet(empty_slot()),
                Equipment::legs(empty_slot()),
                Equipment::chest(empty_slot()),
                Equipment::head(empty_slot()),
            ],
            selected: 0,
        }
    }
}

impl PlayerInventory {
    pub fn from_stored(stored: &StoredInventory) -> Self {
        let mut inventory = Self::default();
        for (index, slot) in stored.hotbar.iter().take(HOTBAR_SIZE).enumerate() {
            inventory.hotbar[index] = slot.into();
        }
        inventory.selected = stored.selected.min(HOTBAR_SIZE - 1);
        inventory.equipment = stored.equipment.iter().map(Equipment::from).collect();
        let held = inventory.held_item().clone();
        inventory.set_equipment_slot(Equipment::MAINHAND, held);
        inventory
    }

    pub fn to_stored(&self) -> StoredInventory {
        StoredInventory {
            selected: self.selected,
            hotbar: self.hotbar.iter().map(StoredSlot::from).collect(),
            equipment: self.equipment.iter().map(StoredEquipment::from).collect(),
        }
    }

    pub fn set_selected(&mut self, slot: i16) -> Option<Slot> {
        let slot = usize::try_from(slot).ok()?;
        if slot >= HOTBAR_SIZE {
            return None;
        }
        self.selected = slot;
        let held = self.held_item().clone();
        self.set_equipment_slot(Equipment::MAINHAND, held.clone());
        Some(held)
    }

    pub fn set_creative_slot(
        &mut self,
        container_slot: i16,
        item: Slot,
    ) -> Option<InventorySlotChange> {
        match inventory_slot_from_container(container_slot)? {
            InventorySlot::Hotbar(slot) => {
                self.hotbar[slot] = item.clone();
                if slot == self.selected {
                    self.set_equipment_slot(Equipment::MAINHAND, item.clone());
                }
                Some(InventorySlotChange::Hotbar { slot, item })
            }
            InventorySlot::Equipment(slot) => {
                self.set_equipment_slot(slot, item.clone());
                Some(InventorySlotChange::Equipment { slot, item })
            }
        }
    }

    pub fn pick_block(&mut self, item_id: i32) -> usize {
        self.hotbar[self.selected] = simple_item(item_id, 64);
        self.set_equipment_slot(Equipment::MAINHAND, self.hotbar[self.selected].clone());
        self.selected
    }

    pub fn held_item(&self) -> &Slot {
        &self.hotbar[self.selected]
    }

    pub fn selected_slot(&self) -> usize {
        self.selected
    }

    pub fn set_player_inventory_packets(&self) -> Vec<SetPlayerInventory> {
        self.hotbar
            .iter()
            .enumerate()
            .map(|(slot, contents)| SetPlayerInventory {
                slot: VarInt(slot as i32),
                contents: contents.clone(),
            })
            .collect()
    }

    pub fn visible_equipment(&self) -> Vec<Equipment> {
        self.equipment.clone()
    }

    fn set_equipment_slot(&mut self, slot: u8, item: Slot) {
        if let Some(equipment) = self
            .equipment
            .iter_mut()
            .find(|equipment| equipment.slot == slot)
        {
            equipment.item = item;
            return;
        }
        self.equipment.push(Equipment::new(slot, item));
    }
}

pub fn acknowledge_block_change(sequence: VarInt) -> BlockUpdateAck {
    BlockUpdateAck { sequence }
}

pub fn placed_block_state_for_item(item: &Slot) -> Option<i32> {
    match item.item_id.as_ref().map(|id| id.0) {
        Some(STONE_ITEM_ID) => Some(STONE_BLOCK_STATE_ID),
        _ => None,
    }
}

pub fn set_player_inventory_packet(slot: usize, contents: Slot) -> SetPlayerInventory {
    SetPlayerInventory {
        slot: VarInt(slot as i32),
        contents,
    }
}

pub fn equipment_packet(entity_id: i32, slots: Vec<Equipment>) -> SetEquipment {
    SetEquipment {
        entity_id: VarInt(entity_id),
        slots,
    }
}

pub fn block_update(position: Position, block_state: i32) -> BlockUpdate {
    BlockUpdate {
        location: position,
        block_state: VarInt(block_state),
    }
}

pub fn empty_slot() -> Slot {
    Slot {
        item_count: VarInt(0),
        ..Slot::default()
    }
}

pub fn simple_item(item_id: i32, count: i32) -> Slot {
    Slot {
        item_count: VarInt(count),
        item_id: Some(VarInt(item_id)),
        number_of_components_to_add: Some(VarInt(0)),
        number_of_components_to_remove: Some(VarInt(0)),
        components_to_add: None,
        components_to_remove: None,
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum InventorySlot {
    Hotbar(usize),
    Equipment(u8),
}

fn inventory_slot_from_container(slot: i16) -> Option<InventorySlot> {
    match slot {
        5 => Some(InventorySlot::Equipment(Equipment::HEAD)),
        6 => Some(InventorySlot::Equipment(Equipment::CHEST)),
        7 => Some(InventorySlot::Equipment(Equipment::LEGS)),
        8 => Some(InventorySlot::Equipment(Equipment::FEET)),
        36..=44 => usize::try_from(slot - 36).ok().map(InventorySlot::Hotbar),
        45 => Some(InventorySlot::Equipment(Equipment::OFFHAND)),
        _ => None,
    }
}

pub fn placement_position(position: &Position, face: i32) -> Position {
    let mut target = position.clone();
    match face {
        0 => target.y -= 1,
        1 => target.y += 1,
        2 => target.z -= 1,
        3 => target.z += 1,
        4 => target.x -= 1,
        5 => target.x += 1,
        _ => {}
    }
    target
}

pub struct BlockUpdateAck {
    pub sequence: VarInt,
}

impl BlockUpdateAck {
    pub fn packet(self) -> qexed_protocol::to_client::play::block_changed_ack::BlockChangedAck {
        qexed_protocol::to_client::play::block_changed_ack::BlockChangedAck {
            sequence: self.sequence,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_player_inventory_hotbar_slots() {
        assert_eq!(
            inventory_slot_from_container(36),
            Some(InventorySlot::Hotbar(0))
        );
        assert_eq!(
            inventory_slot_from_container(44),
            Some(InventorySlot::Hotbar(8))
        );
        assert_eq!(
            inventory_slot_from_container(5),
            Some(InventorySlot::Equipment(Equipment::HEAD))
        );
        assert_eq!(
            inventory_slot_from_container(8),
            Some(InventorySlot::Equipment(Equipment::FEET))
        );
        assert_eq!(
            inventory_slot_from_container(45),
            Some(InventorySlot::Equipment(Equipment::OFFHAND))
        );
    }

    #[test]
    fn place_uses_clicked_face() {
        let placed = placement_position(&Position { x: 1, y: 2, z: 3 }, 1);
        assert_eq!(placed, Position { x: 1, y: 3, z: 3 });
    }
}
