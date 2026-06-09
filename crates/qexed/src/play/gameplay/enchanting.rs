use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
    time::SystemTime,
};

use anyhow::Result;
use qexed_packet::net_types::Position as BlockPosition;
use qexed_packet::net_types::VarInt;
use qexed_protocol::{
    to_client::play::{
        container_set_content::ContainerSetContent, container_set_data::ContainerSetData,
        container_set_slot, open_screen::OpenScreen, set_experience::SetExperience,
    },
    to_server::play::{
        container_button_click::ContainerButtonClick, container_click::ContainerClick,
    },
    types::{ComponentsToAdd, Slot, minecraft},
};
use rand::{Rng, SeedableRng, rngs::StdRng};

use super::items;
use crate::{
    config::RuntimeConfig,
    play::util::text_component,
    plugins::{EnchantingOption, EnchantingQuery},
};

pub(in crate::play) const ENCHANTING_WINDOW_ID: i32 = 23;
const INPUT_SLOT: i16 = 0;
const LAPIS_SLOT: i16 = 1;
const PLAYER_MAIN_START: i16 = 2;
const PLAYER_MAIN_END: i16 = 28;
const PLAYER_HOTBAR_START: i16 = 29;
const PLAYER_HOTBAR_END: i16 = 37;
const MAX_ADDITIONAL_ENCHANTMENTS: usize = 3;
const VANILLA_EXCLUSIVE_SETS: &[&[&str]] = &[
    &[
        "minecraft:protection",
        "minecraft:blast_protection",
        "minecraft:fire_protection",
        "minecraft:projectile_protection",
    ],
    &["minecraft:depth_strider", "minecraft:frost_walker"],
    &["minecraft:infinity", "minecraft:mending"],
    &["minecraft:multishot", "minecraft:piercing"],
    &[
        "minecraft:sharpness",
        "minecraft:smite",
        "minecraft:bane_of_arthropods",
        "minecraft:impaling",
        "minecraft:density",
        "minecraft:breach",
    ],
    &["minecraft:fortune", "minecraft:silk_touch"],
    &[
        "minecraft:riptide",
        "minecraft:loyalty",
        "minecraft:channeling",
    ],
];

#[derive(Debug, Default)]
pub(in crate::play) struct EnchantingRuntime {
    open: bool,
    state_id: i32,
    input: Slot,
    lapis: Slot,
    offers: Vec<EnchantingOffer>,
    selected_offer: Option<usize>,
    seed: u64,
    table_position: Option<BlockPosition>,
}

impl EnchantingRuntime {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn is_open(&self) -> bool {
        self.open
    }

    pub(in crate::play) async fn open<W>(
        &mut self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        config: &RuntimeConfig,
        player: &crate::players::OnlinePlayer,
        plugins: &crate::plugins::PluginManager,
        world: &crate::world::WorldManager,
        world_dimension: &str,
        inventory: &crate::inventory::PlayerInventory,
        table_position: BlockPosition,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        self.open = true;
        self.state_id = self.state_id.wrapping_add(1);
        self.seed = current_seed();
        self.input = crate::inventory::empty_slot();
        self.lapis = crate::inventory::empty_slot();
        self.selected_offer = None;
        self.table_position = Some(table_position);
        self.recompute(config, player, plugins, world, world_dimension);
        sink.send(OpenScreen {
            window_id: VarInt(ENCHANTING_WINDOW_ID),
            menu_type: VarInt(menu_id("minecraft:enchantment")),
            title: text_component(&config.enchanting.title),
        })
        .await?;
        self.sync(sink, player_inventory_slots(inventory)).await
    }

