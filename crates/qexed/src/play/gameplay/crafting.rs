use std::sync::OnceLock;

use anyhow::Result;
use qexed_packet::net_types::VarInt;
use qexed_protocol::{
    to_client::play::{
        container_set_content::ContainerSetContent, container_set_slot, open_screen::OpenScreen,
    },
    to_server::play::container_click::ContainerClick,
    types::Slot,
};
use serde::Deserialize;

use super::{GameplayActionOutcome, items};
use crate::play::util::text_component;

pub(in crate::play) const CRAFTING_WINDOW_ID: i32 = 21;
const RESULT_SLOT: i16 = 0;
const GRID_START: i16 = 1;
const GRID_END: i16 = 9;

#[derive(Debug, Default)]
pub(in crate::play) struct CraftingRuntime {
    open: bool,
    state_id: i32,
    grid: Vec<Slot>,
    result: Slot,
    current_recipe: Option<String>,
}

impl CraftingRuntime {
    pub(super) fn new() -> Self {
        Self {
            grid: vec![crate::inventory::empty_slot(); 9],
            ..Self::default()
        }
    }

    pub(super) fn is_open(&self) -> bool {
        self.open
    }

    pub(in crate::play) async fn open<W>(
        &mut self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        self.open = true;
        self.state_id = self.state_id.wrapping_add(1);
        self.grid.fill(crate::inventory::empty_slot());
        self.result = crate::inventory::empty_slot();
        self.current_recipe = None;
        sink.send(OpenScreen {
            window_id: VarInt(CRAFTING_WINDOW_ID),
            menu_type: VarInt(menu_id("minecraft:crafting")),
            title: text_component("Crafting"),
        })
        .await?;
        self.sync(sink).await
    }

    pub(in crate::play) async fn close<W>(
        &mut self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
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
        for item in self.grid.iter_mut() {
            if item.item_count.0 <= 0 {
                continue;
            }
            if let Some(mut changes) = inventory.add_item_stack(item) {
                outcome.inventory_changes.append(&mut changes);
            }
            *item = crate::inventory::empty_slot();
        }
        self.result = crate::inventory::empty_slot();
        sink.send(
            qexed_protocol::to_client::play::container_close::ContainerClose {
                window_id: VarInt(CRAFTING_WINDOW_ID),
            },
        )
        .await?;
        outcome.close_window = true;
        Ok(outcome)
    }

    pub(in crate::play) async fn handle_click<W>(
        &mut self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        click: &ContainerClick,
        inventory: &mut crate::inventory::PlayerInventory,
        player: &crate::players::OnlinePlayer,
        plugins: &crate::plugins::PluginManager,
        config: &qexed_config::app::qexed::server::Gameplay,
    ) -> Result<Option<GameplayActionOutcome>>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if !self.open || click.window_id.0 != CRAFTING_WINDOW_ID {
            return Ok(None);
        }
        let mut outcome = GameplayActionOutcome::handled();
        if click.slot == RESULT_SLOT {
            if self.result.item_count.0 > 0 {
                let recipe_id = self.current_recipe.clone().unwrap_or_default();
                let response = plugins.handle_craft_item(crate::plugins::CraftItemQuery {
                    player: qexed_plugin_api::player_payload_owned(player),
                    recipe_id,
                    result: items::item_stack_payload(&self.result),
                    ingredients: self
                        .grid
                        .iter()
                        .filter(|slot| slot.item_count.0 > 0)
                        .map(items::item_stack_payload)
                        .collect(),
                });
                for action in response.actions {
                    outcome.handled |= matches!(
                        action,
                        crate::plugins::PlayerAction::SystemMessage { .. }
                            | crate::plugins::PlayerAction::Velocity { .. }
                    );
                }
                if !response.cancel {
                    let crafted = response
                        .result
                        .and_then(|item| {
                            (item.item_id >= 0).then(|| {
                                crate::inventory::simple_item(item.item_id, item.count.max(1))
                            })
                        })
                        .unwrap_or_else(|| self.result.clone());
                    if let Some(mut changes) = inventory.add_item_stack(&crafted) {
                        outcome.inventory_changes.append(&mut changes);
                        consume_grid_once(&mut self.grid);
                        outcome.grant_triggers.push(
                            qexed_config::app::qexed::server::CustomAdvancementTrigger::Craft,
                        );
                    }
                }
            }
        } else if (GRID_START..=GRID_END).contains(&click.slot) {
            let index = usize::try_from(click.slot - GRID_START).unwrap_or_default();
            if click.button == 1 {
                self.grid[index] = crate::inventory::empty_slot();
            } else if let Some(mut held) = inventory.drop_selected(false).map(|(_, item)| item) {
                held.item_count.0 = 1;
                self.grid[index] = held;
                outcome
                    .inventory_changes
                    .push(inventory.selected_hotbar_change());
            }
        } else {
            let Some(slot) = inventory_hotbar_slot(click.slot) else {
                self.sync(sink).await?;
                return Ok(Some(outcome));
            };
            if let Some(item) = inventory.hotbar_item(slot).cloned()
                && item.item_count.0 > 0
            {
                let mut grid_item = item;
                grid_item.item_count.0 = 1;
                if let Some(change) = inventory.decrement_hotbar_slot(slot, 1) {
                    if let Some(first_empty) =
                        self.grid.iter_mut().find(|slot| slot.item_count.0 == 0)
                    {
                        *first_empty = grid_item;
                        outcome.inventory_changes.push(change);
                    }
                }
            }
        }

