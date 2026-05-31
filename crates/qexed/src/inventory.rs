use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};

use anyhow::{Context, Result};
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
const STONE_BLOCK_STATE_ID: i32 = 1;
const AIR_BLOCK_STATE_ID: i32 = 0;
const HOTBAR_SIZE: usize = 9;
const MAIN_INVENTORY_SIZE: usize = 27;
const DEFAULT_STACK_LIMIT: i32 = 64;

#[derive(Debug, Clone, Copy)]
pub struct PlacementContext {
    pub face: i32,
    pub cursor_y: f32,
    pub player_yaw: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockCollisionShape {
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
    pub min_z: f64,
    pub max_z: f64,
}

impl BlockCollisionShape {
    pub const FULL_BLOCK: Self = Self {
        min_x: 0.0,
        max_x: 1.0,
        min_y: 0.0,
        max_y: 1.0,
        min_z: 0.0,
        max_z: 1.0,
    };
}

#[derive(Debug, Clone)]
pub enum InventorySlotChange {
    Hotbar { slot: usize, item: Slot },
    Main { slot: usize, item: Slot },
    Equipment { slot: u8, item: Slot },
}

#[derive(Debug, Clone)]
pub struct PlayerInventory {
    main: Vec<Slot>,
    hotbar: Vec<Slot>,
    equipment: Vec<Equipment>,
    selected: usize,
}

impl Default for PlayerInventory {
    fn default() -> Self {
        Self::empty()
    }
}

impl PlayerInventory {
    pub fn empty() -> Self {
        Self {
            main: vec![empty_slot(); MAIN_INVENTORY_SIZE],
            hotbar: vec![empty_slot(); HOTBAR_SIZE],
            equipment: vec![
                Equipment::mainhand(empty_slot()),
                Equipment::offhand(empty_slot()),
                Equipment::feet(empty_slot()),
                Equipment::legs(empty_slot()),
                Equipment::chest(empty_slot()),
                Equipment::head(empty_slot()),
            ],
            selected: 0,
        }
    }