    pub(in crate::play) async fn close<W>(
        &mut self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        inventory: &mut crate::inventory::PlayerInventory,
    ) -> Result<super::GameplayActionOutcome>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if !self.open {
            return Ok(super::GameplayActionOutcome::default());
        }
        self.open = false;
        let mut outcome = super::GameplayActionOutcome::handled();
        for item in [&mut self.input, &mut self.lapis] {
            if item.item_count.0 > 0
                && let Some(mut changes) = inventory.add_item_stack(item)
            {
                outcome.inventory_changes.append(&mut changes);
            }
            *item = crate::inventory::empty_slot();
        }
        self.table_position = None;
        sink.send(
            qexed_protocol::to_client::play::container_close::ContainerClose {
                window_id: VarInt(ENCHANTING_WINDOW_ID),
            },
        )
        .await?;
        outcome.close_window = true;
        Ok(outcome)
    }

    pub(in crate::play) async fn handle_button_click<W>(
        &mut self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        button: &ContainerButtonClick,
        inventory: &mut crate::inventory::PlayerInventory,
        player: &crate::players::OnlinePlayer,
        plugins: &crate::plugins::PluginManager,
        config: &RuntimeConfig,
        world: &crate::world::WorldManager,
        world_dimension: &str,
    ) -> Result<Option<super::GameplayActionOutcome>>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if !self.open || button.window_id.0 != ENCHANTING_WINDOW_ID {
            return Ok(None);
        }
        let Some(index) = usize::try_from(button.button_id.0).ok() else {
            return Ok(None);
        };
        if index >= self.offers.len() {
            return Ok(None);
        }
        let offer = self.offers[index].clone();
        if !self.can_apply_offer(&offer, config) {
            self.sync(sink, player_inventory_slots(inventory)).await?;
            return Ok(Some(super::GameplayActionOutcome::handled()));
        }
        if !self.consume_cost(&offer, config, inventory) {
            self.sync(sink, player_inventory_slots(inventory)).await?;
            return Ok(Some(super::GameplayActionOutcome::handled()));
        }
        let outcome = super::GameplayActionOutcome::handled();
        self.input = self.apply_enchantments(&offer, player, plugins, config, world_dimension)?;
        self.recompute(config, player, plugins, world, world_dimension);
        self.state_id = self.state_id.wrapping_add(1);
        self.sync(sink, player_inventory_slots(inventory)).await?;
        self.sync_input_slot(sink).await?;
        Ok(Some(outcome))
    }

    pub(in crate::play) async fn handle_click<W>(
        &mut self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        click: &ContainerClick,
        inventory: &mut crate::inventory::PlayerInventory,
        player: &crate::players::OnlinePlayer,
        plugins: &crate::plugins::PluginManager,
        config: &RuntimeConfig,
        world: &crate::world::WorldManager,
        world_dimension: &str,
    ) -> Result<Option<super::GameplayActionOutcome>>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if !self.open || click.window_id.0 != ENCHANTING_WINDOW_ID {
            return Ok(None);
        }
        let mut outcome = super::GameplayActionOutcome::handled();
        match click.slot {
            INPUT_SLOT => {
                if click.button == 1 {
                    if self.input.item_count.0 > 0
                        && let Some(mut changes) = inventory.add_item_stack(&self.input)
                    {
                        outcome.inventory_changes.append(&mut changes);
                    }
                    self.input = crate::inventory::empty_slot();
                } else if self.input.item_count.0 > 0 {
                    if let Some(mut changes) = inventory.add_item_stack(&self.input) {
                        outcome.inventory_changes.append(&mut changes);
                        self.input = crate::inventory::empty_slot();
                    }
                } else if let Some((_, item)) = inventory.drop_selected(false) {
                    if self.input.item_count.0 > 0
                        && let Some(mut changes) = inventory.add_item_stack(&self.input)
                    {
                        outcome.inventory_changes.append(&mut changes);
                    }
                    self.input = item;
                    outcome
                        .inventory_changes
                        .push(inventory.selected_hotbar_change());
                }
            }
            LAPIS_SLOT => {
                if click.button == 1 {
                    if self.lapis.item_count.0 > 0
                        && let Some(mut changes) = inventory.add_item_stack(&self.lapis)
                    {
                        outcome.inventory_changes.append(&mut changes);
                    }
                    self.lapis = crate::inventory::empty_slot();
                } else if self.lapis.item_count.0 > 0 {
                    if let Some(mut changes) = inventory.add_item_stack(&self.lapis) {
                        outcome.inventory_changes.append(&mut changes);
                        self.lapis = crate::inventory::empty_slot();
                    }
                } else if let Some((_, item)) = inventory.drop_selected(false) {
                    if same_resource_key(&items::item_name(&item), &config.enchanting.lapis_item) {
                        self.lapis = item;
                        outcome
                            .inventory_changes
                            .push(inventory.selected_hotbar_change());
                    } else if let Some(mut changes) = inventory.add_item_stack(&item) {
                        outcome.inventory_changes.append(&mut changes);
                    }
                }
            }
            PLAYER_MAIN_START..=PLAYER_HOTBAR_END => {
                self.handle_player_inventory_slot(click.slot, inventory, config, &mut outcome);
            }
            _ => {}
        }
        self.recompute(config, player, plugins, world, world_dimension);
        self.state_id = self.state_id.wrapping_add(1);
        self.sync(sink, player_inventory_slots(inventory)).await?;
        Ok(Some(outcome))
    }

    fn handle_player_inventory_slot(
        &mut self,
        slot: i16,
        inventory: &mut crate::inventory::PlayerInventory,
        config: &RuntimeConfig,
        outcome: &mut super::GameplayActionOutcome,
    ) {
        let taken = match slot {
            PLAYER_MAIN_START..=PLAYER_MAIN_END => {
                inventory.take_main_slot((slot - PLAYER_MAIN_START) as usize)
            }
            PLAYER_HOTBAR_START..=PLAYER_HOTBAR_END => {
                inventory.take_hotbar_slot((slot - PLAYER_HOTBAR_START) as usize)
            }
            _ => None,
        };
        let Some((item, change)) = taken else {
            return;
        };
        outcome.inventory_changes.push(change);
        if same_resource_key(&items::item_name(&item), &config.enchanting.lapis_item) {
            if self.lapis.item_count.0 > 0
                && let Some(mut changes) = inventory.add_item_stack(&self.lapis)
            {
                outcome.inventory_changes.append(&mut changes);
            }
            self.lapis = item;
            return;
        }
        if self.input.item_count.0 > 0
            && let Some(mut changes) = inventory.add_item_stack(&self.input)
        {
            outcome.inventory_changes.append(&mut changes);
        }
        self.input = item;
    }

    pub(in crate::play) fn recompute(
        &mut self,
        config: &RuntimeConfig,
        player: &crate::players::OnlinePlayer,
        plugins: &crate::plugins::PluginManager,
        world: &crate::world::WorldManager,
        world_dimension: &str,
    ) {
        if !config.enchanting.enable
            || self.input.item_count.0 <= 0
            || !input_allows_enchanting(&self.input, &config.enchanting)
        {
            self.offers.clear();
            self.selected_offer = None;
            return;
        }
        let player_level = current_player_level(player);
        let bookshelf_count = bookshelf_count(world, world_dimension, self.table_position.as_ref());
        let mut offers = collect_config_offers(
            &config.enchanting,
            &self.input,
            player_level,
            bookshelf_count,
        );
        let query = EnchantingQuery {
            player: qexed_plugin_api::player_payload_owned(player),
            item: items::item_stack_payload(&self.input),
            lapis_item: (self.lapis.item_count.0 > 0)
                .then(|| items::item_stack_payload(&self.lapis)),
            level: player_level,
            bookshelf_count,
            seed: self.seed,
        };
        if let Some(response) = plugins.apply_enchanting_options(query) {
            if response.replace {
                offers.clear();
            }
            offers.extend(response.options.into_iter().filter_map(|option| {
                EnchantingOffer::from_plugin(option, &self.input, player_level, bookshelf_count)
            }));
        }
        offers.retain(|offer| offer.is_available(&self.input, player_level, bookshelf_count));
        self.offers = select_visible_offers(offers, self.seed, 3);
        self.selected_offer = self
            .selected_offer
            .filter(|index| *index < self.offers.len());
    }

    async fn sync<W>(
        &self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        player_slots: Vec<Slot>,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        sink.send(ContainerSetContent {
            window_id: VarInt(ENCHANTING_WINDOW_ID),
            state_id: VarInt(self.state_id),
            slot_data: self.menu_slots(player_slots),
            carried_item: crate::inventory::empty_slot(),
        })
        .await?;
        for index in 0..3 {
            let value = self
                .offers
                .get(index)
                .map(|offer| offer_cost(offer, index))
                .unwrap_or(0);
            sink.send(ContainerSetData {
                window_id: VarInt(ENCHANTING_WINDOW_ID),
                property: index as i16,
                value,
            })
            .await?;
        }
        sink.send(ContainerSetData {
            window_id: VarInt(ENCHANTING_WINDOW_ID),
            property: 3,
            value: clamp_i16(self.seed as i32),
        })
        .await?;
        for index in 0..3 {
            let value = self
                .offers
                .get(index)
                .and_then(offer_hint_enchantment_id)
                .map(clamp_i16)
                .unwrap_or(-1);
            sink.send(ContainerSetData {
                window_id: VarInt(ENCHANTING_WINDOW_ID),
                property: 4 + index as i16,
                value,
            })
            .await?;
        }
        for index in 0..3 {
            let value = self
                .offers
                .get(index)
                .map(|offer| clamp_i16(offer.level.max(1)))
                .unwrap_or(-1);
            sink.send(ContainerSetData {
                window_id: VarInt(ENCHANTING_WINDOW_ID),
                property: 7 + index as i16,
                value,
            })
            .await?;
        }
        sink.send(SetExperience {
            experience_progress: 0.0,
            experience_level: VarInt(current_player_level_raw()),
            total_experience: VarInt(current_player_level_raw()),
        })
        .await?;
        Ok(())
    }

    async fn sync_input_slot<W>(&self, sink: &mut qexed_tcp_connect::PacketSink<W>) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        sink.send(container_set_slot::ContainerSetContent {
            window_id: VarInt(ENCHANTING_WINDOW_ID),
            state_id: VarInt(self.state_id),
            slot: INPUT_SLOT,
            slot_data: self.input.clone(),
        })
        .await?;
        Ok(())
    }

    fn menu_slots(&self, player_slots: Vec<Slot>) -> Vec<Slot> {
        let mut slots = Vec::with_capacity(38);
        slots.push(self.input.clone());
        slots.push(self.lapis.clone());
        slots.extend(player_slots);
        slots.resize_with(38, crate::inventory::empty_slot);
        slots
    }

    fn can_apply_offer(&self, offer: &EnchantingOffer, config: &RuntimeConfig) -> bool {
        config.enchanting.enable
            && self.input.item_count.0 > 0
            && input_allows_enchanting(&self.input, &config.enchanting)
            && (config.enchanting.creative_free
                || self.lapis.item_count.0 >= offer.lapis_cost.max(0))
            && current_player_level_raw() >= offer.required_level
    }

    fn consume_cost(
        &mut self,
        offer: &EnchantingOffer,
        config: &RuntimeConfig,
        inventory: &mut crate::inventory::PlayerInventory,
    ) -> bool {
        if config.enchanting.creative_free {
            return true;
        }
        if offer.lapis_cost > 0 {
            if self.lapis.item_count.0 < offer.lapis_cost {
                return false;
            }
            items::decrement_slot(&mut self.lapis, offer.lapis_cost);
        }
        let _ = inventory;
        true
    }

    fn apply_enchantments(
        &self,
        offer: &EnchantingOffer,
        player: &crate::players::OnlinePlayer,
        plugins: &crate::plugins::PluginManager,
        config: &RuntimeConfig,
        world_dimension: &str,
    ) -> Result<Slot> {
        let mut item = self.input.clone();
        let mut components = item.components_to_add.take().unwrap_or_default();
        for enchantment in offer.applied_enchantments() {
            if enchantment.plugin {
                upsert_plugin_enchantment(&mut components, &enchantment.id, enchantment.level);
                upsert_glint_override(&mut components);
            } else if let Some(id) = vanilla_enchantment_id(&enchantment.id) {
                upsert_vanilla_enchantment(
                    &mut components,
                    &items::item_name(&item),
                    id,
                    enchantment.level,
                );
            }
        }
        if !components.is_empty() {
            item.number_of_components_to_add = Some(VarInt(components.len() as i32));
            item.components_to_add = Some(components);
        }
        let _ = (player, plugins, config, world_dimension);
        Ok(item)
    }
}

