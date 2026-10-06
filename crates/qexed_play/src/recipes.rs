use qexed_packet::net_types::VarInt;
use qexed_protocol::{
    to_client::play::{
        recipe_book_add::{RecipeBookAdd, Recipes},
        recipe_book_settings::RecipeBookSettings,
        update_recipes::{RecipePropertySetEntry, StonecutterRecipeEntry, UpdateRecipes},
    },
    types::{
        IDSet, RecipeDisplay, SlotDisplay,
        minecraft::{CraftingShaped, CraftingShapeless, Furnace, Stonecutter},
        slot_display_types,
    },
};
use serde::Deserialize;
use std::{collections::HashMap, sync::OnceLock};

use crate::error::Result;

pub(crate) async fn send_initial_recipe_book<W>(
    sink: &mut qexed_connection::transport::PacketSink<W>,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    sink.send(update_recipes_packet()).await?;
    sink.send(RecipeBookSettings::default()).await?;
    sink.send(recipe_book_add_packet()).await?;
    Ok(())
}

fn update_recipes_packet() -> UpdateRecipes {
    let registry = recipe_book_registry();
    UpdateRecipes {
        item_sets: vec![
            RecipePropertySetEntry {
                key: "minecraft:furnace_input".to_string(),
                items: registry.furnace_inputs.clone(),
            },
            RecipePropertySetEntry {
                key: "minecraft:blast_furnace_input".to_string(),
                items: registry.blast_furnace_inputs.clone(),
            },
            RecipePropertySetEntry {
                key: "minecraft:smoker_input".to_string(),
                items: registry.smoker_inputs.clone(),
            },
            RecipePropertySetEntry {
                key: "minecraft:campfire_input".to_string(),
                items: registry.campfire_inputs.clone(),
            },
            RecipePropertySetEntry {
                key: "minecraft:smithing_base".to_string(),
                items: Vec::new(),
            },
            RecipePropertySetEntry {
                key: "minecraft:smithing_template".to_string(),
                items: Vec::new(),
            },
            RecipePropertySetEntry {
                key: "minecraft:smithing_addition".to_string(),
                items: Vec::new(),
            },
        ],
        stonecutter_recipes: registry.stonecutter_recipes.clone(),
    }
}

fn recipe_book_add_packet() -> RecipeBookAdd {
    RecipeBookAdd {
        entries: recipe_book_registry().entries.clone(),
        replace: true,
    }
}

fn recipe_book_registry() -> &'static RecipeBookRegistry {
    static REGISTRY: OnceLock<RecipeBookRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        RecipeBookRegistry::load().unwrap_or_else(|err| {
            log::warn!(
                "{}",
                qexed_language::t("qexed.play.recipes.fallback")
                    .replace("%{error}", &format!("{err:#}"))
            );
            RecipeBookRegistry::fallback()
        })
    })
}

#[derive(Debug, Default)]
struct RecipeBookRegistry {
    entries: Vec<Recipes>,
    stonecutter_recipes: Vec<StonecutterRecipeEntry>,
    furnace_inputs: Vec<VarInt>,
    blast_furnace_inputs: Vec<VarInt>,
    smoker_inputs: Vec<VarInt>,
    campfire_inputs: Vec<VarInt>,
}

impl RecipeBookRegistry {
    fn load() -> Result<Self> {
        let item_ids = qexed_mojang_data::registry_sync::load_registry_id_map("minecraft:item")?;
        let category_ids =
            qexed_mojang_data::registry_sync::load_registry_id_map("minecraft:recipe_book_category")
                .unwrap_or_else(|_| fallback_category_ids());
        let mut registry = Self::default();
        // 数据根候选：cwd 相对（服务器运行目录）+ 编译期工作区根（测试 cwd 是 crate 目录）。
        let root = mojang_recipe_dir()
            .ok_or_else(|| {
                crate::error::PlayError::msg(format!(
                    "mojang recipe data not found for {}",
                    qexed_config::MC_VERSION
                ))
            })?;

        for entry in std::fs::read_dir(&root)? {
            let path = entry?.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let Some(recipe) = read_recipe(&path) else {
                continue;
            };
            let recipe_id = registry.entries.len() as i32;
            if let Some(entry) = recipe.to_recipe_book_entry(recipe_id, &item_ids, &category_ids) {
                registry.record_recipe_property_sets(&recipe, &item_ids);
                if let Some(stonecutter) = recipe.to_stonecutter_entry(&item_ids) {
                    registry.stonecutter_recipes.push(stonecutter);
                }
                registry.entries.push(entry);
            }
        }

        if registry.entries.is_empty() {
            Ok(Self::fallback())
        } else {
            registry.dedup_property_sets();
            Ok(registry)
        }
    }