        self.recompute_result(player, plugins, config);
        self.sync(sink).await?;
        Ok(Some(outcome))
    }

    fn recompute_result(
        &mut self,
        player: &crate::players::OnlinePlayer,
        plugins: &crate::plugins::PluginManager,
        config: &qexed_config::app::qexed::server::Gameplay,
    ) {
        if !config.crafting {
            self.result = crate::inventory::empty_slot();
            self.current_recipe = None;
            return;
        }
        let vanilla = recipe_registry().match_grid(&self.grid, 3, 3);
        let query = crate::plugins::CraftingRecipeQuery {
            player: qexed_plugin_api::player_payload_owned(player),
            width: 3,
            height: 3,
            ingredients: self
                .grid
                .iter()
                .filter(|slot| slot.item_count.0 > 0)
                .map(items::item_stack_payload)
                .collect(),
            vanilla_result: vanilla
                .as_ref()
                .map(|recipe| items::item_stack_payload(&recipe.output)),
        };
        if let Some(response) = plugins.apply_crafting_recipe(query) {
            if response.replace {
                self.result = response
                    .result
                    .map(|item| crate::inventory::simple_item(item.item_id, item.count.max(1)))
                    .unwrap_or_else(crate::inventory::empty_slot);
                self.current_recipe = Some("plugin".to_string());
                return;
            }
            if let Some(result) = response.result {
                self.result = crate::inventory::simple_item(result.item_id, result.count.max(1));
                self.current_recipe = Some("plugin".to_string());
                return;
            }
        }
        if let Some(recipe) = vanilla {
            self.result = recipe.output;
            self.current_recipe = Some(recipe.id);
        } else {
            self.result = crate::inventory::empty_slot();
            self.current_recipe = None;
        }
    }

    async fn sync<W>(&self, sink: &mut qexed_tcp_connect::PacketSink<W>) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let mut slots = Vec::with_capacity(10);
        slots.push(self.result.clone());
        slots.extend(self.grid.iter().cloned());
        sink.send(ContainerSetContent {
            window_id: VarInt(CRAFTING_WINDOW_ID),
            state_id: VarInt(self.state_id),
            slot_data: slots,
            carried_item: crate::inventory::empty_slot(),
        })
        .await?;
        sink.send(container_set_slot::ContainerSetContent {
            window_id: VarInt(CRAFTING_WINDOW_ID),
            state_id: VarInt(self.state_id),
            slot: RESULT_SLOT,
            slot_data: self.result.clone(),
        })
        .await?;
        Ok(())
    }
}

fn consume_grid_once(grid: &mut [Slot]) {
    for slot in grid {
        items::decrement_slot(slot, 1);
    }
}

fn inventory_hotbar_slot(slot: i16) -> Option<usize> {
    (36..=44)
        .contains(&slot)
        .then(|| usize::try_from(slot - 36).ok())
        .flatten()
}

fn menu_id(name: &str) -> i32 {
    crate::registry_sync::load_registry_id_map("minecraft:menu")
        .ok()
        .and_then(|map| map.get(name).copied())
        .unwrap_or_else(|| match name {
            "minecraft:crafting" => 14,
            "minecraft:furnace" => 13,
            _ => 0,
        })
}