fn upsert_vanilla_enchantment(
    components: &mut Vec<ComponentsToAdd>,
    item_name: &str,
    id: i32,
    level: i32,
) {
    let value = minecraft::Enchantment {
        enchantment: VarInt(id),
        level: VarInt(level.max(1)),
    };
    if is_enchanted_book_item(item_name) {
        upsert_stored_enchantment(components, value);
    } else {
        upsert_direct_enchantment(components, value);
    }
}

fn upsert_direct_enchantment(components: &mut Vec<ComponentsToAdd>, value: minecraft::Enchantment) {
    if let Some(ComponentsToAdd::MinecraftEnchantments(enchantments)) = components
        .iter_mut()
        .find(|component| matches!(component, ComponentsToAdd::MinecraftEnchantments(_)))
    {
        if let Some(existing) = enchantments
            .enchantments
            .iter_mut()
            .find(|enchantment| enchantment.enchantment.0 == value.enchantment.0)
        {
            existing.level.0 = existing.level.0.max(value.level.0);
        } else {
            enchantments.enchantments.push(value);
        }
        return;
    }
    components.push(ComponentsToAdd::MinecraftEnchantments(
        minecraft::Enchantments {
            enchantments: vec![value],
        },
    ));
}

fn upsert_stored_enchantment(components: &mut Vec<ComponentsToAdd>, value: minecraft::Enchantment) {
    if let Some(ComponentsToAdd::MinecraftStoredEnchantments(enchantments)) = components
        .iter_mut()
        .find(|component| matches!(component, ComponentsToAdd::MinecraftStoredEnchantments(_)))
    {
        if let Some(existing) = enchantments
            .enchantments
            .iter_mut()
            .find(|enchantment| enchantment.enchantment.0 == value.enchantment.0)
        {
            existing.level.0 = existing.level.0.max(value.level.0);
        } else {
            enchantments.enchantments.push(value);
        }
        return;
    }
    components.push(ComponentsToAdd::MinecraftStoredEnchantments(
        minecraft::StoredEnchantments {
            enchantments: vec![value],
        },
    ));
}