    fn fallback() -> Self {
        let registry = RecipeLookup {
            item_ids: HashMap::from([
                ("minecraft:oak_planks".to_string(), 36),
                ("minecraft:stick".to_string(), 947),
                ("minecraft:crafting_table".to_string(), 333),
                ("minecraft:furnace".to_string(), 395),
                ("minecraft:cobblestone".to_string(), 14),
            ]),
            category_ids: fallback_category_ids(),
        };
        let recipes = vec![
            RawRecipe {
                r#type: "minecraft:crafting_shapeless".to_string(),
                category: Some("building".to_string()),
                group: Some("planks".to_string()),
                ingredients: Some(vec![RawIngredient::String(
                    "#minecraft:oak_logs".to_string(),
                )]),
                pattern: None,
                key: None,
                ingredient: None,
                result: Some(RawResult {
                    id: "minecraft:oak_planks".to_string(),
                    count: Some(4),
                }),
                cookingtime: None,
                experience: None,
            },
            RawRecipe {
                r#type: "minecraft:crafting_shaped".to_string(),
                category: Some("misc".to_string()),
                group: Some("sticks".to_string()),
                ingredients: None,
                pattern: Some(vec!["#".to_string(), "#".to_string()]),
                key: Some(HashMap::from([(
                    "#".to_string(),
                    RawIngredient::String("#minecraft:planks".to_string()),
                )])),
                ingredient: None,
                result: Some(RawResult {
                    id: "minecraft:stick".to_string(),
                    count: Some(4),
                }),
                cookingtime: None,
                experience: None,
            },
        ];
        let entries = recipes
            .into_iter()
            .enumerate()
            .filter_map(|(index, recipe)| {
                recipe.to_recipe_book_entry(
                    index as i32,
                    &registry.item_ids,
                    &registry.category_ids,
                )
            })
            .collect();
        Self {
            entries,
            ..Self::default()
        }
    }

    fn record_recipe_property_sets(&mut self, recipe: &RawRecipe, item_ids: &HashMap<String, i32>) {
        let Some(ingredient) = recipe.ingredient.as_ref() else {
            return;
        };
        let Some(item_id) = ingredient.item_id(item_ids) else {
            return;
        };
        match recipe.r#type.as_str() {
            "minecraft:smelting" => self.furnace_inputs.push(VarInt(item_id)),
            "minecraft:blasting" => self.blast_furnace_inputs.push(VarInt(item_id)),
            "minecraft:smoking" => self.smoker_inputs.push(VarInt(item_id)),
            "minecraft:campfire_cooking" => self.campfire_inputs.push(VarInt(item_id)),
            _ => {}
        }
    }

    fn dedup_property_sets(&mut self) {
        dedup_varints(&mut self.furnace_inputs);
        dedup_varints(&mut self.blast_furnace_inputs);
        dedup_varints(&mut self.smoker_inputs);
        dedup_varints(&mut self.campfire_inputs);
    }
}

struct RecipeLookup {
    item_ids: HashMap<String, i32>,
    category_ids: HashMap<String, i32>,
}

fn read_recipe(path: &std::path::Path) -> Option<RawRecipe> {
    let value = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&value).ok()
}