    pub fn from_stored(stored: &StoredInventory) -> Self {
        let mut inventory = Self::empty();
        for (index, slot) in stored.main.iter().take(MAIN_INVENTORY_SIZE).enumerate() {
            inventory.main[index] = slot.into();
        }
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
            main: self.main.iter().map(StoredSlot::from).collect(),
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
            InventorySlot::Main(slot) => {
                self.main[slot] = item.clone();
                Some(InventorySlotChange::Main { slot, item })
            }
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

    pub fn consume_selected_one(&mut self) -> Option<(usize, Slot)> {
        let slot = self.selected;
        let count = self.hotbar[slot].item_count.0;
        if count <= 0 {
            return None;
        }

        if count == 1 {
            self.hotbar[slot] = empty_slot();
        } else {
            self.hotbar[slot].item_count = VarInt(count - 1);
        }

        let held = self.hotbar[slot].clone();
        self.set_equipment_slot(Equipment::MAINHAND, held.clone());
        Some((slot, held))
    }

    pub fn drop_selected(&mut self, whole_stack: bool) -> Option<(usize, Slot)> {
        let slot = self.selected;
        let count = self.hotbar[slot].item_count.0;
        if count <= 0 {
            return None;
        }

        let mut dropped = self.hotbar[slot].clone();
        if whole_stack || count == 1 {
            self.hotbar[slot] = empty_slot();
        } else {
            dropped.item_count = VarInt(1);
            self.hotbar[slot].item_count = VarInt(count - 1);
        }

        let held = self.hotbar[slot].clone();
        self.set_equipment_slot(Equipment::MAINHAND, held);
        Some((slot, dropped))
    }

    pub fn can_accept_item_stack(&self, item: &Slot) -> bool {
        item_stack_capacity(&self.hotbar, item) + item_stack_capacity(&self.main, item)
            >= item.item_count.0.max(0)
    }

    pub fn add_item_stack(&mut self, item: &Slot) -> Option<Vec<InventorySlotChange>> {
        let mut remaining = item.item_count.0;
        if remaining <= 0 {
            return Some(Vec::new());
        }
        if !self.can_accept_item_stack(item) {
            return None;
        }

        let mut changes = Vec::new();
        for slot in 0..self.hotbar.len() {
            if remaining <= 0 {
                break;
            }
            if !same_stack_kind(&self.hotbar[slot], item) {
                continue;
            }
            let available = DEFAULT_STACK_LIMIT - self.hotbar[slot].item_count.0;
            if available <= 0 {
                continue;
            }
            let added = remaining.min(available);
            self.hotbar[slot].item_count = VarInt(self.hotbar[slot].item_count.0 + added);
            remaining -= added;
            changes.push(InventorySlotChange::Hotbar {
                slot,
                item: self.hotbar[slot].clone(),
            });
        }

        for slot in 0..self.main.len() {
            if remaining <= 0 {
                break;
            }
            if !same_stack_kind(&self.main[slot], item) {
                continue;
            }
            let available = DEFAULT_STACK_LIMIT - self.main[slot].item_count.0;
            if available <= 0 {
                continue;
            }
            let added = remaining.min(available);
            self.main[slot].item_count = VarInt(self.main[slot].item_count.0 + added);
            remaining -= added;
            changes.push(InventorySlotChange::Main {
                slot,
                item: self.main[slot].clone(),
            });
        }

        for slot in 0..self.hotbar.len() {
            if remaining <= 0 {
                break;
            }
            if self.hotbar[slot].item_count.0 != 0 {
                continue;
            }
            let added = remaining.min(DEFAULT_STACK_LIMIT);
            let mut stack = item.clone();
            stack.item_count = VarInt(added);
            self.hotbar[slot] = stack;
            remaining -= added;
            changes.push(InventorySlotChange::Hotbar {
                slot,
                item: self.hotbar[slot].clone(),
            });
        }

        for slot in 0..self.main.len() {
            if remaining <= 0 {
                break;
            }
            if self.main[slot].item_count.0 != 0 {
                continue;
            }
            let added = remaining.min(DEFAULT_STACK_LIMIT);
            let mut stack = item.clone();
            stack.item_count = VarInt(added);
            self.main[slot] = stack;
            remaining -= added;
            changes.push(InventorySlotChange::Main {
                slot,
                item: self.main[slot].clone(),
            });
        }

        let held = self.hotbar[self.selected].clone();
        self.set_equipment_slot(Equipment::MAINHAND, held);
        Some(changes)
    }

    pub fn held_item(&self) -> &Slot {
        &self.hotbar[self.selected]
    }

    pub fn selected_slot(&self) -> usize {
        self.selected
    }

    pub fn hotbar_item(&self, slot: usize) -> Option<&Slot> {
        self.hotbar.get(slot)
    }

    pub fn set_hotbar_slot(&mut self, slot: usize, item: Slot) -> Option<InventorySlotChange> {
        if slot >= self.hotbar.len() {
            return None;
        }
        self.hotbar[slot] = item.clone();
        if slot == self.selected {
            self.set_equipment_slot(Equipment::MAINHAND, item.clone());
        }
        Some(InventorySlotChange::Hotbar { slot, item })
    }

    pub fn set_player_inventory_packets(&self) -> Vec<SetPlayerInventory> {
        self.hotbar
            .iter()
            .enumerate()
            .map(|(slot, contents)| SetPlayerInventory {
                slot: VarInt(slot as i32),
                contents: contents.clone(),
            })
            .chain(
                self.main
                    .iter()
                    .enumerate()
                    .map(|(slot, contents)| SetPlayerInventory {
                        slot: VarInt((HOTBAR_SIZE + slot) as i32),
                        contents: contents.clone(),
                    }),
            )
            .collect()
    }

    pub fn visible_equipment(&self) -> Vec<Equipment> {
        self.equipment.clone()
    }

    pub fn drain_droppable_items(&mut self) -> (Vec<Slot>, Vec<InventorySlotChange>) {
        let mut drops = Vec::new();
        let mut changes = Vec::new();

        for (slot, item) in self.hotbar.iter_mut().enumerate() {
            if item.item_count.0 > 0 {
                drops.push(item.clone());
                *item = empty_slot();
                changes.push(InventorySlotChange::Hotbar {
                    slot,
                    item: item.clone(),
                });
            }
        }

        for (slot, item) in self.main.iter_mut().enumerate() {
            if item.item_count.0 > 0 {
                drops.push(item.clone());
                *item = empty_slot();
                changes.push(InventorySlotChange::Main {
                    slot,
                    item: item.clone(),
                });
            }
        }

        for equipment in &mut self.equipment {
            if equipment.slot == Equipment::MAINHAND {
                continue;
            }
            if equipment.item.item_count.0 > 0 {
                drops.push(equipment.item.clone());
                equipment.item = empty_slot();
                changes.push(InventorySlotChange::Equipment {
                    slot: equipment.slot,
                    item: equipment.item.clone(),
                });
            }
        }

        self.set_equipment_slot(Equipment::MAINHAND, self.hotbar[self.selected].clone());
        if !changes
            .iter()
            .any(|change| matches!(change, InventorySlotChange::Equipment { slot, .. } if *slot == Equipment::MAINHAND))
        {
            changes.push(InventorySlotChange::Equipment {
                slot: Equipment::MAINHAND,
                item: self.hotbar[self.selected].clone(),
            });
        }

        (drops, changes)
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
    if item.item_count.0 <= 0 {
        return None;
    }

    let item_id = item.item_id.as_ref()?.0;
    block_item_registry()
        .block_state_by_item_id
        .get(&item_id)
        .copied()
}

pub fn picked_item_for_block_state(block_state: i32) -> Option<i32> {
    block_item_registry()
        .item_id_by_block_state
        .get(&block_state)
        .copied()
}

pub fn block_name_for_state(block_state: i32) -> Option<String> {
    block_item_registry()
        .block_name_by_state
        .get(&block_state)
        .cloned()
}

pub fn item_id_name_map() -> HashMap<i32, String> {
    block_item_registry().item_name_by_id.clone()
}

pub fn item_id_for_name(name: &str) -> Option<i32> {
    block_item_registry().item_id_by_name.get(name).copied()
}

pub fn is_air_block_state(block_state: i32) -> bool {
    block_item_registry()
        .air_block_states
        .contains(&block_state)
}

pub fn can_replace_block_state(block_state: i32) -> bool {
    block_item_registry()
        .replaceable_block_states
        .contains(&block_state)
}

pub fn block_has_collision(block_state: i32) -> bool {
    let registry = block_item_registry();
    if registry.air_block_states.contains(&block_state) {
        return false;
    }
    if registry.known_block_states.contains(&block_state) {
        return registry.collision_block_states.contains(&block_state);
    }
    true
}

pub fn block_collision_shape(block_state: i32) -> Option<BlockCollisionShape> {
    if !block_has_collision(block_state) {
        return None;
    }
    let registry = block_item_registry();
    let Some(name) = registry.block_name_by_state.get(&block_state) else {
        return Some(BlockCollisionShape::FULL_BLOCK);
    };
    let properties = registry.properties_by_block_state.get(&block_state);
    Some(collision_shape_for_block(name, properties))
}

pub fn upper_half_block_state(lower_state: i32) -> Option<i32> {
    block_item_registry()
        .upper_half_by_lower_state
        .get(&lower_state)
        .copied()
}

pub fn lower_half_block_state(upper_state: i32) -> Option<i32> {
    block_item_registry()
        .lower_half_by_upper_state
        .get(&upper_state)
        .copied()
}

pub fn block_state_for_placement(default_state: i32, context: PlacementContext) -> i32 {
    let registry = block_item_registry();
    let Some(candidate_ids) = registry.states_by_block_state.get(&default_state) else {
        return default_state;
    };
    let Some(default_properties) = registry.properties_by_block_state.get(&default_state) else {
        return default_state;
    };

    let mut desired = default_properties.clone();
    set_property_if_present(&mut desired, "waterlogged", "false");
    set_property_if_present(&mut desired, "powered", "false");
    set_property_if_present(&mut desired, "lit", "false");
    set_property_if_present(&mut desired, "shape", "straight");

    if desired.contains_key("axis") {
        desired.insert("axis".to_string(), axis_for_face(context.face).to_string());
    }

    if desired.contains_key("type") {
        desired.insert(
            "type".to_string(),
            slab_type_for_placement(context.face, context.cursor_y).to_string(),
        );
    }

    if desired.contains_key("half") {
        let half = if desired
            .get("half")
            .is_some_and(|value| value == "upper" || value == "lower")
        {
            "lower"
        } else {
            half_for_placement(context.face, context.cursor_y)
        };
        desired.insert("half".to_string(), half.to_string());
    }

    if desired.contains_key("face") {
        desired.insert(
            "face".to_string(),
            attach_face_for_clicked_face(context.face).to_string(),
        );
    }

    if desired.contains_key("facing") {
        desired.insert(
            "facing".to_string(),
            facing_for_placement(context.face, context.player_yaw).to_string(),
        );
    }

    find_state_with_properties(candidate_ids, &registry.properties_by_block_state, &desired)
        .unwrap_or(default_state)
}

pub fn set_player_inventory_packet(slot: usize, contents: Slot) -> SetPlayerInventory {
    SetPlayerInventory {
        slot: VarInt(slot as i32),
        contents,
    }
}

pub fn set_player_main_inventory_packet(slot: usize, contents: Slot) -> SetPlayerInventory {
    SetPlayerInventory {
        slot: VarInt((HOTBAR_SIZE + slot) as i32),
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

pub fn air_block_state() -> i32 {
    AIR_BLOCK_STATE_ID
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
    Main(usize),
    Hotbar(usize),
    Equipment(u8),
}

fn inventory_slot_from_container(slot: i16) -> Option<InventorySlot> {
    match slot {
        5 => Some(InventorySlot::Equipment(Equipment::HEAD)),
        6 => Some(InventorySlot::Equipment(Equipment::CHEST)),
        7 => Some(InventorySlot::Equipment(Equipment::LEGS)),
        8 => Some(InventorySlot::Equipment(Equipment::FEET)),
        9..=35 => usize::try_from(slot - 9).ok().map(InventorySlot::Main),
        36..=44 => usize::try_from(slot - 36).ok().map(InventorySlot::Hotbar),
        45 => Some(InventorySlot::Equipment(Equipment::OFFHAND)),
        _ => None,
    }
}

fn item_stack_capacity(hotbar: &[Slot], item: &Slot) -> i32 {
    if item.item_count.0 <= 0 || item.item_id.is_none() {
        return 0;
    }

    hotbar
        .iter()
        .map(|existing| {
            if existing.item_count.0 == 0 {
                DEFAULT_STACK_LIMIT
            } else if same_stack_kind(existing, item) {
                (DEFAULT_STACK_LIMIT - existing.item_count.0).max(0)
            } else {
                0
            }
        })
        .sum()
}

fn same_stack_kind(left: &Slot, right: &Slot) -> bool {
    left.item_count.0 > 0
        && right.item_count.0 > 0
        && left.item_id == right.item_id
        && left.number_of_components_to_add == right.number_of_components_to_add
        && left.number_of_components_to_remove == right.number_of_components_to_remove
        && left.components_to_add == right.components_to_add
        && left.components_to_remove == right.components_to_remove
}

fn block_item_registry() -> &'static BlockItemRegistry {
    static REGISTRY: OnceLock<BlockItemRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        load_block_item_registry().unwrap_or_else(|err| {
            log::warn!("failed to load block item registry mapping: {err:#}");
            BlockItemRegistry::fallback()
        })
    })
}

fn load_block_item_registry() -> Result<BlockItemRegistry> {
    let item_ids = crate::registry_sync::load_registry_id_map("minecraft:item")?;
    let item_name_by_id = item_ids
        .iter()
        .map(|(name, id)| (*id, name.clone()))
        .collect::<HashMap<_, _>>();
    let block_states = load_block_state_metadata()?;
    let mut block_state_by_item_id = HashMap::new();
    let mut item_id_by_block_state = HashMap::new();
    let mut states_by_block_state = HashMap::new();

    for (block_name, block_state) in &block_states.default_block_states {
        let Some(item_id) = item_ids.get(block_name).copied() else {
            continue;
        };
        block_state_by_item_id.insert(item_id, *block_state);
        if let Some(states) = block_states.states_by_block_name.get(block_name) {
            for state in states {
                states_by_block_state.insert(*state, states.clone());
            }
            for state in states {
                item_id_by_block_state.entry(*state).or_insert(item_id);
            }
        } else {
            item_id_by_block_state
                .entry(*block_state)
                .or_insert(item_id);
        }
    }

    if block_state_by_item_id.is_empty() {
        anyhow::bail!("block item registry mapping is empty");
    }

    Ok(BlockItemRegistry {
        block_state_by_item_id,
        item_id_by_block_state,
        air_block_states: block_states.air_block_states,
        replaceable_block_states: block_states.replaceable_block_states,
        collision_block_states: block_states.collision_block_states,
        known_block_states: block_states.known_block_states,
        upper_half_by_lower_state: block_states.upper_half_by_lower_state,
        lower_half_by_upper_state: block_states.lower_half_by_upper_state,
        properties_by_block_state: block_states.properties_by_block_state,
        block_name_by_state: block_states.block_name_by_state,
        item_id_by_name: item_ids,
        item_name_by_id,
        states_by_block_state,
    })
}

fn load_block_state_metadata() -> Result<BlockStateMetadata> {
    let value = crate::registry_sync::load_blocks_report()?;
    let blocks = value
        .as_object()
        .with_context(|| "block report root is not object".to_string())?;
    let mut metadata = BlockStateMetadata::default();

    for (name, block) in blocks {
        let definition_type = block
            .get("definition")
            .and_then(|definition| definition.get("type"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let states = block
            .get("states")
            .and_then(serde_json::Value::as_array)
            .with_context(|| format!("block has no states: {name}"))?;
        let Some(default_state) = default_block_state_value(block) else {
            continue;
        };
        let Some(default_id) = default_state
            .get("id")
            .and_then(serde_json::Value::as_i64)
            .and_then(|id| i32::try_from(id).ok())
        else {
            continue;
        };
        metadata
            .default_block_states
            .insert(name.clone(), default_id);

        let mut lower_half_states = HashMap::new();
        let mut upper_half_states = HashMap::new();
        for state in states {
            let Some(id) = state
                .get("id")
                .and_then(serde_json::Value::as_i64)
                .and_then(|id| i32::try_from(id).ok())
            else {
                continue;
            };
            metadata.known_block_states.insert(id);
            metadata.block_name_by_state.insert(id, name.clone());
            metadata
                .properties_by_block_state
                .insert(id, block_state_properties(state));
            metadata
                .states_by_block_name
                .entry(name.clone())
                .or_default()
                .push(id);

            if definition_type == "minecraft:air" {
                metadata.air_block_states.insert(id);
            }
            if block_state_is_replaceable(definition_type, state) {
                metadata.replaceable_block_states.insert(id);
            }
            if block_state_has_collision(definition_type) {
                metadata.collision_block_states.insert(id);
            }

            match state_property_value(state, "half") {
                Some("lower") => {
                    lower_half_states.insert(state_properties_signature_without_half(state), id);
                }
                Some("upper") => {
                    upper_half_states.insert(state_properties_signature_without_half(state), id);
                }
                _ => {}
            }
        }

        for (signature, lower) in lower_half_states {
            let Some(upper) = upper_half_states.get(&signature).copied() else {
                continue;
            };
            metadata.upper_half_by_lower_state.insert(lower, upper);
            metadata.lower_half_by_upper_state.insert(upper, lower);
        }
    }

    Ok(metadata)
}

fn default_block_state_value(block: &serde_json::Value) -> Option<&serde_json::Value> {
    let states = block.get("states")?.as_array()?;
    states
        .iter()
        .find(|state| {
            state
                .get("default")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        })
        .or_else(|| states.first())
}

#[derive(Default)]
struct BlockStateMetadata {
    default_block_states: HashMap<String, i32>,
    states_by_block_name: HashMap<String, Vec<i32>>,
    air_block_states: HashSet<i32>,
    replaceable_block_states: HashSet<i32>,
    collision_block_states: HashSet<i32>,
    known_block_states: HashSet<i32>,
    upper_half_by_lower_state: HashMap<i32, i32>,
    lower_half_by_upper_state: HashMap<i32, i32>,
    properties_by_block_state: HashMap<i32, HashMap<String, String>>,
    block_name_by_state: HashMap<i32, String>,
}

struct BlockItemRegistry {
    block_state_by_item_id: HashMap<i32, i32>,
    item_id_by_block_state: HashMap<i32, i32>,
    air_block_states: HashSet<i32>,
    replaceable_block_states: HashSet<i32>,
    collision_block_states: HashSet<i32>,
    known_block_states: HashSet<i32>,
    upper_half_by_lower_state: HashMap<i32, i32>,
    lower_half_by_upper_state: HashMap<i32, i32>,
    properties_by_block_state: HashMap<i32, HashMap<String, String>>,
    block_name_by_state: HashMap<i32, String>,
    item_id_by_name: HashMap<String, i32>,
    item_name_by_id: HashMap<i32, String>,
    states_by_block_state: HashMap<i32, Vec<i32>>,
}

impl BlockItemRegistry {
    fn fallback() -> Self {
        Self {
            block_state_by_item_id: HashMap::from([(STONE_ITEM_ID, STONE_BLOCK_STATE_ID)]),
            item_id_by_block_state: HashMap::from([(STONE_BLOCK_STATE_ID, STONE_ITEM_ID)]),
            air_block_states: HashSet::from([AIR_BLOCK_STATE_ID]),
            replaceable_block_states: HashSet::from([AIR_BLOCK_STATE_ID]),
            collision_block_states: HashSet::from([STONE_BLOCK_STATE_ID]),
            known_block_states: HashSet::from([AIR_BLOCK_STATE_ID, STONE_BLOCK_STATE_ID]),
            upper_half_by_lower_state: HashMap::new(),
            lower_half_by_upper_state: HashMap::new(),
            properties_by_block_state: HashMap::new(),
            block_name_by_state: HashMap::from([(
                STONE_BLOCK_STATE_ID,
                "minecraft:stone".to_string(),
            )]),
            item_id_by_name: HashMap::from([("minecraft:stone".to_string(), STONE_ITEM_ID)]),
            item_name_by_id: HashMap::from([(STONE_ITEM_ID, "minecraft:stone".to_string())]),
            states_by_block_state: HashMap::new(),
        }
    }
}

fn block_state_properties(state: &serde_json::Value) -> HashMap<String, String> {
    let Some(properties) = state
        .get("properties")
        .and_then(serde_json::Value::as_object)
    else {
        return HashMap::new();
    };

    properties
        .iter()
        .filter_map(|(key, value)| value.as_str().map(|value| (key.clone(), value.to_string())))
        .collect()
}

fn state_property_value<'a>(state: &'a serde_json::Value, name: &str) -> Option<&'a str> {
    state
        .get("properties")
        .and_then(|properties| properties.get(name))
        .and_then(serde_json::Value::as_str)
}

fn state_properties_signature_without_half(state: &serde_json::Value) -> String {
    let Some(properties) = state
        .get("properties")
        .and_then(serde_json::Value::as_object)
    else {
        return String::new();
    };

    let mut entries = properties
        .iter()
        .filter_map(|(key, value)| {
            if key == "half" {
                return None;
            }
            value.as_str().map(|value| format!("{key}={value}"))
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries.join(";")
}

fn block_state_is_replaceable(definition_type: &str, state: &serde_json::Value) -> bool {
    match definition_type {
        "minecraft:air"
        | "minecraft:liquid"
        | "minecraft:fire"
        | "minecraft:tall_grass"
        | "minecraft:dry_vegetation"
        | "minecraft:flower"
        | "minecraft:tall_flower"
        | "minecraft:pink_petals"
        | "minecraft:wildflowers"
        | "minecraft:leaf_litter"
        | "minecraft:vine"
        | "minecraft:cave_vines"
        | "minecraft:twisting_vines"
        | "minecraft:weeping_vines"
        | "minecraft:kelp"
        | "minecraft:seagrass" => true,
        "minecraft:snow_layer" => state
            .get("properties")
            .and_then(|properties| properties.get("layers"))
            .and_then(serde_json::Value::as_str)
            .is_none_or(|layers| layers == "1"),
        _ => false,
    }
}

fn block_state_has_collision(definition_type: &str) -> bool {
    !matches!(
        definition_type,
        "minecraft:air"
            | "minecraft:liquid"
            | "minecraft:fire"
            | "minecraft:tall_grass"
            | "minecraft:dry_vegetation"
            | "minecraft:flower"
            | "minecraft:tall_flower"
            | "minecraft:pink_petals"
            | "minecraft:wildflowers"
            | "minecraft:leaf_litter"
            | "minecraft:vine"
            | "minecraft:cave_vines"
            | "minecraft:twisting_vines"
            | "minecraft:weeping_vines"
            | "minecraft:kelp"
            | "minecraft:seagrass"
    )
}

fn collision_shape_for_block(
    name: &str,
    properties: Option<&HashMap<String, String>>,
) -> BlockCollisionShape {
    if name.ends_with("_carpet") || name == "minecraft:carpet" {
        return BlockCollisionShape {
            max_y: 1.0 / 16.0,
            ..BlockCollisionShape::FULL_BLOCK
        };
    }
    if name == "minecraft:snow" || name == "minecraft:snow_layer" {
        let layers = properties
            .and_then(|properties| properties.get("layers"))
            .and_then(|layers| layers.parse::<u8>().ok())
            .unwrap_or(1)
            .clamp(1, 8);
        return BlockCollisionShape {
            max_y: f64::from(layers) / 8.0,
            ..BlockCollisionShape::FULL_BLOCK
        };
    }
    if name.ends_with("_slab") {
        return match properties
            .and_then(|properties| properties.get("type"))
            .map(String::as_str)
        {
            Some("top") => BlockCollisionShape {
                min_y: 0.5,
                ..BlockCollisionShape::FULL_BLOCK
            },
            Some("double") => BlockCollisionShape::FULL_BLOCK,
            _ => BlockCollisionShape {
                max_y: 0.5,
                ..BlockCollisionShape::FULL_BLOCK
            },
        };
    }
    if name.ends_with("_trapdoor") {
        return match properties
            .and_then(|properties| properties.get("open"))
            .map(String::as_str)
        {
            Some("true") => BlockCollisionShape::FULL_BLOCK,
            _ => match properties
                .and_then(|properties| properties.get("half"))
                .map(String::as_str)
            {
                Some("top") => BlockCollisionShape {
                    min_y: 13.0 / 16.0,
                    ..BlockCollisionShape::FULL_BLOCK
                },
                _ => BlockCollisionShape {
                    max_y: 3.0 / 16.0,
                    ..BlockCollisionShape::FULL_BLOCK
                },
            },
        };
    }
    BlockCollisionShape::FULL_BLOCK
}

fn find_state_with_properties(
    candidate_ids: &[i32],
    properties_by_state: &HashMap<i32, HashMap<String, String>>,
    desired: &HashMap<String, String>,
) -> Option<i32> {
    candidate_ids
        .iter()
        .copied()
        .find(|id| properties_by_state.get(id) == Some(desired))
}

fn set_property_if_present(properties: &mut HashMap<String, String>, key: &str, value: &str) {
    if properties.contains_key(key) {
        properties.insert(key.to_string(), value.to_string());
    }
}

fn axis_for_face(face: i32) -> &'static str {
    match face {
        4 | 5 => "x",
        2 | 3 => "z",
        _ => "y",
    }
}

fn slab_type_for_placement(face: i32, cursor_y: f32) -> &'static str {
    if face == 0 || (face != 1 && cursor_y > 0.5) {
        "top"
    } else {
        "bottom"
    }
}

fn half_for_placement(face: i32, cursor_y: f32) -> &'static str {
    if face == 0 || (face != 1 && cursor_y > 0.5) {
        "top"
    } else {
        "bottom"
    }
}

fn attach_face_for_clicked_face(face: i32) -> &'static str {
    match face {
        0 => "ceiling",
        1 => "floor",
        _ => "wall",
    }
}