fn upsert_plugin_enchantment(components: &mut Vec<ComponentsToAdd>, id: &str, level: i32) {
    let id = id.trim();
    if id.is_empty() {
        return;
    }
    let mut plugin_enchantments =
        HashMap::from([(id.to_string(), qexed_nbt::Tag::Int(level.max(1)))]);
    for component in components.iter_mut() {
        let ComponentsToAdd::MinecraftCustomData(custom_data) = component else {
            continue;
        };
        let qexed_nbt::Tag::Compound(root) = &custom_data.data else {
            continue;
        };
        let mut next_root = (**root).clone();
        if let Some(qexed_nbt::Tag::Compound(existing)) = next_root.get("qexed:enchantments") {
            plugin_enchantments.extend((**existing).clone());
        }
        plugin_enchantments.insert(id.to_string(), qexed_nbt::Tag::Int(level.max(1)));
        next_root.insert(
            "qexed:enchantments".to_string(),
            qexed_nbt::Tag::Compound(std::sync::Arc::new(plugin_enchantments)),
        );
        custom_data.data = qexed_nbt::Tag::Compound(std::sync::Arc::new(next_root));
        return;
    }

    let mut root = HashMap::new();
    root.insert(
        "qexed:enchantments".to_string(),
        qexed_nbt::Tag::Compound(std::sync::Arc::new(plugin_enchantments)),
    );
    components.push(ComponentsToAdd::MinecraftCustomData(
        minecraft::CustomData {
            data: qexed_nbt::Tag::Compound(std::sync::Arc::new(root)),
        },
    ));
}

fn upsert_glint_override(components: &mut Vec<ComponentsToAdd>) {
    if let Some(ComponentsToAdd::MinecraftEnchantmentGlintOverride(glint)) =
        components.iter_mut().find(|component| {
            matches!(
                component,
                ComponentsToAdd::MinecraftEnchantmentGlintOverride(_)
            )
        })
    {
        glint.has_glint = true;
        return;
    }
    components.push(ComponentsToAdd::MinecraftEnchantmentGlintOverride(
        minecraft::EnchantmentGlintOverride { has_glint: true },
    ));
}

fn input_allows_enchanting(
    item: &Slot,
    config: &qexed_config::app::qexed_enchanting::EnchantingConfig,
) -> bool {
    config.allow_reenchanting || !item_has_existing_enchantments(item)
}

fn item_has_existing_enchantments(item: &Slot) -> bool {
    !items::enchantments(item).is_empty() || !items::plugin_enchantments(item).is_empty()
}

#[derive(Debug, Clone)]
struct EnchantingOffer {
    id: String,
    level: i32,
    weight: i32,
    required_level: i32,
    lapis_cost: i32,
    max_player_level: i32,
    min_bookshelves: i32,
    hidden: bool,
    plugin: bool,
    item_suffixes: Vec<String>,
    items: Vec<String>,
    additional_enchantments: Vec<AppliedEnchantment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AppliedEnchantment {
    id: String,
    level: i32,
    plugin: bool,
}

impl EnchantingOffer {
    fn from_plugin(
        option: EnchantingOption,
        item: &Slot,
        player_level: i32,
        bookshelf_count: i32,
    ) -> Option<Self> {
        Some(Self {
            id: option.id,
            level: option.level.max(1),
            weight: option.weight.max(1),
            required_level: option.required_level.max(0),
            lapis_cost: option.lapis_cost.max(0),
            max_player_level: i32::MAX,
            min_bookshelves: 0,
            hidden: option.hidden,
            plugin: true,
            item_suffixes: Vec::new(),
            items: Vec::new(),
            additional_enchantments: Vec::new(),
        })
        .filter(|offer| offer.is_available(item, player_level, bookshelf_count))
    }

    fn is_available(&self, item: &Slot, player_level: i32, bookshelf_count: i32) -> bool {
        if self.hidden {
            return false;
        }
        if player_level < self.required_level {
            return false;
        }
        if player_level > self.max_player_level {
            return false;
        }
        if bookshelf_count < self.min_bookshelves {
            return false;
        }
        if !self.item_matches(item) {
            return false;
        }
        true
    }

    fn item_matches(&self, item: &Slot) -> bool {
        let name = items::item_name(item);
        if is_enchantable_book_item(&name) {
            return true;
        }
        let configured_match = self.items.iter().any(|candidate| candidate == &name)
            || self
                .item_suffixes
                .iter()
                .any(|suffix| name.ends_with(suffix));
        if !configured_match {
            return false;
        }
        self.plugin || vanilla_enchantment_supports_item(&self.id, &name)
    }

    fn applied_enchantments(&self) -> Vec<AppliedEnchantment> {
        let mut enchantments = Vec::with_capacity(1 + self.additional_enchantments.len());
        enchantments.push(self.primary_enchantment());
        enchantments.extend(self.additional_enchantments.iter().cloned());
        enchantments
    }

