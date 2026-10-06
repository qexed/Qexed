use std::sync::OnceLock;

use crate::error::Result;

use qexed_packet::net_types::VarInt;
use qexed_protocol::{
    to_client::play::{
        container_set_content::ContainerSetContent, container_set_data::ContainerSetData,
        open_screen::OpenScreen,
    },
    to_server::play::container_click::ContainerClick,
    types::Slot,
};
use serde::Deserialize;

use super::{GameplayActionOutcome, items};
use crate::util::text_component;

pub(in crate::gameplay) const FURNACE_WINDOW_ID: i32 = 22;
const INPUT_SLOT: i16 = 0;
const FUEL_SLOT: i16 = 1;
const OUTPUT_SLOT: i16 = 2;

#[derive(Debug, Default)]
pub(in crate::gameplay) struct FurnaceRuntime {
    open: bool,
    state_id: i32,
    input: Slot,
    fuel: Slot,
    output: Slot,
    burn_time: i32,
    burn_time_total: i32,
    cook_time: i32,
    cook_time_total: i32,
    active_recipe: Option<FurnaceRecipe>,
}

impl FurnaceRuntime {
    pub(super) fn new() -> Self {
        Self {
            input: crate::inventory::empty_slot(),
            fuel: crate::inventory::empty_slot(),
            output: crate::inventory::empty_slot(),
            ..Self::default()
        }
    }

    pub(super) fn is_open(&self) -> bool {
        self.open
    }

    pub(in crate::gameplay) async fn open<W>(
        &mut self,
        sink: &mut qexed_connection::transport::PacketSink<W>,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        self.open = true;
        self.state_id = self.state_id.wrapping_add(1);
        sink.send(OpenScreen {
            window_id: VarInt(FURNACE_WINDOW_ID),
            menu_type: VarInt(menu_id("minecraft:furnace")),
            title: text_component("Furnace"),
        })
        .await?;
        self.sync(sink).await
    }