#[derive(Debug, Clone)]
struct MatchedRecipe {
    id: String,
    output: Slot,
}

#[derive(Debug, Default)]
struct RecipeRegistry {
    recipes: Vec<CraftingRecipe>,
}

#[derive(Debug, Clone)]
struct CraftingRecipe {
    id: String,
    width: usize,
    height: usize,
    ingredients: Vec<Option<Ingredient>>,
    shapeless: bool,
    result: Slot,
}

#[derive(Debug, Clone)]
enum Ingredient {
    Item(String),
    Tag(String),
}

impl RecipeRegistry {
    fn match_grid(&self, grid: &[Slot], width: usize, height: usize) -> Option<MatchedRecipe> {
        self.recipes
            .iter()
            .filter(|recipe| recipe.width <= width && recipe.height <= height)
            .find(|recipe| recipe.matches(grid, width, height))
            .map(|recipe| MatchedRecipe {
                id: recipe.id.clone(),
                output: recipe.result.clone(),
            })
    }
}

impl CraftingRecipe {
    fn matches(&self, grid: &[Slot], width: usize, height: usize) -> bool {
        if self.shapeless {
            let mut available = grid
                .iter()
                .filter(|slot| slot.item_count.0 > 0)
                .map(items::item_name)
                .collect::<Vec<_>>();
            if available.len() != self.ingredients.len() {
                return false;
            }
            for ingredient in &self.ingredients {
                let Some(ingredient) = ingredient else {
                    continue;
                };
                let Some(index) = available
                    .iter()
                    .position(|item_name| ingredient.matches(item_name))
                else {
                    return false;
                };
                available.remove(index);
            }
            return true;
        }

        for y_offset in 0..=height.saturating_sub(self.height) {
            for x_offset in 0..=width.saturating_sub(self.width) {
                if self.matches_shaped_at(grid, width, height, x_offset, y_offset) {
                    return true;
                }
            }
        }
        false
    }

    fn matches_shaped_at(
        &self,
        grid: &[Slot],
        grid_width: usize,
        grid_height: usize,
        x_offset: usize,
        y_offset: usize,
    ) -> bool {
        for y in 0..grid_height {
            for x in 0..grid_width {
                let grid_slot = &grid[y * grid_width + x];
                let recipe_x = x.checked_sub(x_offset);
                let recipe_y = y.checked_sub(y_offset);
                let ingredient = recipe_y
                    .filter(|recipe_y| *recipe_y < self.height)
                    .zip(recipe_x.filter(|recipe_x| *recipe_x < self.width))
                    .and_then(|(recipe_y, recipe_x)| {
                        self.ingredients.get(recipe_y * self.width + recipe_x)
                    });
                match (grid_slot.item_count.0 > 0, ingredient) {
                    (false, None) => {}
                    (false, Some(None)) => {}
                    (false, Some(Some(_))) => return false,
                    (true, None) => return false,
                    (true, Some(None)) => return false,
                    (true, Some(Some(ingredient)))
                        if ingredient.matches(&items::item_name(grid_slot)) => {}
                    (true, Some(_)) => return false,
                }
            }
        }
        true
    }
}

impl Ingredient {
    fn matches(&self, item_name: &str) -> bool {
        match self {
            Self::Item(name) => name == item_name,
            Self::Tag(tag) => tag_matches_item(tag, item_name),
        }
    }
}

fn recipe_registry() -> &'static RecipeRegistry {
    static REGISTRY: OnceLock<RecipeRegistry> = OnceLock::new();
    REGISTRY.get_or_init(load_recipes)
}

fn load_recipes() -> RecipeRegistry {
    let root = std::path::Path::new("cache")
        .join("mojang")
        .join(qexed_config::MC_VERSION)
        .join("data/minecraft/recipe");
    let mut recipes = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&root) {
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
    RecipeRegistry { recipes }
}

fn read_recipe(path: &std::path::Path) -> Option<CraftingRecipe> {
    let value = std::fs::read_to_string(path).ok()?;
    let recipe: RawRecipe = serde_json::from_str(&value).ok()?;
    let id = path.file_stem()?.to_string_lossy();
    raw_recipe_to_recipe(format!("minecraft:{id}"), recipe)
}