    fn primary_enchantment(&self) -> AppliedEnchantment {
        AppliedEnchantment {
            id: self.id.clone(),
            level: self.level,
            plugin: self.plugin,
        }
    }
}

fn vanilla_enchantment_supports_item(enchantment_id: &str, item_name: &str) -> bool {
    let enchantment_id = normalize_resource_key(enchantment_id);
    let item_name = normalize_resource_key(item_name);
    vanilla_enchantment_supported_items()
        .get(&enchantment_id)
        .is_none_or(|supported_items| supported_items.contains(&item_name))
}

fn vanilla_enchantment_supported_items() -> &'static HashMap<String, HashSet<String>> {
    static SUPPORTED_ITEMS: OnceLock<HashMap<String, HashSet<String>>> = OnceLock::new();
    SUPPORTED_ITEMS.get_or_init(load_vanilla_enchantment_supported_items)
}

fn load_vanilla_enchantment_supported_items() -> HashMap<String, HashSet<String>> {
    let item_tags = load_item_tag_names();
    let Ok(registries) = crate::registry_sync::load_registry_packets(true) else {
        return HashMap::new();
    };
    let Some(enchantments) = registries
        .into_iter()
        .find(|registry| registry.id == "minecraft:enchantment")
    else {
        return HashMap::new();
    };

    enchantments
        .entries
        .into_iter()
        .filter_map(|entry| {
            let supported_items =
                supported_items_from_enchantment_data(entry.data.as_ref()?, &item_tags)?;
            Some((normalize_resource_key(&entry.entry_id), supported_items))
        })
        .collect()
}

fn load_item_tag_names() -> HashMap<String, HashSet<String>> {
    let Ok(packet) = crate::registry_sync::load_tag_packet() else {
        return HashMap::new();
    };
    let Some(item_tags) = packet
        .tags
        .into_iter()
        .find(|tags| tags.registry == "minecraft:item")
    else {
        return HashMap::new();
    };

    item_tags
        .tags
        .into_iter()
        .map(|tag| {
            let entries = tag
                .entries
                .into_iter()
                .filter_map(|entry| crate::inventory::item_name_for_id(entry.0))
                .collect();
            (normalize_resource_key(&tag.name), entries)
        })
        .collect()
}

fn supported_items_from_enchantment_data(
    data: &qexed_nbt::Tag,
    item_tags: &HashMap<String, HashSet<String>>,
) -> Option<HashSet<String>> {
    let qexed_nbt::Tag::Compound(root) = data else {
        return None;
    };
    let mut supported_items = HashSet::new();
    collect_supported_item_names(
        root.get("supported_items")?,
        item_tags,
        &mut supported_items,
    );
    (!supported_items.is_empty()).then_some(supported_items)
}

fn collect_supported_item_names(
    tag: &qexed_nbt::Tag,
    item_tags: &HashMap<String, HashSet<String>>,
    out: &mut HashSet<String>,
) {
    match tag {
        qexed_nbt::Tag::String(value) => extend_supported_item_name(value, item_tags, out),
        qexed_nbt::Tag::List(_, values) => {
            for value in values.iter() {
                collect_supported_item_names(value, item_tags, out);
            }
        }
        qexed_nbt::Tag::Compound(value) => {
            if let Some(values) = value.get("values") {
                collect_supported_item_names(values, item_tags, out);
            } else if let Some(id) = value.get("id") {
                collect_supported_item_names(id, item_tags, out);
            }
        }
        _ => {}
    }
}

fn extend_supported_item_name(
    value: &str,
    item_tags: &HashMap<String, HashSet<String>>,
    out: &mut HashSet<String>,
) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    if let Some(tag_name) = value.strip_prefix('#') {
        if let Some(entries) = item_tags.get(&normalize_resource_key(tag_name)) {
            out.extend(entries.iter().cloned());
        }
    } else {
        out.insert(normalize_resource_key(value));
    }
}

fn collect_config_offers(
    config: &qexed_config::app::qexed_enchanting::EnchantingConfig,
    item: &Slot,
    player_level: i32,
    bookshelf_count: i32,
) -> Vec<EnchantingOffer> {
    config
        .options
        .iter()
        .filter_map(|option| {
            let min_bookshelves = option.min_bookshelves.max(0);
            let offer = EnchantingOffer {
                id: option.id.clone(),
                level: scaled_enchantment_level(option.max_level, min_bookshelves, bookshelf_count),
                weight: option.weight.max(1),
                required_level: scaled_offer_required_level(
                    option.min_player_level,
                    min_bookshelves,
                    bookshelf_count,
                ),
                lapis_cost: scaled_lapis_cost(option.lapis_cost, min_bookshelves, bookshelf_count),
                max_player_level: option.max_player_level.max(option.min_player_level),
                min_bookshelves,
                hidden: false,
                plugin: option.plugin,
                item_suffixes: option.item_suffixes.clone(),
                items: option.items.clone(),
                additional_enchantments: Vec::new(),
            };
            offer
                .is_available(item, player_level, bookshelf_count)
                .then_some(offer)
        })
        .collect()
}

fn select_visible_offers(
    mut offers: Vec<EnchantingOffer>,
    seed: u64,
    limit: usize,
) -> Vec<EnchantingOffer> {
    let candidates = offers.clone();
    let mut visible = Vec::new();
    let mut fallback = Vec::new();
    for offer in offers.drain(..) {
        if offer_hint_enchantment_id(&offer).is_some() {
            visible.push(offer);
        } else {
            fallback.push(offer);
        }
    }

    let mut selected = weighted_sample_without_replacement(visible, seed, limit);
    if selected.len() < limit {
        selected.extend(weighted_sample_without_replacement(
            fallback,
            seed ^ 0x9e37_79b9_7f4a_7c15,
            limit - selected.len(),
        ));
    }
    selected.sort_by(|left, right| {
        left.required_level
            .cmp(&right.required_level)
            .then_with(|| left.id.cmp(&right.id))
    });
    populate_additional_enchantments(&mut selected, &candidates, seed);
    selected
}