    pub(in crate::gameplay) async fn close<W>(
        &mut self,
        sink: &mut qexed_connection::transport::PacketSink<W>,
        inventory: &mut crate::inventory::PlayerInventory,
    ) -> Result<GameplayActionOutcome>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if !self.open {
            return Ok(GameplayActionOutcome::default());
        }
        self.open = false;
        let mut outcome = GameplayActionOutcome::handled();
        for item in [&mut self.input, &mut self.fuel, &mut self.output] {
            if item.item_count.0 <= 0 {
                continue;
            }
            if let Some(mut changes) = inventory.add_item_stack(item) {
                outcome.inventory_changes.append(&mut changes);
                *item = crate::inventory::empty_slot();
            }
        }
        sink.send(
            qexed_protocol::to_client::play::container_close::ContainerClose {
                window_id: VarInt(FURNACE_WINDOW_ID),
            },
        )
        .await?;
        outcome.close_window = true;
        Ok(outcome)
    }

    pub(in crate::gameplay) async fn handle_click<W>(
        &mut self,
        sink: &mut qexed_connection::transport::PacketSink<W>,
        click: &ContainerClick,
        inventory: &mut crate::inventory::PlayerInventory,
    ) -> Result<Option<GameplayActionOutcome>>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if !self.open || click.window_id.0 != FURNACE_WINDOW_ID {
            return Ok(None);
        }
        let mut outcome = GameplayActionOutcome::handled();
        match click.slot {
            INPUT_SLOT | FUEL_SLOT => {
                let target = if click.slot == INPUT_SLOT {
                    &mut self.input
                } else {
                    &mut self.fuel
                };
                if click.button == 1 {
                    if target.item_count.0 > 0
                        && let Some(mut changes) = inventory.add_item_stack(target)
                    {
                        outcome.inventory_changes.append(&mut changes);
                    }
                    *target = crate::inventory::empty_slot();
                } else if let Some((_, mut item)) = inventory.drop_selected(false) {
                    item.item_count.0 = 1;
                    *target = item;
                    outcome
                        .inventory_changes
                        .push(inventory.selected_hotbar_change());
                }
            }
            OUTPUT_SLOT => {
                if self.output.item_count.0 > 0
                    && let Some(mut changes) = inventory.add_item_stack(&self.output)
                {
                    outcome.inventory_changes.append(&mut changes);
                    self.output = crate::inventory::empty_slot();
                }
            }
            _ => {}
        }
        self.sync(sink).await?;
        Ok(Some(outcome))
    }

    pub(in crate::gameplay) async fn tick<W>(
        &mut self,
        sink: &mut qexed_connection::transport::PacketSink<W>,
        player: &qexed_player::OnlinePlayer,
        plugins: &qexed_plugins::PluginManager,
        config: &crate::config::GameplayConfig,
    ) -> Result<GameplayActionOutcome>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if !self.open || !config.furnace {
            return Ok(GameplayActionOutcome::default());
        }
        let mut outcome = GameplayActionOutcome::handled();
        let recipe = self.resolve_recipe(player, plugins);
        if self.burn_time <= 0 && recipe.is_some() {
            if let Some(fuel_ticks) = fuel_ticks(&self.fuel) {
                self.burn_time = fuel_ticks;
                self.burn_time_total = fuel_ticks;
                items::decrement_slot(&mut self.fuel, 1);
            }
        }

        if self.burn_time > 0 {
            self.burn_time -= 1;
            if let Some(recipe) = recipe {
                self.cook_time_total = recipe.cook_time.max(1);
                self.cook_time += 1;
                self.active_recipe = Some(recipe.clone());
                if self.cook_time >= self.cook_time_total {
                    if self.can_accept_output(&recipe.result) {
                        items::decrement_slot(&mut self.input, 1);
                        merge_output(&mut self.output, &recipe.result);
                        self.cook_time = 0;
                        outcome.grant_triggers.push(
                            crate::config::CustomAdvancementTrigger::Smelt,
                        );
                    }
                }
            } else {
                self.cook_time = 0;
            }
        } else {
            self.cook_time = 0;
        }

        plugins.emit_furnace_tick(&qexed_plugins::api::FurnaceTickPayload {
            player: crate::plugin_bridge::player_payload_owned(player),
            input: (self.input.item_count.0 > 0).then(|| items::item_stack_payload(&self.input)),
            fuel: (self.fuel.item_count.0 > 0).then(|| items::item_stack_payload(&self.fuel)),
            output: (self.output.item_count.0 > 0).then(|| items::item_stack_payload(&self.output)),
            burn_time: self.burn_time,
            cook_time: self.cook_time,
            cook_time_total: self.cook_time_total,
        });
        self.sync(sink).await?;
        Ok(outcome)
    }

    fn resolve_recipe(
        &self,
        player: &qexed_player::OnlinePlayer,
        plugins: &qexed_plugins::PluginManager,
    ) -> Option<FurnaceRecipe> {
        if self.input.item_count.0 <= 0 {
            return None;
        }
        let vanilla = furnace_registry().recipe_for(&items::item_name(&self.input));
        let query = qexed_plugins::api::FurnaceRecipeQuery {
            player: crate::plugin_bridge::player_payload_owned(player),
            input: items::item_stack_payload(&self.input),
            fuel: items::item_stack_payload(&self.fuel),
            vanilla_result: vanilla
                .as_ref()
                .map(|recipe| items::item_stack_payload(&recipe.result)),
            cook_time: vanilla
                .as_ref()
                .map(|recipe| recipe.cook_time)
                .unwrap_or(200),
            experience: vanilla
                .as_ref()
                .map(|recipe| recipe.experience)
                .unwrap_or(0.0),
        };
        if let Some(response) = plugins.apply_furnace_recipe(query) {
            if response.replace {
                return response.result.and_then(|item| {
                    Some(FurnaceRecipe {
                        input: items::item_name(&self.input),
                        result: crate::inventory::simple_item(item.item_id, item.count.max(1)),
                        cook_time: response.cook_time.unwrap_or(200).max(1),
                        experience: response.experience.unwrap_or(0.0),
                    })
                });
            }
            if let Some(item) = response.result {
                return Some(FurnaceRecipe {
                    input: items::item_name(&self.input),
                    result: crate::inventory::simple_item(item.item_id, item.count.max(1)),
                    cook_time: response.cook_time.unwrap_or(200).max(1),
                    experience: response.experience.unwrap_or(0.0),
                });
            }
        }
        vanilla
    }

    fn can_accept_output(&self, result: &Slot) -> bool {
        self.output.item_count.0 == 0
            || (crate::inventory::same_stack_kind(&self.output, result)
                && self.output.item_count.0 + result.item_count.0 <= 64)
    }

    async fn sync<W>(&self, sink: &mut qexed_connection::transport::PacketSink<W>) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        sink.send(ContainerSetContent {
            window_id: VarInt(FURNACE_WINDOW_ID),
            state_id: VarInt(self.state_id),
            slot_data: vec![self.input.clone(), self.fuel.clone(), self.output.clone()],
            carried_item: crate::inventory::empty_slot(),
        })
        .await?;
        for (property, value) in [
            (0, self.burn_time),
            (1, self.burn_time_total),
            (2, self.cook_time),
            (3, self.cook_time_total),
        ] {
            sink.send(ContainerSetData {
                window_id: VarInt(FURNACE_WINDOW_ID),
                property,
                value: value.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
            })
            .await?;
        }
        Ok(())
    }
}

fn merge_output(output: &mut Slot, result: &Slot) {
    if output.item_count.0 <= 0 {
        *output = result.clone();
    } else if crate::inventory::same_stack_kind(output, result) {
        output.item_count.0 += result.item_count.0;
    }
}