impl RawRecipe {
    fn to_recipe_book_entry(
        &self,
        recipe_id: i32,
        item_ids: &HashMap<String, i32>,
        category_ids: &HashMap<String, i32>,
    ) -> Option<Recipes> {
        let result = self.result_display(item_ids)?;
        let category = recipe_book_category(&self.r#type, self.category.as_deref(), category_ids);
        let group = stable_group_id(self.group.as_deref().unwrap_or_default());
        let display = match self.r#type.as_str() {
            "minecraft:crafting_shapeless" => {
                let ingredients = self
                    .ingredients
                    .as_ref()?
                    .iter()
                    .filter_map(|ingredient| ingredient.slot_display(item_ids))
                    .collect::<Vec<_>>();
                if ingredients.is_empty() {
                    return None;
                }
                RecipeDisplay::MinecraftCraftingShapeless(CraftingShapeless {
                    ingredients,
                    result,
                    crafting_station: crafting_station(item_ids),
                })
            }
            "minecraft:crafting_shaped" => {
                let pattern = self.pattern.as_ref()?;
                let width = pattern
                    .iter()
                    .map(|row| row.chars().count())
                    .max()
                    .unwrap_or(0);
                let height = pattern.len();
                if width == 0 || height == 0 {
                    return None;
                }
                let key = self.key.as_ref()?;
                let mut ingredients = Vec::with_capacity(width * height);
                for row in pattern {
                    for symbol in row.chars().chain(std::iter::repeat(' ')).take(width) {
                        if symbol == ' ' {
                            ingredients.push(SlotDisplay::Empty);
                        } else {
                            ingredients.push(key.get(&symbol.to_string())?.slot_display(item_ids)?);
                        }
                    }
                }
                RecipeDisplay::MinecraftCraftingShaped(CraftingShaped {
                    width: VarInt(width as i32),
                    height: VarInt(height as i32),
                    ingredients,
                    result,
                    crafting_station: crafting_station(item_ids),
                })
            }
            "minecraft:smelting"
            | "minecraft:blasting"
            | "minecraft:smoking"
            | "minecraft:campfire_cooking" => {
                let ingredient = self.ingredient.as_ref()?.slot_display(item_ids)?;
                RecipeDisplay::MinecraftFurnace(Furnace {
                    ingredient,
                    fuel: SlotDisplay::AnyFuel,
                    result,
                    crafting_station: furnace_station(&self.r#type, item_ids),
                    cooking_time: VarInt(self.cookingtime.unwrap_or(200)),
                    experience: self.experience.unwrap_or(0.0),
                })
            }
            "minecraft:stonecutting" => {
                let ingredient = self.ingredient.as_ref()?.slot_display(item_ids)?;
                RecipeDisplay::MinecraftStonecutter(Stonecutter {
                    ingredient,
                    result,
                    crafting_station: item_slot_display("minecraft:stonecutter", item_ids)
                        .unwrap_or(SlotDisplay::Empty),
                })
            }
            _ => return None,
        };

        Some(Recipes {
            recipe: VarInt(recipe_id),
            display,
            group: qexed_packet::net_types::OptionalVarInt(
                (!self.group.as_deref().unwrap_or_default().is_empty()).then_some(VarInt(group)),
            ),
            category: VarInt(category),
            ingredients: self.ingredient_sets(item_ids),
            flags: 0,
        })
    }

    fn to_stonecutter_entry(
        &self,
        item_ids: &HashMap<String, i32>,
    ) -> Option<StonecutterRecipeEntry> {
        if self.r#type != "minecraft:stonecutting" {
            return None;
        }
        Some(StonecutterRecipeEntry {
            input: self.ingredient.as_ref()?.id_set(item_ids)?,
            option_display: self.result_display(item_ids)?,
        })
    }

    fn result_display(&self, item_ids: &HashMap<String, i32>) -> Option<SlotDisplay> {
        let result = self.result.as_ref()?;
        let item_id = item_ids.get(&result.id).copied()?;
        Some(item_stack_display(item_id, result.count.unwrap_or(1)))
    }

    fn ingredient_sets(&self, item_ids: &HashMap<String, i32>) -> Option<Vec<IDSet>> {
        let sets = match self.r#type.as_str() {
            "minecraft:crafting_shapeless" => self
                .ingredients
                .as_ref()?
                .iter()
                .filter_map(|ingredient| ingredient.id_set(item_ids))
                .collect::<Vec<_>>(),
            "minecraft:crafting_shaped" => {
                let key = self.key.as_ref()?;
                let pattern = self.pattern.as_ref()?;
                let width = pattern
                    .iter()
                    .map(|row| row.chars().count())
                    .max()
                    .unwrap_or(0);
                let mut sets = Vec::new();
                for row in pattern {
                    for symbol in row.chars().chain(std::iter::repeat(' ')).take(width) {
                        if symbol == ' ' {
                            continue;
                        }
                        if let Some(set) = key
                            .get(&symbol.to_string())
                            .and_then(|ingredient| ingredient.id_set(item_ids))
                        {
                            sets.push(set);
                        }
                    }
                }
                sets
            }
            "minecraft:smelting"
            | "minecraft:blasting"
            | "minecraft:smoking"
            | "minecraft:campfire_cooking"
            | "minecraft:stonecutting" => vec![self.ingredient.as_ref()?.id_set(item_ids)?],
            _ => Vec::new(),
        };
        (!sets.is_empty()).then_some(sets)
    }
}