fn populate_additional_enchantments(
    offers: &mut [EnchantingOffer],
    candidates: &[EnchantingOffer],
    seed: u64,
) {
    for (index, offer) in offers.iter_mut().enumerate() {
        offer.additional_enchantments = select_additional_enchantments(
            offer,
            candidates,
            seed ^ additional_enchantment_seed(&offer.id, index),
        );
    }
}

fn select_additional_enchantments(
    primary: &EnchantingOffer,
    candidates: &[EnchantingOffer],
    seed: u64,
) -> Vec<AppliedEnchantment> {
    if primary.plugin {
        return Vec::new();
    }

    let mut rng = StdRng::seed_from_u64(seed);
    let mut power = primary.required_level.max(primary.level * 4).max(1);
    let mut selected = vec![primary.primary_enchantment()];
    let mut additional = Vec::new();

    while additional.len() < MAX_ADDITIONAL_ENCHANTMENTS
        && power > 0
        && rng.gen_range(0..50) <= power
    {
        let compatible = candidates
            .iter()
            .filter(|candidate| additional_candidate_matches(primary, candidate, &selected))
            .collect::<Vec<_>>();
        let Some(candidate) = weighted_pick(&compatible, &mut rng) else {
            break;
        };

        let enchantment = candidate.primary_enchantment();
        selected.push(enchantment.clone());
        additional.push(enchantment);
        power /= 2;
    }

    additional
}

fn additional_candidate_matches(
    primary: &EnchantingOffer,
    candidate: &EnchantingOffer,
    selected: &[AppliedEnchantment],
) -> bool {
    !candidate.plugin
        && candidate.required_level <= primary.required_level
        && selected.iter().all(|enchantment| {
            enchantments_are_compatible(
                &candidate.id,
                candidate.plugin,
                &enchantment.id,
                enchantment.plugin,
            )
        })
}

fn weighted_pick<'a>(
    offers: &[&'a EnchantingOffer],
    rng: &mut StdRng,
) -> Option<&'a EnchantingOffer> {
    let total_weight: i32 = offers.iter().map(|offer| offer.weight.max(1)).sum();
    if total_weight <= 0 {
        return None;
    }
    let mut pick = rng.gen_range(0..total_weight);
    for offer in offers {
        pick -= offer.weight.max(1);
        if pick < 0 {
            return Some(*offer);
        }
    }
    None
}

fn enchantments_are_compatible(
    left_id: &str,
    left_plugin: bool,
    right_id: &str,
    right_plugin: bool,
) -> bool {
    if same_resource_key(left_id, right_id) {
        return false;
    }
    if left_plugin || right_plugin {
        return true;
    }
    !vanilla_enchantments_conflict(left_id, right_id)
}

fn vanilla_enchantments_conflict(left_id: &str, right_id: &str) -> bool {
    let left_id = normalize_resource_key(left_id);
    let right_id = normalize_resource_key(right_id);
    VANILLA_EXCLUSIVE_SETS
        .iter()
        .any(|set| set.iter().any(|id| *id == left_id) && set.iter().any(|id| *id == right_id))
}

fn additional_enchantment_seed(id: &str, index: usize) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64 ^ index as u64;
    for byte in id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}

fn weighted_sample_without_replacement(
    mut offers: Vec<EnchantingOffer>,
    seed: u64,
    limit: usize,
) -> Vec<EnchantingOffer> {
    let mut selected = Vec::new();
    let mut rng = StdRng::seed_from_u64(seed);
    while selected.len() < limit && !offers.is_empty() {
        let total_weight: i32 = offers.iter().map(|offer| offer.weight.max(1)).sum();
        if total_weight <= 0 {
            break;
        }
        let mut pick = rng.gen_range(0..total_weight);
        let mut index = 0;
        for (candidate_index, offer) in offers.iter().enumerate() {
            pick -= offer.weight.max(1);
            if pick < 0 {
                index = candidate_index;
                break;
            }
        }
        selected.push(offers.swap_remove(index));
    }
    selected
}

fn scaled_enchantment_level(max_level: i32, min_bookshelves: i32, bookshelf_count: i32) -> i32 {
    let max_level = max_level.max(1);
    let available = bookshelf_count.clamp(min_bookshelves.max(0), 15);
    let unlocked = (available - min_bookshelves.max(0)).max(0);
    let span = (15 - min_bookshelves.max(0)).max(1);
    let scaled = 1 + ((max_level - 1) * unlocked) / span;
    scaled.clamp(1, max_level)
}

fn scaled_offer_required_level(
    min_player_level: i32,
    min_bookshelves: i32,
    bookshelf_count: i32,
) -> i32 {
    let base = min_player_level.clamp(0, 30);
    let available = bookshelf_count.clamp(min_bookshelves.max(0), 15);
    let unlocked = (available - min_bookshelves.max(0)).max(0);
    let span = (15 - min_bookshelves.max(0)).max(1);
    let scaled = 1 + ((30 - base.max(1)) * unlocked) / span;
    base.max(scaled).clamp(0, 30)
}

fn scaled_lapis_cost(lapis_cost: i32, min_bookshelves: i32, bookshelf_count: i32) -> i32 {
    let configured = lapis_cost.max(0);
    if configured == 0 {
        return 0;
    }
    let available = bookshelf_count.clamp(min_bookshelves.max(0), 15);
    match available {
        0..=4 => configured.min(1),
        5..=9 => configured.min(2),
        _ => configured,
    }
}

fn current_player_level(_player: &crate::players::OnlinePlayer) -> i32 {
    current_player_level_raw()
}

fn current_player_level_raw() -> i32 {
    30
}