fn raw_recipe_to_recipe(id: String, recipe: RawRecipe) -> Option<CraftingRecipe> {
    match recipe.r#type.as_str() {
        "minecraft:crafting_shapeless" => {
            let result = result_slot(recipe.result?)?;
            Some(CraftingRecipe {
                id,
                width: 1,
                height: 1,
                ingredients: recipe
                    .ingredients?
                    .into_iter()
                    .filter_map(ingredient)
                    .map(Some)
                    .collect(),
                shapeless: true,
                result,
            })
        }
        "minecraft:crafting_shaped" => {
            let pattern = recipe.pattern?;
            let width = pattern
                .iter()
                .map(|row| row.chars().count())
                .max()
                .unwrap_or(0);
            let height = pattern.len();
            let key = recipe.key.unwrap_or_default();
            let mut ingredients = Vec::new();
            for row in pattern {
                for symbol in row.chars().chain(std::iter::repeat(' ')).take(width) {
                    if symbol == ' ' {
                        ingredients.push(None);
                        continue;
                    }
                    ingredients.push(Some(ingredient(key.get(&symbol.to_string())?.clone())?));
                }
            }
            let result = result_slot(recipe.result?)?;
            Some(CraftingRecipe {
                id,
                width,
                height,
                ingredients,
                shapeless: false,
                result,
            })
        }
        _ => None,
    }
}

fn ingredient(raw: RawIngredient) -> Option<Ingredient> {
    let value = raw.value()?;
    if let Some(item) = value.strip_prefix('#') {
        Some(Ingredient::Tag(item.to_string()))
    } else {
        Some(Ingredient::Item(value))
    }
}

fn result_slot(raw: RawResult) -> Option<Slot> {
    let item_id = crate::inventory::item_id_for_name(&raw.id)?;
    Some(crate::inventory::simple_item(
        item_id,
        raw.count.unwrap_or(1),
    ))
}

fn tag_matches_item(tag: &str, item: &str) -> bool {
    match tag.trim_start_matches("minecraft:") {
        "planks" => item.ends_with("_planks"),
        "oak_logs" => matches!(item, "minecraft:oak_log" | "minecraft:oak_wood"),
        "logs" => item.ends_with("_log") || item.ends_with("_wood") || item.ends_with("_stem"),
        "stone_tool_materials" => item == "minecraft:cobblestone" || item == "minecraft:blackstone",
        "wool" => item.ends_with("_wool"),
        "wooden_buttons" => item.ends_with("_button"),
        _ => false,
    }
}

fn fallback_recipes() -> Vec<CraftingRecipe> {
    let mut recipes = Vec::new();
    let Some(oak_planks) = crate::inventory::item_id_for_name("minecraft:oak_planks") else {
        return recipes;
    };
    if let Some(stick) = crate::inventory::item_id_for_name("minecraft:stick") {
        recipes.push(CraftingRecipe {
            id: "minecraft:stick".to_string(),
            width: 1,
            height: 2,
            ingredients: vec![
                Some(Ingredient::Tag("minecraft:planks".to_string())),
                Some(Ingredient::Tag("minecraft:planks".to_string())),
            ],
            shapeless: false,
            result: crate::inventory::simple_item(stick, 4),
        });
    }
    if let Some(crafting_table) = crate::inventory::item_id_for_name("minecraft:crafting_table") {
        recipes.push(CraftingRecipe {
            id: "minecraft:crafting_table".to_string(),
            width: 2,
            height: 2,
            ingredients: vec![Some(Ingredient::Tag("minecraft:planks".to_string())); 4],
            shapeless: false,
            result: crate::inventory::simple_item(crafting_table, 1),
        });
    }
    recipes.push(CraftingRecipe {
        id: "minecraft:oak_planks".to_string(),
        width: 1,
        height: 1,
        ingredients: vec![Some(Ingredient::Tag("minecraft:oak_logs".to_string()))],
        shapeless: true,
        result: crate::inventory::simple_item(oak_planks, 4),
    });
    recipes
}

#[derive(Debug, Deserialize)]
struct RawRecipe {
    r#type: String,
    #[serde(default)]
    ingredients: Option<Vec<RawIngredient>>,
    #[serde(default)]
    pattern: Option<Vec<String>>,
    #[serde(default)]
    key: Option<std::collections::HashMap<String, RawIngredient>>,
    #[serde(default)]
    result: Option<RawResult>,
}

#[derive(Debug, Clone, Deserialize)]
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
    count: Option<i32>,
}
