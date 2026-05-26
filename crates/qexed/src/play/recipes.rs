use anyhow::{Context, Result};
use qexed_packet::net_types::VarInt;
use qexed_protocol::{
    to_client::play::{
        recipe_book_add::{RecipeBookAdd, Recipes},
        recipe_book_settings::RecipeBookSettings,
        update_recipes::{RecipePropertySetEntry, UpdateRecipes},
    },
    types::{
        IDSet, RecipeDisplay, SlotDisplay,
        minecraft::{CraftingShaped, CraftingShapeless},
        slot_display_types,
    },
};
use serde_json::Value;
use std::{collections::HashMap, path::Path, sync::OnceLock};

const REGISTRIES_REPORT: &str = "assets/reports/registries.json";

pub(super) async fn send_initial_recipe_book<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
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
    UpdateRecipes {
        item_sets: vec![
            RecipePropertySetEntry {
                key: "minecraft:furnace_input".to_string(),
                items: Vec::new(),
            },
            RecipePropertySetEntry {
                key: "minecraft:blast_furnace_input".to_string(),
                items: Vec::new(),
            },
            RecipePropertySetEntry {
                key: "minecraft:smoker_input".to_string(),
                items: Vec::new(),
            },
            RecipePropertySetEntry {
                key: "minecraft:campfire_input".to_string(),
                items: Vec::new(),
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
        stonecutter_recipes: Vec::new(),
    }
}

fn recipe_book_add_packet() -> RecipeBookAdd {
    RecipeBookAdd {
        entries: basic_recipe_entries(),
        replace: true,
    }
}

fn basic_recipe_entries() -> Vec<Recipes> {
    let registry = recipe_registry();
    vec![
        Recipes {
            recipe: VarInt(0),
            display: RecipeDisplay::MinecraftCraftingShapeless(CraftingShapeless {
                ingredients: vec![tag_display("minecraft:oak_logs")],
                result: item_stack_display(registry.item_id("minecraft:oak_planks"), 4),
                crafting_station: SlotDisplay::Empty,
            }),
            group: VarInt(0),
            category: VarInt(recipe_category("minecraft:crafting_building_blocks")),
            ingredients: Some(vec![tag_ingredient("minecraft:oak_logs")]),
            flags: 0,
        },
        Recipes {
            recipe: VarInt(1),
            display: RecipeDisplay::MinecraftCraftingShaped(CraftingShaped {
                width: VarInt(1),
                height: VarInt(2),
                ingredients: vec![
                    tag_display("minecraft:planks"),
                    tag_display("minecraft:planks"),
                ],
                result: item_stack_display(registry.item_id("minecraft:stick"), 4),
                crafting_station: SlotDisplay::Empty,
            }),
            group: VarInt(1),
            category: VarInt(recipe_category("minecraft:crafting_misc")),
            ingredients: Some(vec![
                tag_ingredient("minecraft:planks"),
                tag_ingredient("minecraft:planks"),
            ]),
            flags: 0,
        },
        Recipes {
            recipe: VarInt(2),
            display: RecipeDisplay::MinecraftCraftingShaped(CraftingShaped {
                width: VarInt(2),
                height: VarInt(2),
                ingredients: vec![
                    tag_display("minecraft:planks"),
                    tag_display("minecraft:planks"),
                    tag_display("minecraft:planks"),
                    tag_display("minecraft:planks"),
                ],
                result: item_stack_display(registry.item_id("minecraft:crafting_table"), 1),
                crafting_station: SlotDisplay::Empty,
            }),
            group: VarInt(0),
            category: VarInt(recipe_category("minecraft:crafting_misc")),
            ingredients: Some(vec![
                tag_ingredient("minecraft:planks"),
                tag_ingredient("minecraft:planks"),
                tag_ingredient("minecraft:planks"),
                tag_ingredient("minecraft:planks"),
            ]),
            flags: 0,
        },
        Recipes {
            recipe: VarInt(3),
            display: RecipeDisplay::MinecraftCraftingShaped(CraftingShaped {
                width: VarInt(3),
                height: VarInt(1),
                ingredients: vec![
                    item_display(registry.item_id("minecraft:oak_planks")),
                    item_display(registry.item_id("minecraft:oak_planks")),
                    item_display(registry.item_id("minecraft:oak_planks")),
                ],
                result: item_stack_display(registry.item_id("minecraft:oak_slab"), 6),
                crafting_station: SlotDisplay::Empty,
            }),
            group: VarInt(2),
            category: VarInt(recipe_category("minecraft:crafting_building_blocks")),
            ingredients: Some(vec![
                item_ingredient(registry.item_id("minecraft:oak_planks")),
                item_ingredient(registry.item_id("minecraft:oak_planks")),
                item_ingredient(registry.item_id("minecraft:oak_planks")),
            ]),
            flags: 0,
        },
    ]
}

fn recipe_category(name: &str) -> i32 {
    recipe_registry().category_id(name)
}

fn item_display(item_id: i32) -> SlotDisplay {
    SlotDisplay::Item(slot_display_types::minecraft::Item {
        item_type: VarInt(item_id),
    })
}

fn item_stack_display(item_id: i32, count: i32) -> SlotDisplay {
    SlotDisplay::ItemStack(slot_display_types::minecraft::ItemStack {
        item_stack: crate::inventory::simple_item(item_id, count),
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

fn recipe_registry() -> &'static RecipeRegistry {
    static REGISTRY: OnceLock<RecipeRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        RecipeRegistry::load().unwrap_or_else(|err| {
            log::warn!("failed to load recipe registry data, using fallback ids: {err:#}");
            RecipeRegistry::fallback()
        })
    })
}

struct RecipeRegistry {
    item_ids: HashMap<String, i32>,
    category_ids: HashMap<String, i32>,
}

impl RecipeRegistry {
    fn load() -> Result<Self> {
        let root = workspace_root();
        let path = root.join(REGISTRIES_REPORT);
        Ok(Self {
            item_ids: load_registry_id_map(&path, "minecraft:item")?,
            category_ids: load_registry_id_map(&path, "minecraft:recipe_book_category")?,
        })
    }

    fn fallback() -> Self {
        Self {
            item_ids: HashMap::from([
                ("minecraft:oak_planks".to_string(), 36),
                ("minecraft:oak_slab".to_string(), 271),
                ("minecraft:stick".to_string(), 947),
                ("minecraft:crafting_table".to_string(), 333),
            ]),
            category_ids: HashMap::from([
                ("minecraft:crafting_building_blocks".to_string(), 0),
                ("minecraft:crafting_misc".to_string(), 3),
            ]),
        }
    }

    fn item_id(&self, name: &str) -> i32 {
        self.item_ids.get(name).copied().unwrap_or_default()
    }

    fn category_id(&self, name: &str) -> i32 {
        self.category_ids.get(name).copied().unwrap_or_default()
    }
}

fn load_registry_id_map(path: &Path, registry_id: &str) -> Result<HashMap<String, i32>> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let value: Value =
        serde_json::from_str(&content).with_context(|| format!("parse {}", path.display()))?;
    let entries = value
        .get(registry_id)
        .and_then(|registry| registry.get("entries"))
        .and_then(Value::as_object)
        .with_context(|| format!("registry not found in {}: {registry_id}", path.display()))?;

    let mut ids = HashMap::new();
    for (name, value) in entries {
        let Some(id) = value
            .get("protocol_id")
            .and_then(Value::as_i64)
            .and_then(|id| i32::try_from(id).ok())
        else {
            continue;
        };
        ids.insert(name.clone(), id);
    }
    Ok(ids)
}

fn workspace_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use qexed_packet::{Packet, PacketReader, PacketWriter};

    #[test]
    fn initial_recipe_book_contains_basic_crafting_recipes() {
        let packet = recipe_book_add_packet();

        assert!(packet.replace);
        assert_eq!(packet.entries.len(), 4);
        assert_eq!(packet.entries[0].recipe.0, 0);
        assert_eq!(
            packet.entries[0].category.0,
            recipe_category("minecraft:crafting_building_blocks")
        );
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
    fn update_recipes_packet_round_trips_with_empty_stonecutter_state() {
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