fn current_seed() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn offer_cost(offer: &EnchantingOffer, index: usize) -> i16 {
    clamp_i16(offer.required_level.max((index + 1) as i32))
}

fn offer_hint_enchantment_id(offer: &EnchantingOffer) -> Option<i32> {
    if offer.plugin {
        return None;
    }
    vanilla_enchantment_id(&offer.id)
}

fn clamp_i16(value: i32) -> i16 {
    value.min(i16::MAX as i32).max(i16::MIN as i32) as i16
}

fn bookshelf_count(
    world: &crate::world::WorldManager,
    dimension: &str,
    table_position: Option<&BlockPosition>,
) -> i32 {
    let Some(table) = table_position else {
        return 0;
    };
    let mut count = 0;
    for dx in -2i32..=2 {
        for dz in -2i32..=2 {
            if dx.abs() != 2 && dz.abs() != 2 {
                continue;
            }
            for dy in 0..=1 {
                if !is_enchantment_power_transmitter(
                    world,
                    dimension,
                    &BlockPosition {
                        x: table.x + dx / 2,
                        y: table.y + dy,
                        z: table.z + dz / 2,
                    },
                ) {
                    continue;
                };
                if is_enchantment_power_provider(
                    world,
                    dimension,
                    &BlockPosition {
                        x: table.x + dx,
                        y: table.y + dy,
                        z: table.z + dz,
                    },
                ) {
                    count += 1;
                }
            }
        }
    }
    count
}

fn is_enchantment_power_provider(
    world: &crate::world::WorldManager,
    dimension: &str,
    position: &BlockPosition,
) -> bool {
    block_name_at(world, dimension, position).is_some_and(|name| name == "minecraft:bookshelf")
}

fn is_enchantment_power_transmitter(
    world: &crate::world::WorldManager,
    dimension: &str,
    position: &BlockPosition,
) -> bool {
    world
        .block_state_at(dimension, position)
        .is_none_or(crate::inventory::is_air_block_state)
}

fn block_name_at(
    world: &crate::world::WorldManager,
    dimension: &str,
    position: &BlockPosition,
) -> Option<String> {
    let block_state = world.block_state_at(dimension, position)?;
    Some(
        crate::inventory::block_name_for_state(block_state)
            .unwrap_or_else(|| crate::world::chunk_nbt::block_state_entry(block_state).name),
    )
}

fn same_resource_key(left: &str, right: &str) -> bool {
    normalize_resource_key(left) == normalize_resource_key(right)
}

fn player_inventory_slots(inventory: &crate::inventory::PlayerInventory) -> Vec<Slot> {
    let mut slots = Vec::with_capacity(36);
    slots.extend(inventory.main_items().iter().cloned());
    slots.extend(inventory.hotbar_items().iter().cloned());
    slots
}

fn normalize_resource_key(value: &str) -> String {
    let value = value.trim();
    if value.contains(':') {
        value.to_string()
    } else {
        format!("minecraft:{value}")
    }
}

fn is_enchanted_book_item(item_name: &str) -> bool {
    matches!(
        normalize_resource_key(item_name).as_str(),
        "minecraft:book" | "minecraft:enchanted_book"
    )
}

fn is_enchantable_book_item(item_name: &str) -> bool {
    normalize_resource_key(item_name) == "minecraft:book"
}

fn menu_id(name: &str) -> i32 {
    crate::registry_sync::load_registry_id_map("minecraft:menu")
        .ok()
        .and_then(|map| map.get(name).copied())
        .unwrap_or_else(|| match name {
            "minecraft:enchantment" => 17,
            _ => 0,
        })
}