fn fuel_ticks(slot: &Slot) -> Option<i32> {
    let name = items::item_name(slot);
    let ticks = match name.as_str() {
        "minecraft:coal" | "minecraft:charcoal" => 1600,
        "minecraft:coal_block" => 16000,
        "minecraft:lava_bucket" => 20000,
        "minecraft:blaze_rod" => 2400,
        "minecraft:dried_kelp_block" => 4000,
        "minecraft:bamboo" => 50,
        name if name.ends_with("_planks")
            || name.ends_with("_log")
            || name.ends_with("_wood")
            || name.ends_with("_stem")
            || name.ends_with("_hyphae") =>
        {
            300
        }
        name if name.ends_with("_slab") => 150,
        name if name.ends_with("_sapling") || name.ends_with("_button") => 100,
        _ => return None,
    };
    Some(ticks)
}

fn menu_id(name: &str) -> i32 {
    qexed_mojang_data::registry_sync::load_registry_id_map("minecraft:menu")
        .ok()
        .and_then(|map| map.get(name).copied())
        .unwrap_or_else(|| match name {
            "minecraft:furnace" => 13,
            "minecraft:crafting" => 14,
            _ => 0,
        })
}

#[derive(Debug, Clone)]
struct FurnaceRecipe {
    input: String,
    result: Slot,
    cook_time: i32,
    experience: f32,
}

#[derive(Debug, Default)]
struct FurnaceRegistry {
    recipes: Vec<FurnaceRecipe>,
}

impl FurnaceRegistry {
    fn recipe_for(&self, item: &str) -> Option<FurnaceRecipe> {
        self.recipes
            .iter()
            .find(|recipe| recipe.input == item)
            .cloned()
    }
}

fn furnace_registry() -> &'static FurnaceRegistry {
    static REGISTRY: OnceLock<FurnaceRegistry> = OnceLock::new();
    REGISTRY.get_or_init(load_recipes)
}

fn load_recipes() -> FurnaceRegistry {
    // 数据根候选：运行目录相对路径 + 编译期工作区根（复用 recipes 域的候选逻辑）。
    let root = crate::recipes::mojang_recipe_dir();
    let mut recipes = Vec::new();
    if let Some(root) = root
        && let Ok(entries) = std::fs::read_dir(root)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let Some(recipe) = read_recipe(&path) else {
                continue;
            };
            recipes.push(recipe);
        }
    }
    if recipes.is_empty() {
        recipes = fallback_recipes();
    }
    FurnaceRegistry { recipes }
}

fn read_recipe(path: &std::path::Path) -> Option<FurnaceRecipe> {
    let value = std::fs::read_to_string(path).ok()?;
    let raw: RawFurnaceRecipe = serde_json::from_str(&value).ok()?;
    if !matches!(
        raw.r#type.as_str(),
        "minecraft:smelting" | "minecraft:blasting" | "minecraft:smoking"
    ) {
        return None;
    }
    let input = raw.ingredient.value()?;
    let item_id = crate::inventory::item_id_for_name(&raw.result.id)?;
    Some(FurnaceRecipe {
        input,
        result: crate::inventory::simple_item(item_id, raw.result.count.unwrap_or(1)),
        cook_time: raw.cookingtime.unwrap_or(200),
        experience: raw.experience.unwrap_or(0.0),
    })
}

fn fallback_recipes() -> Vec<FurnaceRecipe> {
    [
        ("minecraft:iron_ore", "minecraft:iron_ingot"),
        ("minecraft:deepslate_iron_ore", "minecraft:iron_ingot"),
        ("minecraft:raw_iron", "minecraft:iron_ingot"),
        ("minecraft:gold_ore", "minecraft:gold_ingot"),
        ("minecraft:deepslate_gold_ore", "minecraft:gold_ingot"),
        ("minecraft:raw_gold", "minecraft:gold_ingot"),
        ("minecraft:copper_ore", "minecraft:copper_ingot"),
        ("minecraft:raw_copper", "minecraft:copper_ingot"),
        ("minecraft:sand", "minecraft:glass"),
        ("minecraft:cobblestone", "minecraft:stone"),
        ("minecraft:raw_beef", "minecraft:cooked_beef"),
        ("minecraft:raw_chicken", "minecraft:cooked_chicken"),
        ("minecraft:raw_porkchop", "minecraft:cooked_porkchop"),
    ]
    .into_iter()
    .filter_map(|(input, output)| {
        crate::inventory::item_id_for_name(output).map(|item_id| FurnaceRecipe {
            input: input.to_string(),
            result: crate::inventory::simple_item(item_id, 1),
            cook_time: 200,
            experience: 0.0,
        })
    })
    .collect()
}

#[derive(Debug, Deserialize)]
struct RawFurnaceRecipe {
    r#type: String,
    ingredient: RawIngredient,
    result: RawResult,
    #[serde(default)]
    cookingtime: Option<i32>,
    #[serde(default)]
    experience: Option<f32>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawIngredient {
    String(String),
    Object {
        item: Option<String>,
        tag: Option<String>,
    },
}

impl RawIngredient {
    fn value(self) -> Option<String> {
        match self {
            Self::String(value) => Some(value),
            Self::Object {
                item: Some(item), ..
            } => Some(item),
            Self::Object { tag: Some(tag), .. } => Some(format!("#{tag}")),
            Self::Object { .. } => None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawResult {
    id: String,
    #[serde(default)]
    count: Option<i32>,
}