#[derive(Debug, Deserialize)]
struct RawRecipe {
    r#type: String,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    group: Option<String>,
    #[serde(default)]
    ingredients: Option<Vec<RawIngredient>>,
    #[serde(default)]
    pattern: Option<Vec<String>>,
    #[serde(default)]
    key: Option<HashMap<String, RawIngredient>>,
    #[serde(default)]
    ingredient: Option<RawIngredient>,
    #[serde(default)]
    result: Option<RawResult>,
    #[serde(default)]
    cookingtime: Option<i32>,
    #[serde(default)]
    experience: Option<f32>,
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
    fn value(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            Self::Object {
                item: Some(item), ..
            } => Some(item),
            Self::Object { tag: Some(tag), .. } => Some(tag),
            Self::Object { .. } => None,
        }
    }

    fn is_tag(&self) -> bool {
        match self {
            Self::String(value) => value.starts_with('#'),
            Self::Object { tag: Some(_), .. } => true,
            Self::Object { .. } => false,
        }
    }

    fn item_id(&self, item_ids: &HashMap<String, i32>) -> Option<i32> {
        if self.is_tag() {
            return None;
        }
        item_ids.get(self.value()?).copied()
    }

    fn slot_display(&self, item_ids: &HashMap<String, i32>) -> Option<SlotDisplay> {
        let value = self.value()?;
        if self.is_tag() {
            Some(tag_display(value.trim_start_matches('#')))
        } else {
            item_slot_display(value, item_ids)
        }
    }

    fn id_set(&self, item_ids: &HashMap<String, i32>) -> Option<IDSet> {
        let value = self.value()?;
        if self.is_tag() {
            Some(tag_ingredient(value.trim_start_matches('#')))
        } else {
            Some(item_ingredient(item_ids.get(value).copied()?))
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawResult {
    id: String,
    #[serde(default)]
    count: Option<i32>,
}

fn item_slot_display(name: &str, item_ids: &HashMap<String, i32>) -> Option<SlotDisplay> {
    item_ids.get(name).copied().map(item_display)
}

fn item_display(item_id: i32) -> SlotDisplay {
    SlotDisplay::Item(slot_display_types::minecraft::Item {
        item_type: VarInt(item_id),
    })
}

fn item_stack_display(item_id: i32, count: i32) -> SlotDisplay {
    SlotDisplay::ItemStack(slot_display_types::minecraft::ItemStack {
        item_stack: crate::inventory::simple_item(item_id, count.max(1)),
    })
}

fn tag_display(tag: &str) -> SlotDisplay {
    SlotDisplay::Tag(slot_display_types::minecraft::Tag {
        tag: tag.to_string(),
    })
}

fn item_ingredient(item_id: i32) -> IDSet {
    IDSet {
        r#type: VarInt(2),
        tag_name: None,
        ids: Some(vec![VarInt(item_id)]),
    }
}

fn tag_ingredient(tag: &str) -> IDSet {
    IDSet {
        r#type: VarInt(0),
        tag_name: Some(tag.to_string()),
        ids: None,
    }
}

fn furnace_station(recipe_type: &str, item_ids: &HashMap<String, i32>) -> SlotDisplay {
    let station = match recipe_type {
        "minecraft:blasting" => "minecraft:blast_furnace",
        "minecraft:smoking" => "minecraft:smoker",
        "minecraft:campfire_cooking" => "minecraft:campfire",
        _ => "minecraft:furnace",
    };
    item_slot_display(station, item_ids).unwrap_or(SlotDisplay::Empty)
}

fn crafting_station(item_ids: &HashMap<String, i32>) -> SlotDisplay {
    item_slot_display("minecraft:crafting_table", item_ids).unwrap_or(SlotDisplay::Empty)
}

fn recipe_book_category(
    recipe_type: &str,
    category: Option<&str>,
    category_ids: &HashMap<String, i32>,
) -> i32 {
    let key = match recipe_type {
        "minecraft:smelting" | "minecraft:blasting" => match category.unwrap_or("misc") {
            "blocks" => "minecraft:furnace_blocks",
            "food" => "minecraft:furnace_food",
            _ => "minecraft:furnace_misc",
        },
        "minecraft:smoking" | "minecraft:campfire_cooking" => "minecraft:furnace_food",
        _ => match category.unwrap_or("misc") {
            "building" | "blocks" => "minecraft:crafting_building_blocks",
            "equipment" => "minecraft:crafting_equipment",
            "redstone" => "minecraft:crafting_redstone",
            _ => "minecraft:crafting_misc",
        },
    };
    category_ids
        .get(key)
        .copied()
        .or_else(|| fallback_category_ids().get(key).copied())
        .unwrap_or_default()
}

fn fallback_category_ids() -> HashMap<String, i32> {
    HashMap::from([
        ("minecraft:crafting_building_blocks".to_string(), 0),
        ("minecraft:crafting_redstone".to_string(), 1),
        ("minecraft:crafting_equipment".to_string(), 2),
        ("minecraft:crafting_misc".to_string(), 3),
        ("minecraft:furnace_food".to_string(), 4),
        ("minecraft:furnace_blocks".to_string(), 5),
        ("minecraft:furnace_misc".to_string(), 6),
    ])
}

fn stable_group_id(group: &str) -> i32 {
    if group.is_empty() {
        return 0;
    }
    let hash = group.as_bytes().iter().fold(0_u32, |acc, byte| {
        acc.wrapping_mul(31).wrapping_add(u32::from(*byte))
    });
    (hash & 0x3fff_ffff) as i32
}

fn dedup_varints(items: &mut Vec<VarInt>) {
    items.sort_by_key(|item| item.0);
    items.dedup_by_key(|item| item.0);
}

/// Mojang 配方数据目录候选（运行目录 cache/ 与工作区 run/cache/）。
pub(crate) fn mojang_recipe_dir() -> Option<std::path::PathBuf> {
    let relative = ["cache", "run/cache"].map(|base| {
        std::path::Path::new(base)
            .join("mojang")
            .join(qexed_config::MC_VERSION)
            .join("data/minecraft/recipe")
    });
    let workspace = option_env!("CARGO_MANIFEST_DIR")
        .map(std::path::Path::new)
        .and_then(|dir| dir.parent().and_then(|parent| parent.parent()))
        .map(|root| {
            root.join("run/cache")
                .join("mojang")
                .join(qexed_config::MC_VERSION)
                .join("data/minecraft/recipe")
        });
    relative
        .into_iter()
        .chain(workspace)
        .find(|root| root.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;
    use qexed_packet::{Packet, PacketReader, PacketWriter};

    #[test]
    fn initial_recipe_book_contains_many_vanilla_recipes() {
        let packet = recipe_book_add_packet();

        assert!(packet.replace);
        assert!(
            packet.entries.len() > 100,
            "expected recipe book to be generated from Mojang recipes"
        );
        assert!(
            packet
                .entries
                .iter()
                .any(|entry| matches!(entry.display, RecipeDisplay::MinecraftCraftingShaped(_)))
        );
        assert!(
            packet
                .entries
                .iter()
                .any(|entry| matches!(entry.display, RecipeDisplay::MinecraftCraftingShapeless(_)))
        );
    }

    #[test]
    fn crafting_recipe_book_entries_use_crafting_table_station_icon() {
        let packet = recipe_book_add_packet();
        let mut checked = 0;

        for entry in &packet.entries {
            match &entry.display {
                RecipeDisplay::MinecraftCraftingShaped(recipe) => {
                    assert!(
                        !matches!(recipe.crafting_station, SlotDisplay::Empty),
                        "shaped recipe {:?} has empty station icon",
                        entry.recipe
                    );
                    checked += 1;
                }
                RecipeDisplay::MinecraftCraftingShapeless(recipe) => {
                    assert!(
                        !matches!(recipe.crafting_station, SlotDisplay::Empty),
                        "shapeless recipe {:?} has empty station icon",
                        entry.recipe
                    );
                    checked += 1;
                }
                _ => {}
            }
        }

        assert!(checked > 0);
    }

    #[test]
    fn update_recipes_contains_furnace_and_stonecutter_property_sets() {
        let packet = update_recipes_packet();

        assert!(
            packet
                .item_sets
                .iter()
                .any(|entry| entry.key == "minecraft:furnace_input" && !entry.items.is_empty())
        );
        assert!(!packet.stonecutter_recipes.is_empty());
    }

    #[test]
    fn recipe_book_packets_round_trip() {
        let mut buf = bytes::BytesMut::new();
        let mut writer = PacketWriter::new(&mut buf);
        recipe_book_add_packet().serialize(&mut writer).unwrap();

        let mut bytes = buf.freeze();
        let mut reader = PacketReader::new(&mut bytes);
        let mut decoded = RecipeBookAdd::default();
        decoded.deserialize(&mut reader).unwrap();

        assert_eq!(decoded, recipe_book_add_packet());
    }

    #[test]
    fn update_recipes_packet_round_trips() {
        let mut buf = bytes::BytesMut::new();
        let mut writer = PacketWriter::new(&mut buf);
        update_recipes_packet().serialize(&mut writer).unwrap();

        let mut bytes = buf.freeze();
        let mut reader = PacketReader::new(&mut bytes);
        let mut decoded = UpdateRecipes::default();
        decoded.deserialize(&mut reader).unwrap();

        assert_eq!(decoded, update_recipes_packet());
    }
}