fn vanilla_enchantment_id(value: &str) -> Option<i32> {
    let key = if value.contains(':') {
        value.to_string()
    } else {
        format!("minecraft:{value}")
    };
    static IDS: OnceLock<HashMap<String, i32>> = OnceLock::new();
    IDS.get_or_init(|| {
        crate::registry_sync::load_dynamic_registry_id_map(
            std::path::Path::new("__qexed_missing_dynamic_registry__"),
            "enchantment",
        )
        .unwrap_or_default()
    })
    .get(&key)
    .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIMENSION: &str = "minecraft:overworld";

    #[test]
    fn bookshelf_count_requires_clear_transmitter_space() {
        let temp = tempfile::tempdir().unwrap();
        let world = crate::world::WorldManager::new(temp.path().join("world"));
        let table = BlockPosition { x: 0, y: 64, z: 0 };
        let bookshelf = crate::world::chunk_nbt::default_block_state_id("minecraft:bookshelf");
        let stone = crate::world::chunk_nbt::default_block_state_id("minecraft:stone");

        world.set_runtime_block(DIMENSION, BlockPosition { x: 2, y: 64, z: 0 }, bookshelf);
        world.set_runtime_block(DIMENSION, BlockPosition { x: -2, y: 64, z: 0 }, bookshelf);
        world.set_runtime_block(DIMENSION, BlockPosition { x: -1, y: 64, z: 0 }, stone);

        assert_eq!(bookshelf_count(&world, DIMENSION, Some(&table)), 1);
    }

    #[test]
    fn config_offer_level_scales_with_bookshelves() {
        let item_id = crate::inventory::item_id_for_name("minecraft:diamond_pickaxe").unwrap();
        let item = crate::inventory::simple_item(item_id, 1);
        let mut config = qexed_config::app::qexed_enchanting::EnchantingConfig::default();
        config.options = vec![
            qexed_config::app::qexed_enchanting::EnchantingOptionConfig {
                id: "minecraft:efficiency".to_string(),
                max_level: 5,
                min_player_level: 1,
                item_suffixes: vec!["_pickaxe".to_string()],
                ..Default::default()
            },
        ];

        let low = collect_config_offers(&config, &item, 30, 0);
        let high = collect_config_offers(&config, &item, 30, 15);

        assert_eq!(low[0].level, 1);
        assert_eq!(high[0].level, 5);
        assert!(high[0].required_level > low[0].required_level);
    }

    #[test]
    fn min_bookshelves_still_filters_locked_options() {
        let item_id = crate::inventory::item_id_for_name("minecraft:diamond_pickaxe").unwrap();
        let item = crate::inventory::simple_item(item_id, 1);
        let mut config = qexed_config::app::qexed_enchanting::EnchantingConfig::default();
        config.options = vec![
            qexed_config::app::qexed_enchanting::EnchantingOptionConfig {
                id: "minecraft:fortune".to_string(),
                max_level: 3,
                min_player_level: 1,
                min_bookshelves: 8,
                item_suffixes: vec!["_pickaxe".to_string()],
                ..Default::default()
            },
        ];

        assert!(collect_config_offers(&config, &item, 30, 7).is_empty());
        assert_eq!(collect_config_offers(&config, &item, 30, 8)[0].level, 1);
    }

    #[test]
    fn existing_enchantments_respect_reenchanting_config() {
        let item_id = crate::inventory::item_id_for_name("minecraft:diamond_pickaxe").unwrap();
        let mut item = crate::inventory::simple_item(item_id, 1);
        item.components_to_add = Some(vec![ComponentsToAdd::MinecraftEnchantments(
            minecraft::Enchantments {
                enchantments: vec![minecraft::Enchantment {
                    enchantment: VarInt(8),
                    level: VarInt(1),
                }],
            },
        )]);
        let mut config = qexed_config::app::qexed_enchanting::EnchantingConfig::default();

        assert!(!input_allows_enchanting(&item, &config));

        config.allow_reenchanting = true;

        assert!(input_allows_enchanting(&item, &config));
    }

    #[test]
    fn vanilla_supported_items_block_misconfigured_crossbow_enchantment_on_pickaxe() {
        let item_id = crate::inventory::item_id_for_name("minecraft:diamond_pickaxe").unwrap();
        let item = crate::inventory::simple_item(item_id, 1);
        let mut config = qexed_config::app::qexed_enchanting::EnchantingConfig::default();
        config.options = vec![
            qexed_config::app::qexed_enchanting::EnchantingOptionConfig {
                id: "minecraft:quick_charge".to_string(),
                max_level: 3,
                min_player_level: 1,
                item_suffixes: vec!["_pickaxe".to_string()],
                items: vec!["minecraft:crossbow".to_string()],
                ..Default::default()
            },
            qexed_config::app::qexed_enchanting::EnchantingOptionConfig {
                id: "minecraft:efficiency".to_string(),
                max_level: 5,
                min_player_level: 1,
                item_suffixes: vec!["_pickaxe".to_string()],
                ..Default::default()
            },
        ];

        let offers = collect_config_offers(&config, &item, 30, 15);

        assert!(
            offers
                .iter()
                .any(|offer| offer.id == "minecraft:efficiency")
        );
        assert!(
            offers
                .iter()
                .all(|offer| offer.id != "minecraft:quick_charge")
        );
    }

    #[test]
    fn visible_offer_selection_prefers_previewable_vanilla_pool() {
        let offers = vec![
            test_offer("minecraft:efficiency", 15, false),
            test_offer("minecraft:fortune", 8, false),
            test_offer("minecraft:unbreaking", 10, false),
            test_offer("qexed:vein_miner", 100, true),
        ];

        let selected = select_visible_offers(offers, 42, 3);

        assert_eq!(selected.len(), 3);
        assert!(selected.iter().all(|offer| !offer.plugin));
        assert!(
            selected
                .iter()
                .all(|offer| offer_hint_enchantment_id(offer).is_some())
        );
    }

    #[test]
    fn high_level_vanilla_offer_can_include_additional_enchantments() {
        let primary = test_offer("minecraft:efficiency", 10, false).with_required_level(30);
        let candidates = vec![
            primary.clone(),
            test_offer("minecraft:fortune", 8, false).with_required_level(30),
            test_offer("minecraft:unbreaking", 5, false).with_required_level(30),
        ];
        let seed = (0..500)
            .find(|seed| !select_additional_enchantments(&primary, &candidates, *seed).is_empty())
            .expect("high level seed should produce at least one additional enchantment");

        let additional = select_additional_enchantments(&primary, &candidates, seed);

        assert!(!additional.is_empty());
        assert!(additional.iter().all(|enchantment| {
            enchantments_are_compatible(
                &primary.id,
                primary.plugin,
                &enchantment.id,
                enchantment.plugin,
            )
        }));
    }

    #[test]
    fn additional_enchantments_skip_vanilla_exclusive_set_conflicts() {
        let primary = test_offer("minecraft:fortune", 10, false).with_required_level(30);
        let candidates = vec![
            primary.clone(),
            test_offer("minecraft:silk_touch", 10, false).with_required_level(30),
            test_offer("minecraft:efficiency", 10, false).with_required_level(30),
        ];

        for seed in 0..100 {
            let additional = select_additional_enchantments(&primary, &candidates, seed);
            assert!(
                additional
                    .iter()
                    .all(|enchantment| enchantment.id != "minecraft:silk_touch")
            );
        }
    }

    fn test_offer(id: &str, weight: i32, plugin: bool) -> EnchantingOffer {
        EnchantingOffer {
            id: id.to_string(),
            level: 1,
            weight,
            required_level: 1,
            lapis_cost: 0,
            max_player_level: 30,
            min_bookshelves: 0,
            hidden: false,
            plugin,
            item_suffixes: Vec::new(),
            items: Vec::new(),
            additional_enchantments: Vec::new(),
        }
    }

    trait TestOfferExt {
        fn with_required_level(self, required_level: i32) -> Self;
    }

    impl TestOfferExt for EnchantingOffer {
        fn with_required_level(mut self, required_level: i32) -> Self {
            self.required_level = required_level;
            self
        }
    }
}
