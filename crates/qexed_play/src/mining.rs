use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
    time::Duration,
};

use qexed_packet::net_types::Position as BlockPosition;
use qexed_protocol::types::{ComponentsToAdd, Slot};

use qexed_plugins::api::{
    BlockDropPosition, BlockDropQuery, ItemEnchantment, MiningSpeedQuery, PluginEnchantment,
};

/// Duration of a single gameplay tick (20 TPS).
pub(crate) const TICK_DURATION: Duration = Duration::from_millis(50);

const TICK_SECONDS: f64 = 0.05;
const DEFAULT_HARDNESS: f32 = 1.0;
const MIN_BREAK_DURATION: Duration = Duration::from_millis(50);
const EFFICIENCY_ENCHANTMENT_ID: i32 = 8;
const FORTUNE_ENCHANTMENT_ID: i32 = 13;
const SILK_TOUCH_ENCHANTMENT_ID: i32 = 34;
const UNBREAKING_ENCHANTMENT_ID: i32 = 40;

#[derive(Debug, Clone)]
pub(crate) struct PendingDig {
    pub(crate) position: BlockPosition,
    pub(crate) block_state: i32,
    pub(crate) held_signature: HeldItemSignature,
    pub(crate) required: Duration,
    /// Accumulated game ticks elapsed since mining started.
    /// Incremented by [`tick()`] once per gameplay tick (20 TPS).
    elapsed: Duration,
}

impl PendingDig {
    pub(crate) fn new(
        position: BlockPosition,
        block_state: i32,
        held_item: &Slot,
        required: Duration,
    ) -> Self {
        Self {
            position,
            block_state,
            held_signature: HeldItemSignature::from_slot(held_item),
            required,
            elapsed: Duration::ZERO,
        }
    }

    /// Advance one gameplay tick (50 ms of game time).
    pub(crate) fn tick(&mut self) {
        self.elapsed = self.elapsed.saturating_add(TICK_DURATION);
    }

    pub(crate) fn matches(
        &self,
        position: &BlockPosition,
        block_state: i32,
        held_item: &Slot,
    ) -> bool {
        self.position == position.clone()
            && self.block_state == block_state
            && self.held_signature == HeldItemSignature::from_slot(held_item)
    }

    pub(crate) fn is_complete(&self) -> bool {
        // Allow up to 1.5 ticks early to compensate for latency between the
        // client finishing its local progress and the STOP_DESTROY_BLOCK
        // packet reaching the server.
        self.elapsed.saturating_add(Duration::from_millis(75)) >= self.required
    }