fn facing_for_placement(face: i32, player_yaw: f32) -> &'static str {
    match face {
        2 => "south",
        3 => "north",
        4 => "east",
        5 => "west",
        _ => horizontal_facing_from_yaw(player_yaw),
    }
}

fn horizontal_facing_from_yaw(yaw: f32) -> &'static str {
    match ((yaw / 90.0).round() as i32).rem_euclid(4) {
        0 => "south",
        1 => "west",
        2 => "north",
        _ => "east",
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
            inventory_slot_from_container(9),
            Some(InventorySlot::Main(0))
        );
        assert_eq!(
            inventory_slot_from_container(35),
            Some(InventorySlot::Main(26))
        );
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

    #[test]
    fn maps_block_items_to_default_block_states_from_reports() {
        assert_eq!(
            placed_block_state_for_item(&simple_item(STONE_ITEM_ID, 1)),
            Some(STONE_BLOCK_STATE_ID)
        );
        assert_eq!(
            picked_item_for_block_state(STONE_BLOCK_STATE_ID),
            Some(STONE_ITEM_ID)
        );
        assert!(placed_block_state_for_item(&simple_item(923, 1)).is_none());
    }

    #[test]
    fn consuming_selected_item_updates_mainhand() {
        let mut inventory = PlayerInventory::default();
        inventory.hotbar[0] = simple_item(STONE_ITEM_ID, 64);
        inventory.set_equipment_slot(Equipment::MAINHAND, inventory.hotbar[0].clone());

        let (slot, held) = inventory.consume_selected_one().unwrap();

        assert_eq!(slot, 0);
        assert_eq!(held.item_count.0, 63);
        assert_eq!(inventory.visible_equipment()[0].item.item_count.0, 63);
    }

    #[test]
    fn adding_item_stack_merges_hotbar_slots() {
        let mut inventory = PlayerInventory::default();
        inventory.hotbar[0] = simple_item(STONE_ITEM_ID, 63);
        inventory.set_equipment_slot(Equipment::MAINHAND, inventory.hotbar[0].clone());

        let changes = inventory
            .add_item_stack(&simple_item(STONE_ITEM_ID, 1))
            .unwrap();

        assert_eq!(changes.len(), 1);
        assert_eq!(inventory.held_item().item_count.0, 64);
        assert_eq!(inventory.visible_equipment()[0].item.item_count.0, 64);
    }

    #[test]
    fn draining_droppable_items_clears_hotbar_and_mainhand() {
        let mut inventory = PlayerInventory::default();
        inventory.hotbar[0] = simple_item(STONE_ITEM_ID, 64);
        inventory.main[0] = simple_item(2, 1);
        inventory.set_equipment_slot(Equipment::MAINHAND, inventory.hotbar[0].clone());

        let (drops, changes) = inventory.drain_droppable_items();

        assert_eq!(drops.len(), 2);
        assert_eq!(drops[0].item_count.0, 64);
        assert_eq!(inventory.held_item().item_count.0, 0);
        assert!(changes.iter().any(
            |change| matches!(change, InventorySlotChange::Hotbar { slot: 0, item } if item.item_count.0 == 0)
        ));
        assert!(changes.iter().any(
            |change| matches!(change, InventorySlotChange::Main { slot: 0, item } if item.item_count.0 == 0)
        ));
        assert!(
            inventory
                .visible_equipment()
                .iter()
                .any(|equipment| equipment.slot == Equipment::MAINHAND
                    && equipment.item.item_count.0 == 0)
        );
    }

    #[test]
    fn adding_item_stack_uses_empty_hotbar_slot() {
        let mut inventory = PlayerInventory::default();

        let changes = inventory.add_item_stack(&simple_item(2, 1)).unwrap();

        assert_eq!(changes.len(), 1);
        assert_eq!(inventory.hotbar[0].item_id.as_ref().unwrap().0, 2);
        assert_eq!(inventory.hotbar[0].item_count.0, 1);
    }

    #[test]
    fn adding_item_stack_refuses_full_hotbar() {
        let mut inventory = PlayerInventory::default();
        for slot in &mut inventory.hotbar {
            *slot = simple_item(STONE_ITEM_ID, 64);
        }
        for slot in &mut inventory.main {
            *slot = simple_item(STONE_ITEM_ID, 64);
        }

        assert!(inventory.add_item_stack(&simple_item(2, 1)).is_none());
    }

    #[test]
    fn adding_item_stack_uses_main_inventory_after_hotbar() {
        let mut inventory = PlayerInventory::default();
        for slot in &mut inventory.hotbar {
            *slot = simple_item(STONE_ITEM_ID, 64);
        }

        let changes = inventory.add_item_stack(&simple_item(2, 1)).unwrap();

        assert_eq!(changes.len(), 1);
        assert!(matches!(
            &changes[0],
            InventorySlotChange::Main { slot: 0, item }
                if item.item_id.as_ref().unwrap().0 == 2 && item.item_count.0 == 1
        ));
    }

    #[test]
    fn creative_slot_maps_main_inventory() {
        let mut inventory = PlayerInventory::default();
        let change = inventory.set_creative_slot(9, simple_item(2, 3)).unwrap();

        assert!(matches!(
            change,
            InventorySlotChange::Main { slot: 0, item }
                if item.item_id.as_ref().unwrap().0 == 2 && item.item_count.0 == 3
        ));
        assert_eq!(inventory.main[0].item_id.as_ref().unwrap().0, 2);
    }

    #[test]
    fn block_report_metadata_marks_air_and_vegetation_replaceable() {
        assert!(is_air_block_state(AIR_BLOCK_STATE_ID));
        assert!(can_replace_block_state(AIR_BLOCK_STATE_ID));
        assert!(!block_has_collision(AIR_BLOCK_STATE_ID));
        assert!(!can_replace_block_state(STONE_BLOCK_STATE_ID));
        assert!(block_has_collision(STONE_BLOCK_STATE_ID));
    }

    #[test]
    fn block_report_metadata_pairs_double_height_halves() {
        assert_eq!(upper_half_block_state(12920), Some(12919));
        assert_eq!(lower_half_block_state(12919), Some(12920));
    }

    #[test]
    fn placement_context_selects_axis_for_logs() {
        let context = PlacementContext {
            face: 5,
            cursor_y: 0.5,
            player_yaw: 0.0,
        };

        assert_eq!(block_state_for_placement(137, context), 136);
    }

    #[test]
    fn placement_context_selects_slab_half() {
        let context = PlacementContext {
            face: 3,
            cursor_y: 0.8,
            player_yaw: 0.0,
        };

        assert_eq!(block_state_for_placement(13399, context), 13397);
    }

    #[test]
    fn placement_context_selects_stair_half_and_facing() {
        let context = PlacementContext {
            face: 1,
            cursor_y: 0.4,
            player_yaw: 90.0,
        };

        assert_eq!(block_state_for_placement(15787, context), 15827);
    }

    #[test]
    fn placement_context_keeps_floor_torch_on_top_clicks() {
        let context = PlacementContext {
            face: 1,
            cursor_y: 0.4,
            player_yaw: 0.0,
        };

        assert_eq!(block_state_for_placement(3370, context), 3370);
    }
}