    /// Calculate the block destruction stage (0-9) based on accumulated tick
    /// time.  Returns None if breaking hasn't started or the required time is
    /// zero.
    pub(crate) fn destroy_stage(&self) -> Option<i8> {
        if self.required.is_zero() {
            return None;
        }
        let progress = self.elapsed.as_secs_f64() / self.required.as_secs_f64();
        let stage = (progress * 10.0).floor() as i8;
        Some(stage.min(9))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HeldItemSignature {
    item_id: Option<i32>,
    components_to_add: Option<Vec<ComponentsToAdd>>,
    components_to_remove: Option<Vec<qexed_packet::net_types::VarInt>>,
}

impl HeldItemSignature {
    fn from_slot(slot: &Slot) -> Self {
        Self {
            item_id: slot.item_id.as_ref().map(|id| id.0),
            components_to_add: slot.components_to_add.clone(),
            components_to_remove: slot.components_to_remove.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolKind {
    Pickaxe,
    Axe,
    Shovel,
    Hoe,
    Shears,
    Sword,
    Other,
}

#[derive(Debug, Clone, Copy)]
struct ToolInfo {
    kind: ToolKind,
    speed: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct MiningContext {
    pub(crate) block_state: i32,
    pub(crate) block_name: String,
    pub(crate) item_id: Option<i32>,
    pub(crate) enchantments: Vec<ItemEnchantment>,
    pub(crate) plugin_enchantments: Vec<PluginEnchantment>,
}

pub(crate) fn mining_context(block_state: i32, held_item: &Slot) -> MiningContext {
    let item_id = held_item.item_id.as_ref().map(|id| id.0);
    let enchantments = item_enchantments(held_item);
    let plugin_enchantments = plugin_enchantments(held_item);

    MiningContext {
        block_state,
        block_name: crate::inventory::block_name_for_state(block_state)
            .unwrap_or_else(|| "minecraft:unknown".to_string()),
        item_id,
        enchantments,
        plugin_enchantments,
    }
}

pub(crate) fn held_item_allows_adventure_break(held_item: &Slot, block_state: i32) -> bool {
    let Some(components) = held_item.components_to_add.as_deref() else {
        return false;
    };
    let Some(block_name) = crate::inventory::block_name_for_state(block_state) else {
        return false;
    };
    let Some(block_id) = block_registry_id(&block_name) else {
        return false;
    };
    let block_properties =
        crate::inventory::block_properties_for_state(block_state).unwrap_or_default();

    components.iter().any(|component| {
        let ComponentsToAdd::MinecraftCanBreak(can_break) = component else {
            return false;
        };
        can_break
            .block_predicates
            .iter()
            .any(|predicate| block_predicate_allows(predicate, block_id, &block_properties))
    })
}

fn block_predicate_allows(
    predicate: &qexed_protocol::types::minecraft::BlockPredicate,
    block_id: i32,
    block_properties: &HashMap<String, String>,
) -> bool {
    if predicate.nbt.is_some() {
        return false;
    }
    if !predicate
        .blocks
        .as_ref()
        .is_none_or(|blocks| block_id_set_contains(blocks, block_id))
    {
        return false;
    }
    predicate
        .properties
        .as_ref()
        .is_none_or(|properties| state_properties_match(properties, block_properties))
}

fn block_id_set_contains(id_set: &qexed_protocol::types::IDSet, block_id: i32) -> bool {
    if let Some(ids) = &id_set.ids {
        return ids.iter().any(|id| id.0 == block_id);
    }
    id_set
        .tag_name
        .as_deref()
        .is_some_and(|tag| block_tag_contains(tag, block_id))
}

fn state_properties_match(
    predicate: &qexed_protocol::types::minecraft::StatePropertiesPredicate,
    block_properties: &HashMap<String, String>,
) -> bool {
    predicate.properties.iter().all(|property| {
        let Some(value) = block_properties.get(&property.name) else {
            return false;
        };
        match &property.matcher {
            qexed_protocol::types::minecraft::StatePropertyMatcherValue::Exact(expected) => {
                value == expected
            }
            qexed_protocol::types::minecraft::StatePropertyMatcherValue::Range { min, max } => {
                property_range_contains(value, min.as_deref(), max.as_deref())
            }
        }
    })
}

fn property_range_contains(value: &str, min: Option<&str>, max: Option<&str>) -> bool {
    if let Ok(value) = value.parse::<i64>() {
        if let Some(min) = min.and_then(|min| min.parse::<i64>().ok())
            && value < min
        {
            return false;
        }
        if let Some(max) = max.and_then(|max| max.parse::<i64>().ok())
            && value > max
        {
            return false;
        }
        return true;
    }

    min.is_none_or(|min| value >= min) && max.is_none_or(|max| value <= max)
}

fn block_registry_id(block_name: &str) -> Option<i32> {
    static BLOCK_IDS: OnceLock<Option<HashMap<String, i32>>> = OnceLock::new();
    BLOCK_IDS
        .get_or_init(|| qexed_mojang_data::registry_sync::load_registry_id_map("minecraft:block").ok())
        .as_ref()?
        .get(block_name)
        .copied()
}

fn block_tag_contains(tag_name: &str, block_id: i32) -> bool {
    static BLOCK_TAGS: OnceLock<HashMap<String, HashSet<i32>>> = OnceLock::new();
    let tag_name = normalize_tag_name(tag_name);
    BLOCK_TAGS
        .get_or_init(load_block_tags)
        .get(&tag_name)
        .is_some_and(|ids| ids.contains(&block_id))
}

fn load_block_tags() -> HashMap<String, HashSet<i32>> {
    let Ok(packet) = qexed_mojang_data::registry_sync::load_tag_packet() else {
        return HashMap::new();
    };
    let Some(block_registry) = packet
        .tags
        .into_iter()
        .find(|registry| registry.registry == "minecraft:block")
    else {
        return HashMap::new();
    };

    block_registry
        .tags
        .into_iter()
        .map(|tag| {
            (
                tag.name,
                tag.entries.into_iter().map(|entry| entry.0).collect(),
            )
        })
        .collect()
}

fn normalize_tag_name(tag_name: &str) -> String {
    let tag_name = tag_name.strip_prefix('#').unwrap_or(tag_name);
    if tag_name.contains(':') {
        tag_name.to_string()
    } else {
        format!("minecraft:{tag_name}")
    }
}

pub(crate) fn required_break_duration(
    block_state: i32,
    held_item: &Slot,
    plugins: &qexed_plugins::PluginManager,
) -> Duration {
    if crate::inventory::is_air_block_state(block_state) {
        return Duration::ZERO;
    }

    let mut context = mining_context(block_state, held_item);
    let base_speed = vanilla_mining_speed(&context);
    let query = MiningSpeedQuery {
        block_state,
        block_name: context.block_name.clone(),
        item_id: context.item_id,
        enchantments: context.enchantments.clone(),
        plugin_enchantments: context.plugin_enchantments.clone(),
        speed: base_speed,
    };
    let speed = plugins.apply_mining_speed(query).max(0.01);
    context.plugin_enchantments.clear();
    break_duration(block_hardness(&context.block_name), speed)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn default_block_drops(
    block_state: i32,
    position: &BlockPosition,
    held_item: &Slot,
    plugins: &qexed_plugins::PluginManager,
) -> Vec<Slot> {
    block_drop_outcome(block_state, position, held_item, plugins, None, None).drops
}

#[derive(Debug, Clone)]
pub(crate) struct BlockDropOutcome {
    pub(crate) drops: Vec<Slot>,
    pub(crate) break_positions: Vec<BlockPosition>,
}

pub(crate) fn block_drop_outcome(
    block_state: i32,
    position: &BlockPosition,
    held_item: &Slot,
    plugins: &qexed_plugins::PluginManager,
    player: Option<qexed_plugins::api::PlayerPayloadOwned>,
    player_position: Option<qexed_plugins::api::PlayerPositionPayload>,
) -> BlockDropOutcome {
    let context = mining_context(block_state, held_item);
    let default_item_id = vanilla_drop_item_id(&context)
        .or_else(|| crate::inventory::picked_item_for_block_state(block_state));
    let mut drops = default_item_id
        .map(|item_id| {
            vec![crate::inventory::simple_item(
                item_id,
                vanilla_drop_count(&context),
            )]
        })
        .unwrap_or_default();
    let query = BlockDropQuery {
        player,
        player_position,
        block_state,
        block_name: context.block_name.clone(),
        position: BlockDropPosition {
            x: position.x,
            y: position.y,
            z: position.z,
        },
        tool_item_id: context.item_id,
        enchantments: context.enchantments.clone(),
        plugin_enchantments: context.plugin_enchantments.clone(),
        default_item_id,
    };

    let Some(plugin_result) = plugins.apply_block_drops(query) else {
        return BlockDropOutcome {
            drops,
            break_positions: Vec::new(),
        };
    };
    let plugin_drops = plugin_result
        .items
        .into_iter()
        .filter(|item| item.count > 0)
        .filter_map(|item| {
            let item_id = if item.item_id >= 0 {
                Some(item.item_id)
            } else {
                crate::inventory::item_id_for_name(&normalize_item_name(&item.item_name))
            }?;
            Some(crate::inventory::simple_item(item_id, item.count.min(64)))
        })
        .collect::<Vec<_>>();

    drops = if plugin_result.replace {
        plugin_drops
    } else {
        drops.extend(plugin_drops);
        drops
    };
    let break_positions = plugin_result
        .break_positions
        .into_iter()
        .map(|position| BlockPosition {
            x: position.x,
            y: position.y,
            z: position.z,
        })
        .collect();
    BlockDropOutcome {
        drops,
        break_positions,
    }
}

fn normalize_item_name(value: &str) -> String {
    let value = value.trim();
    if value.contains(':') {
        value.to_string()
    } else {
        format!("minecraft:{value}")
    }
}

pub(crate) fn drop_position(
    position: &BlockPosition,
) -> qexed_protocol::types::EntityPosition {
    qexed_protocol::types::EntityPosition {
        x: position.x as f64 + 0.5,
        y: position.y as f64 + 0.375,
        z: position.z as f64 + 0.5,
        yaw: 0.0,
        pitch: 0.0,
        on_ground: false,
    }
}

fn vanilla_mining_speed(context: &MiningContext) -> f32 {
    let tool = context
        .item_id
        .and_then(tool_info_for_item)
        .unwrap_or(ToolInfo {
            kind: ToolKind::Other,
            speed: 1.0,
        });
    let mut speed = if tool_matches_block(tool.kind, &context.block_name) {
        tool.speed
    } else {
        1.0
    };

    if speed > 1.0 {
        if let Some(level) = enchantment_level(&context.enchantments, "minecraft:efficiency") {
            speed += (level * level + 1) as f32;
        }
    }

    speed
}

fn vanilla_drop_item_id(context: &MiningContext) -> Option<i32> {
    if enchantment_level(&context.enchantments, "minecraft:silk_touch").is_some() {
        return crate::inventory::picked_item_for_block_state(context.block_state);
    }

    let item_name = match context.block_name.as_str() {
        "minecraft:stone" => "minecraft:cobblestone",
        "minecraft:deepslate" => "minecraft:cobbled_deepslate",
        "minecraft:coal_ore" | "minecraft:deepslate_coal_ore" => "minecraft:coal",
        "minecraft:diamond_ore" | "minecraft:deepslate_diamond_ore" => "minecraft:diamond",
        "minecraft:emerald_ore" | "minecraft:deepslate_emerald_ore" => "minecraft:emerald",
        "minecraft:lapis_ore" | "minecraft:deepslate_lapis_ore" => "minecraft:lapis_lazuli",
        "minecraft:redstone_ore" | "minecraft:deepslate_redstone_ore" => "minecraft:redstone",
        "minecraft:copper_ore" | "minecraft:deepslate_copper_ore" => "minecraft:raw_copper",
        "minecraft:iron_ore" | "minecraft:deepslate_iron_ore" => "minecraft:raw_iron",
        "minecraft:gold_ore" | "minecraft:deepslate_gold_ore" => "minecraft:raw_gold",
        "minecraft:nether_gold_ore" => "minecraft:gold_nugget",
        "minecraft:nether_quartz_ore" => "minecraft:quartz",
        _ => return None,
    };
    crate::inventory::item_id_for_name(item_name)
}

fn vanilla_drop_count(context: &MiningContext) -> i32 {
    if enchantment_level(&context.enchantments, "minecraft:silk_touch").is_some() {
        return 1;
    }
    let fortune = enchantment_level(&context.enchantments, "minecraft:fortune").unwrap_or(0);
    match context.block_name.as_str() {
        "minecraft:lapis_ore" | "minecraft:deepslate_lapis_ore" => 4 + fortune.max(0),
        "minecraft:redstone_ore" | "minecraft:deepslate_redstone_ore" => 4 + fortune.max(0),
        "minecraft:copper_ore" | "minecraft:deepslate_copper_ore" => 2 + fortune.max(0),
        "minecraft:nether_gold_ore" => 2 + fortune.max(0),
        "minecraft:coal_ore"
        | "minecraft:deepslate_coal_ore"
        | "minecraft:diamond_ore"
        | "minecraft:deepslate_diamond_ore"
        | "minecraft:emerald_ore"
        | "minecraft:deepslate_emerald_ore"
        | "minecraft:nether_quartz_ore" => 1 + fortune.max(0),
        _ => 1,
    }
}

fn break_duration(hardness: f32, speed: f32) -> Duration {
    if hardness <= 0.0 {
        return MIN_BREAK_DURATION;
    }
    let ticks = (hardness as f64 * 30.0 / speed as f64).ceil().max(1.0);
    Duration::from_secs_f64(ticks * TICK_SECONDS).max(MIN_BREAK_DURATION)
}

fn item_enchantments(slot: &Slot) -> Vec<ItemEnchantment> {
    let mut result = Vec::new();
    let Some(components) = slot.components_to_add.as_ref() else {
        return result;
    };

    for component in components {
        let enchantments = match component {
            ComponentsToAdd::MinecraftEnchantments(enchantments) => &enchantments.enchantments,
            ComponentsToAdd::MinecraftStoredEnchantments(enchantments) => {
                &enchantments.enchantments
            }
            _ => continue,
        };
        for enchantment in enchantments {
            if enchantment.level.0 <= 0 {
                continue;
            }
            result.push(ItemEnchantment {
                id: vanilla_enchantment_name(enchantment.enchantment.0).to_string(),
                level: enchantment.level.0,
            });
        }
    }

    result
}

fn plugin_enchantments(slot: &Slot) -> Vec<PluginEnchantment> {
    let mut result = Vec::new();
    let Some(components) = slot.components_to_add.as_ref() else {
        return result;
    };

    for component in components {
        let ComponentsToAdd::MinecraftCustomData(custom_data) = component else {
            continue;
        };
        collect_plugin_enchantments(&custom_data.data, &mut result);
    }
    result
}

fn collect_plugin_enchantments(tag: &qexed_nbt::Tag, out: &mut Vec<PluginEnchantment>) {
    let qexed_nbt::Tag::Compound(root) = tag else {
        return;
    };
    let Some(enchantments) = root
        .get("qexed:enchantments")
        .or_else(|| root.get("qexed_enchantments"))
    else {
        return;
    };

    match enchantments {
        qexed_nbt::Tag::Compound(values) => {
            for (id, level) in values.iter() {
                if let Some(level) = tag_i32(level).filter(|level| *level > 0) {
                    out.push(PluginEnchantment {
                        id: id.clone(),
                        level,
                    });
                }
            }
        }
        qexed_nbt::Tag::List(_, values) => {
            for value in values.iter() {
                let qexed_nbt::Tag::Compound(entry) = value else {
                    continue;
                };
                let Some(qexed_nbt::Tag::String(id)) = entry.get("id") else {
                    continue;
                };
                let level = entry.get("level").and_then(tag_i32).unwrap_or(1);
                if level > 0 {
                    out.push(PluginEnchantment {
                        id: id.to_string(),
                        level,
                    });
                }
            }
        }
        _ => {}
    }
}

fn tag_i32(tag: &qexed_nbt::Tag) -> Option<i32> {
    match tag {
        qexed_nbt::Tag::Byte(value) => Some(i32::from(*value)),
        qexed_nbt::Tag::Short(value) => Some(i32::from(*value)),
        qexed_nbt::Tag::Int(value) => Some(*value),
        qexed_nbt::Tag::Long(value) => i32::try_from(*value).ok(),
        _ => None,
    }
}

fn enchantment_level(enchantments: &[ItemEnchantment], id: &str) -> Option<i32> {
    enchantments
        .iter()
        .find(|enchantment| enchantment.id == id)
        .map(|enchantment| enchantment.level)
}

fn vanilla_enchantment_name(id: i32) -> &'static str {
    match id {
        EFFICIENCY_ENCHANTMENT_ID => "minecraft:efficiency",
        FORTUNE_ENCHANTMENT_ID => "minecraft:fortune",
        SILK_TOUCH_ENCHANTMENT_ID => "minecraft:silk_touch",
        UNBREAKING_ENCHANTMENT_ID => "minecraft:unbreaking",
        _ => "minecraft:unknown",
    }
}

fn tool_info_for_item(item_id: i32) -> Option<ToolInfo> {
    let ids = item_ids();
    let name = ids.get(&item_id)?.as_str();
    let speed = if name.contains("wooden_") || name.contains("stone_") {
        2.0
    } else if name.contains("iron_") {
        6.0
    } else if name.contains("diamond_") {
        8.0
    } else if name.contains("netherite_") {
        9.0
    } else if name.contains("golden_") {
        12.0
    } else if name == "minecraft:shears" {
        5.0
    } else {
        1.0
    };
    let kind = if name.ends_with("_pickaxe") {
        ToolKind::Pickaxe
    } else if name.ends_with("_axe") {
        ToolKind::Axe
    } else if name.ends_with("_shovel") {
        ToolKind::Shovel
    } else if name.ends_with("_hoe") {
        ToolKind::Hoe
    } else if name.ends_with("_sword") {
        ToolKind::Sword
    } else if name == "minecraft:shears" {
        ToolKind::Shears
    } else {
        ToolKind::Other
    };
    Some(ToolInfo { kind, speed })
}

fn tool_matches_block(kind: ToolKind, block_name: &str) -> bool {
    match kind {
        ToolKind::Pickaxe => is_pickaxe_block(block_name),
        ToolKind::Axe => is_axe_block(block_name),
        ToolKind::Shovel => is_shovel_block(block_name),
        ToolKind::Hoe => is_hoe_block(block_name),
        ToolKind::Shears => is_shears_block(block_name),
        ToolKind::Sword => block_name.ends_with("_web"),
        ToolKind::Other => false,
    }
}

fn is_pickaxe_block(name: &str) -> bool {
    name.contains("stone")
        || name.contains("ore")
        || name.contains("deepslate")
        || name.contains("netherrack")
        || name.contains("basalt")
        || name.contains("blackstone")
        || name.contains("andesite")
        || name.contains("diorite")
        || name.contains("granite")
        || name.contains("tuff")
        || name.contains("copper")
        || name.contains("iron")
        || name.contains("gold")
        || name.contains("diamond")
        || name.contains("emerald")
        || name.contains("lapis")
        || name.contains("redstone")
        || name.contains("coal")
        || name.contains("quartz")
        || name.contains("obsidian")
        || name.contains("prismarine")
        || name.contains("brick")
        || name.contains("terracotta")
        || name.contains("concrete")
        || name.contains("anvil")
        || name.contains("furnace")
        || name.contains("ancient_debris")
        || name.contains("netherite")
        || name.contains("end_stone")
        || name.contains("sandstone")
        || name.contains("calcite")
        || name.contains("amethyst")
        || name.contains("dripstone")
        || name.contains("shulker")
        || name.contains("lodestone")
        || name.contains("bell")
        || name.contains("enchanting_table")
        || name.contains("ender_chest")
        || name.contains("grindstone")
        || name.contains("hopper")
        || name.contains("piston")
        || name.contains("purpur")
        || name.contains("spawner")
        || name.contains("ice")
}

fn is_axe_block(name: &str) -> bool {
    name.contains("log")
        || name.contains("wood")
        || name.contains("stem")
        || name.contains("hyphae")
        || name.contains("planks")
        || name.contains("fence")
        || name.contains("door")
        || name.contains("trapdoor")
        || name.contains("sign")
        || name.contains("chest")
        || name.contains("bookshelf")
        || name.contains("barrel")
        || name.contains("crafting_table")
}

fn is_shovel_block(name: &str) -> bool {
    name.contains("dirt")
        || name.contains("grass_block")
        || name.contains("sand")
        || name.contains("gravel")
        || name.contains("clay")
        || name.contains("snow")
        || name.contains("soul_sand")
        || name.contains("soul_soil")
        || name.contains("mud")
        || name.contains("mycelium")
        || name.contains("podzol")
}

fn is_hoe_block(name: &str) -> bool {
    name.contains("leaves")
        || name.contains("hay_block")
        || name.contains("moss")
        || name.contains("sculk")
        || name.contains("sponge")
        || name.contains("target")
        || name.contains("nether_wart_block")
}

fn is_shears_block(name: &str) -> bool {
    name.contains("leaves")
        || name.contains("wool")
        || name.contains("vine")
        || name.contains("grass")
        || name.contains("fern")
        || name.contains("web")
}

fn block_hardness(name: &str) -> f32 {
    if name == "minecraft:air" {
        0.0
    } else if name.contains("obsidian") {
        50.0
    } else if name.contains("ancient_debris") {
        30.0
    } else if name.contains("ender_chest") {
        22.5
    } else if name.contains("deepslate") || name.contains("anvil") {
        3.0
    } else if name.contains("ore")
        || name.contains("stone")
        || name.contains("brick")
        || name.contains("concrete")
        || name.contains("terracotta")
    {
        1.5
    } else if name.contains("log")
        || name.contains("wood")
        || name.contains("stem")
        || name.contains("hyphae")
    {
        2.0
    } else if name.contains("planks")
        || name.contains("chest")
        || name.contains("barrel")
        || name.contains("crafting_table")
    {
        2.5
    } else if name.contains("dirt")
        || name.contains("sand")
        || name.contains("gravel")
        || name.contains("clay")
        || name.contains("snow")
        || name.contains("leaves")
        || name.contains("grass")
        || name.contains("flower")
        || name.contains("mushroom")
    {
        0.5
    } else if name.contains("wool") || name.contains("carpet") {
        0.8
    } else {
        DEFAULT_HARDNESS
    }
}

fn item_ids() -> &'static HashMap<i32, String> {
    static IDS: std::sync::OnceLock<HashMap<i32, String>> = std::sync::OnceLock::new();
    IDS.get_or_init(|| crate::inventory::item_id_name_map())
}

#[cfg(test)]
mod tests {
    use super::*;
    use qexed_packet::net_types::VarInt;
    use qexed_protocol::types::{ComponentsToAdd, minecraft};
    use std::{collections::HashMap, sync::Arc};

    /// 空插件管理器（v6 qexed_plugins 的 empty_for_tests 是其 crate 内
    /// cfg(test)，跨 crate 不可见；从无插件目录构建等价实例）。
    fn empty_plugins() -> qexed_plugins::PluginManager {
        let dir = std::env::temp_dir().join(format!(
            "qexed-play-mining-empty-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&dir);
        let manager = qexed_plugins::PluginManager::from_dir(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        manager
    }

    #[test]
    #[ignore = "TODO(data): 依赖 mojang blocks.json 注册表数据"]
    fn pickaxe_is_faster_than_hand_on_stone() {
        let hand = crate::inventory::empty_slot();
        let pickaxe = crate::inventory::simple_item(934, 1);
        let plugins = empty_plugins();

        let hand_duration = required_break_duration(1, &hand, &plugins);
        let pickaxe_duration = required_break_duration(1, &pickaxe, &plugins);

        assert!(pickaxe_duration < hand_duration);
    }

    #[test]
    #[ignore = "TODO(data): 依赖 mojang blocks.json 注册表数据"]
    fn efficiency_enchantment_increases_speed() {
        let plugins = empty_plugins();
        let normal = crate::inventory::simple_item(934, 1);
        let mut efficient = normal.clone();
        efficient.number_of_components_to_add = Some(VarInt(1));
        efficient.components_to_add = Some(vec![ComponentsToAdd::MinecraftEnchantments(
            minecraft::Enchantments {
                enchantments: vec![minecraft::Enchantment {
                    enchantment: VarInt(EFFICIENCY_ENCHANTMENT_ID),
                    level: VarInt(3),
                }],
            },
        )]);

        assert!(
            required_break_duration(1, &efficient, &plugins)
                < required_break_duration(1, &normal, &plugins)
        );
    }

    #[test]
    #[ignore = "TODO(data): 依赖 mojang blocks.json 注册表数据"]
    fn stone_drops_cobblestone_without_silk_touch() {
        let plugins = empty_plugins();
        let drops = default_block_drops(
            1,
            &BlockPosition { x: 0, y: 64, z: 0 },
            &crate::inventory::empty_slot(),
            &plugins,
        );

        assert_eq!(drops.len(), 1);
        assert_eq!(drops[0].item_id.as_ref().unwrap().0, 35);
    }

    #[test]
    fn silk_touch_keeps_original_block_drop() {
        let plugins = empty_plugins();
        let mut tool = crate::inventory::simple_item(934, 1);
        tool.number_of_components_to_add = Some(VarInt(1));
        tool.components_to_add = Some(vec![ComponentsToAdd::MinecraftEnchantments(
            minecraft::Enchantments {
                enchantments: vec![minecraft::Enchantment {
                    enchantment: VarInt(SILK_TOUCH_ENCHANTMENT_ID),
                    level: VarInt(1),
                }],
            },
        )]);

        let drops = default_block_drops(1, &BlockPosition { x: 0, y: 64, z: 0 }, &tool, &plugins);

        assert_eq!(drops.len(), 1);
        assert_eq!(drops[0].item_id.as_ref().unwrap().0, 1);
    }

    #[test]
    #[ignore = "TODO(data): 依赖 mojang blocks.json 注册表数据"]
    fn fortune_increases_known_ore_drop_count() {
        let plugins = empty_plugins();
        let mut tool = crate::inventory::simple_item(934, 1);
        tool.number_of_components_to_add = Some(VarInt(1));
        tool.components_to_add = Some(vec![ComponentsToAdd::MinecraftEnchantments(
            minecraft::Enchantments {
                enchantments: vec![minecraft::Enchantment {
                    enchantment: VarInt(FORTUNE_ENCHANTMENT_ID),
                    level: VarInt(3),
                }],
            },
        )]);

        let drops =
            default_block_drops(5307, &BlockPosition { x: 0, y: 64, z: 0 }, &tool, &plugins);

        assert_eq!(drops.len(), 1);
        assert_eq!(drops[0].item_id.as_ref().unwrap().0, 899);
        assert_eq!(drops[0].item_count.0, 4);
    }

    #[test]
    fn custom_data_exposes_plugin_enchantments() {
        let mut enchantments = HashMap::new();
        enchantments.insert("example:haste".to_string(), qexed_nbt::Tag::Int(2));
        let mut custom_data = HashMap::new();
        custom_data.insert(
            "qexed:enchantments".to_string(),
            qexed_nbt::Tag::Compound(Arc::new(enchantments)),
        );
        let mut item = crate::inventory::simple_item(934, 1);
        item.number_of_components_to_add = Some(VarInt(1));
        item.components_to_add = Some(vec![ComponentsToAdd::MinecraftCustomData(
            minecraft::CustomData {
                data: qexed_nbt::Tag::Compound(Arc::new(custom_data)),
            },
        )]);

        let context = mining_context(1, &item);

        assert_eq!(
            context.plugin_enchantments,
            vec![PluginEnchantment {
                id: "example:haste".to_string(),
                level: 2,
            }]
        );
    }
}
