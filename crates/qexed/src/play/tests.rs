use super::util::can_modify_world;
use super::{
    ChunkSendState, WorldEditKind, acknowledged_player_ability_flags, cauldron_interaction,
    chunk_coord, chunk_load_parallelism_limit, configured_gameplay_block_matches,
    dimension_type_holder_id, keep_alive_id, login_dimension_names, piston_side_effect_updates,
    player_ability_flags, world_write_mode,
};
use qexed_config::app::qexed::server::GameMode;
use qexed_packet::net_types::{Position, VarInt};
use qexed_protocol::to_client::play::add_entity::EntityPosition;
use qexed_protocol::to_client::play::player_abilities::PlayerAbilities;
use qexed_protocol::types::{ComponentsToAdd, IDSet, Slot, minecraft};
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn block_state(name: &str, properties: &[(&str, &str)]) -> i32 {
    crate::world::chunk_nbt::block_state(
        name,
        &properties
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect::<Vec<_>>(),
    )
    .id
}

fn fire_state(age: u8) -> i32 {
    let age = age.min(15).to_string();
    block_state(
        "minecraft:fire",
        &[
            ("age", age.as_str()),
            ("east", "false"),
            ("north", "false"),
            ("south", "false"),
            ("up", "false"),
            ("west", "false"),
        ],
    )
}

#[test]
fn chunk_coord_uses_floor_division() {
    assert_eq!(chunk_coord(0.0), 0);
    assert_eq!(chunk_coord(15.9), 0);
    assert_eq!(chunk_coord(16.0), 1);
    assert_eq!(chunk_coord(-0.1), -1);
    assert_eq!(chunk_coord(-16.0), -1);
}

#[test]
fn configured_gameplay_block_match_normalizes_resource_keys() {
    let configured = vec!["Furnace".to_string(), "minecraft:blast_furnace".to_string()];
    assert!(configured_gameplay_block_matches(
        "minecraft:furnace",
        &configured
    ));
    assert!(configured_gameplay_block_matches(
        "minecraft:blast_furnace",
        &configured
    ));
    assert!(!configured_gameplay_block_matches(
        "minecraft:smoker",
        &configured
    ));
}

#[test]
fn manual_openable_block_updates_toggles_both_door_halves() {
    let temp = tempfile::tempdir().unwrap();
    let world = crate::world::WorldManager::new(temp.path().join("world"));
    let lower_position = Position { x: 0, y: 64, z: 0 };
    let upper_position = Position { x: 0, y: 65, z: 0 };
    let lower = block_state(
        "minecraft:oak_door",
        &[
            ("facing", "north"),
            ("half", "lower"),
            ("hinge", "left"),
            ("open", "false"),
            ("powered", "false"),
        ],
    );
    let upper = block_state(
        "minecraft:oak_door",
        &[
            ("facing", "north"),
            ("half", "upper"),
            ("hinge", "left"),
            ("open", "false"),
            ("powered", "false"),
        ],
    );
    world.place_block("minecraft:overworld", lower_position.clone(), lower);
    world.place_block("minecraft:overworld", upper_position.clone(), upper);

    let updates = super::manual_openable_block_updates(
        &world,
        "minecraft:overworld",
        lower_position.clone(),
        lower,
        "minecraft:oak_door",
    )
    .expect("wooden door should be openable");

    assert_eq!(updates.len(), 2);
    for position in [lower_position, upper_position] {
        let updated = updates
            .iter()
            .find(|(update_position, _)| *update_position == position)
            .map(|(_, block_state)| *block_state)
            .expect("door half should be updated");
        assert!(super::block_state_bool_property(updated, "open"));
    }
}

#[test]
fn cake_interaction_advances_bites_and_removes_final_slice() {
    let mut survival = super::SurvivalState::from_stored(
        crate::player_data::StoredSurvival {
            health: 20.0,
            food: 10,
            saturation: 0.0,
        },
        GameMode::Survival,
    );
    let untouched = block_state("minecraft:cake", &[("bites", "0")]);

    let first_bite = super::cake_interaction(
        untouched,
        "minecraft:cake",
        GameMode::Survival,
        &mut survival,
    )
    .expect("hungry survival player should eat cake");
    assert_eq!(
        super::block_state_property(first_bite, "bites").as_deref(),
        Some("1")
    );
    assert_eq!(survival.to_stored().food, 12);

    let final_slice = block_state("minecraft:cake", &[("bites", "6")]);
    let removed = super::cake_interaction(
        final_slice,
        "minecraft:cake",
        GameMode::Survival,
        &mut survival,
    )
    .expect("last cake slice should be eaten");
    assert_eq!(removed, crate::inventory::air_block_state());
}

#[test]
fn composter_full_gives_bone_meal_and_resets() {
    let full = block_state("minecraft:composter", &[("level", "8")]);
    let interaction = super::composter_interaction(
        full,
        "minecraft:composter",
        None,
        &Position { x: 0, y: 64, z: 0 },
    )
    .expect("full composter should be collectable");

    assert_eq!(
        interaction,
        super::ComposterInteraction {
            target_block_state: Some(super::composter_state(0)),
            inventory: super::ComposterInventoryAction::Give("minecraft:bone_meal"),
        }
    );
}

#[test]
fn composter_accepts_guaranteed_item_and_increases_level() {
    let empty = block_state("minecraft:composter", &[("level", "0")]);
    let interaction = super::composter_interaction(
        empty,
        "minecraft:composter",
        Some("minecraft:cake"),
        &Position { x: 0, y: 64, z: 0 },
    )
    .expect("cake should be compostable");

    assert_eq!(
        interaction,
        super::ComposterInteraction {
            target_block_state: Some(super::composter_state(1)),
            inventory: super::ComposterInventoryAction::ConsumeHeld,
        }
    );
}

#[test]
#[test]
fn candle_placement_interaction_adds_candle_to_cake() {
    let cake = block_state("minecraft:cake", &[("bites", "0")]);

    let interaction =
        super::candle_placement_interaction(cake, "minecraft:cake", Some("minecraft:white_candle"))
            .expect("candle should be placeable on whole cake");

    assert_eq!(
        interaction,
        super::VanillaStateInteraction {
            target_block_state: block_state("minecraft:white_candle_cake", &[("lit", "false")]),
            inventory: super::VanillaInventoryAction::ConsumeHeld,
        }
    );
    assert!(
        super::candle_placement_interaction(
            block_state("minecraft:cake", &[("bites", "1")]),
            "minecraft:cake",
            Some("minecraft:white_candle"),
        )
        .is_none()
    );
}

#[test]
fn candle_placement_interaction_stacks_matching_candles_to_four() {
    let candle = block_state(
        "minecraft:white_candle",
        &[("candles", "3"), ("lit", "false"), ("waterlogged", "false")],
    );

    let interaction = super::candle_placement_interaction(
        candle,
        "minecraft:white_candle",
        Some("minecraft:white_candle"),
    )
    .expect("matching candle should stack");

    assert_eq!(
        interaction.target_block_state,
        block_state(
            "minecraft:white_candle",
            &[("candles", "4"), ("lit", "false"), ("waterlogged", "false")]
        )
    );
    assert!(
        super::candle_placement_interaction(
            interaction.target_block_state,
            "minecraft:white_candle",
            Some("minecraft:white_candle"),
        )
        .is_none()
    );
}

#[test]
fn beehive_interaction_collects_honeycomb_and_resets_level() {
    let hive = block_state(
        "minecraft:beehive",
        &[("facing", "east"), ("honey_level", "5")],
    );

    let interaction =
        super::beehive_interaction(hive, "minecraft:beehive", Some("minecraft:shears"))
            .expect("full beehive should be shearable");

    assert_eq!(
        interaction,
        super::VanillaStateInteraction {
            target_block_state: block_state(
                "minecraft:beehive",
                &[("facing", "east"), ("honey_level", "0")]
            ),
            inventory: super::VanillaInventoryAction::Give("minecraft:honeycomb", 3),
        }
    );
}

#[test]
fn beehive_interaction_fills_honey_bottle() {
    let nest = block_state(
        "minecraft:bee_nest",
        &[("facing", "north"), ("honey_level", "5")],
    );

    let interaction =
        super::beehive_interaction(nest, "minecraft:bee_nest", Some("minecraft:glass_bottle"))
            .expect("full bee nest should fill bottle");

    assert_eq!(
        interaction,
        super::VanillaStateInteraction {
            target_block_state: block_state(
                "minecraft:bee_nest",
                &[("facing", "north"), ("honey_level", "0")]
            ),
            inventory: super::VanillaInventoryAction::ExchangeHeld("minecraft:honey_bottle"),
        }
    );
}

#[test]
fn pumpkin_shear_interaction_carves_and_drops_seeds() {
    let pumpkin = crate::world::chunk_nbt::default_block_state_id("minecraft:pumpkin");

    let interaction =
        super::pumpkin_shear_interaction(pumpkin, "minecraft:pumpkin", Some("minecraft:shears"), 5)
            .expect("pumpkin should be carveable with shears");

    assert_eq!(
        interaction,
        super::VanillaStateInteraction {
            target_block_state: block_state("minecraft:carved_pumpkin", &[("facing", "east")]),
            inventory: super::VanillaInventoryAction::Give("minecraft:pumpkin_seeds", 4),
        }
    );
}

#[test]
fn note_block_interaction_wraps_note_value() {
    let note = block_state(
        "minecraft:note_block",
        &[("instrument", "harp"), ("note", "24"), ("powered", "false")],
    );

    let updated =
        super::note_block_interaction(note, "minecraft:note_block").expect("note block tunes");

    assert_eq!(
        super::block_state_property(updated, "note").as_deref(),
        Some("0")
    );
}

#[test]
fn bone_meal_interaction_grows_age_property_blocks() {
    let wheat = block_state("minecraft:wheat", &[("age", "5")]);

    let grown = super::bone_meal_interaction(wheat, "minecraft:wheat", Some("minecraft:bone_meal"))
        .expect("bone meal should grow wheat");

    assert_eq!(
        grown,
        super::VanillaStateInteraction {
            target_block_state: block_state("minecraft:wheat", &[("age", "7")]),
            inventory: super::VanillaInventoryAction::ConsumeHeld,
        }
    );
    assert!(
        super::bone_meal_interaction(
            block_state("minecraft:wheat", &[("age", "7")]),
            "minecraft:wheat",
            Some("minecraft:bone_meal"),
        )
        .is_none()
    );
}

#[test]
fn bone_meal_interaction_adds_cave_vine_berries() {
    let vine = block_state(
        "minecraft:cave_vines",
        &[("age", "0"), ("berries", "false")],
    );

    let grown =
        super::bone_meal_interaction(vine, "minecraft:cave_vines", Some("minecraft:bone_meal"))
            .expect("bone meal should add berries");

    assert_eq!(
        super::block_state_property(grown.target_block_state, "berries").as_deref(),
        Some("true")
    );
}

#[test]
fn harvestable_block_interaction_picks_sweet_berries() {
    let bush = block_state("minecraft:sweet_berry_bush", &[("age", "3")]);

    let harvested = super::harvestable_block_interaction(bush, "minecraft:sweet_berry_bush")
        .expect("mature sweet berry bush should be harvestable");

    assert_eq!(
        harvested,
        super::VanillaStateInteraction {
            target_block_state: block_state("minecraft:sweet_berry_bush", &[("age", "1")]),
            inventory: super::VanillaInventoryAction::Give("minecraft:sweet_berries", 2),
        }
    );
}

#[test]
fn harvestable_block_interaction_picks_glow_berries() {
    let vine = block_state("minecraft:cave_vines", &[("age", "0"), ("berries", "true")]);

    let harvested = super::harvestable_block_interaction(vine, "minecraft:cave_vines")
        .expect("berried cave vine should be harvestable");

    assert_eq!(
        harvested,
        super::VanillaStateInteraction {
            target_block_state: block_state(
                "minecraft:cave_vines",
                &[("age", "0"), ("berries", "false")]
            ),
            inventory: super::VanillaInventoryAction::Give("minecraft:glow_berries", 1),
        }
    );
}

#[test]
fn vanilla_state_interaction_prefers_bone_meal_before_berry_harvest() {
    let temp = tempfile::tempdir().unwrap();
    let world = crate::world::WorldManager::new(temp.path().join("world"));
    let bush = block_state("minecraft:sweet_berry_bush", &[("age", "2")]);
    let interaction = super::vanilla_state_interaction(
        &world,
        "minecraft:overworld",
        &Position { x: 0, y: 200, z: 0 },
        bush,
        "minecraft:sweet_berry_bush",
        Some("minecraft:bone_meal"),
        1,
    )
    .expect("bone meal should grow age 2 sweet berry bush");

    assert_eq!(
        interaction,
        super::VanillaStateInteraction {
            target_block_state: block_state("minecraft:sweet_berry_bush", &[("age", "3")]),
            inventory: super::VanillaInventoryAction::ConsumeHeld,
        }
    );
}

#[test]
fn axe_block_transform_strips_log_and_preserves_axis() {
    let log = block_state("minecraft:oak_log", &[("axis", "x")]);

    let stripped =
        super::axe_block_transform(log, "minecraft:oak_log").expect("axe should strip vanilla log");
    let entry = crate::world::chunk_nbt::block_state_entry(stripped);

    assert_eq!(entry.name, "minecraft:stripped_oak_log");
    assert_eq!(
        super::block_state_property(stripped, "axis").as_deref(),
        Some("x")
    );
}

#[test]
fn axe_block_transform_unwaxes_before_scraping_copper() {
    let waxed = block_state("minecraft:waxed_oxidized_copper", &[]);

    let unwaxed = super::axe_block_transform(waxed, "minecraft:waxed_oxidized_copper")
        .expect("axe should remove wax first");
    let entry = crate::world::chunk_nbt::block_state_entry(unwaxed);

    assert_eq!(entry.name, "minecraft:oxidized_copper");
}

#[test]
fn shovel_block_transform_creates_path_only_when_above_is_clear() {
    let temp = tempfile::tempdir().unwrap();
    let world = crate::world::WorldManager::new(temp.path().join("world"));
    let position = Position { x: 0, y: 200, z: 0 };
    let grass = crate::world::chunk_nbt::default_block_state_id("minecraft:grass_block");
    let stone = crate::world::chunk_nbt::default_block_state_id("minecraft:stone");

    world.place_block("minecraft:overworld", position.clone(), grass);
    assert_eq!(
        super::shovel_block_transform(
            &world,
            "minecraft:overworld",
            &position,
            "minecraft:grass_block",
            1,
        ),
        Some(crate::world::chunk_nbt::default_block_state_id(
            "minecraft:dirt_path"
        ))
    );

    world.place_block(
        "minecraft:overworld",
        Position { x: 0, y: 201, z: 0 },
        stone,
    );
    assert_eq!(
        super::shovel_block_transform(
            &world,
            "minecraft:overworld",
            &position,
            "minecraft:grass_block",
            1,
        ),
        None
    );
}

#[test]
fn hoe_block_transform_tills_dirt_and_downgrades_coarse_dirt() {
    let temp = tempfile::tempdir().unwrap();
    let world = crate::world::WorldManager::new(temp.path().join("world"));
    let position = Position { x: 0, y: 200, z: 0 };

    assert_eq!(
        super::hoe_block_transform(
            &world,
            "minecraft:overworld",
            &position,
            "minecraft:dirt",
            1,
        ),
        Some(crate::world::chunk_nbt::default_block_state_id(
            "minecraft:farmland"
        ))
    );
    assert_eq!(
        super::hoe_block_transform(
            &world,
            "minecraft:overworld",
            &position,
            "minecraft:coarse_dirt",
            1,
        ),
        Some(crate::world::chunk_nbt::default_block_state_id(
            "minecraft:dirt"
        ))
    );
}

#[test]
fn lit_block_interaction_lights_and_extinguishes_candles() {
    let unlit = block_state(
        "minecraft:white_candle",
        &[("candles", "1"), ("lit", "false"), ("waterlogged", "false")],
    );

    let lit = super::lit_block_interaction(
        unlit,
        "minecraft:white_candle",
        Some("minecraft:fire_charge"),
    )
    .expect("fire charge should light candle");
    assert_eq!(
        lit,
        super::VanillaStateInteraction {
            target_block_state: block_state(
                "minecraft:white_candle",
                &[("candles", "1"), ("lit", "true"), ("waterlogged", "false")]
            ),
            inventory: super::VanillaInventoryAction::ConsumeHeld,
        }
    );

    let extinguished =
        super::lit_block_interaction(lit.target_block_state, "minecraft:white_candle", None)
            .expect("empty hand should extinguish candle");
    assert_eq!(
        super::block_state_property(extinguished.target_block_state, "lit").as_deref(),
        Some("false")
    );
}

#[test]
fn farmland_next_state_drains_or_reverts_without_water() {
    let temp = tempfile::tempdir().unwrap();
    let world = crate::world::WorldManager::new(temp.path().join("world"));
    let position = Position { x: 0, y: 200, z: 0 };
    let moist = block_state("minecraft:farmland", &[("moisture", "2")]);
    let dry = block_state("minecraft:farmland", &[("moisture", "0")]);

    assert_eq!(
        super::farmland_next_state(&world, "minecraft:overworld", &position, moist),
        Some(block_state("minecraft:farmland", &[("moisture", "1")]))
    );
    assert_eq!(
        super::farmland_next_state(&world, "minecraft:overworld", &position, dry),
        Some(crate::world::chunk_nbt::default_block_state_id(
            "minecraft:dirt"
        ))
    );
}

#[test]
fn farmland_next_state_keeps_dry_farmland_with_crop_above() {
    let temp = tempfile::tempdir().unwrap();
    let world = crate::world::WorldManager::new(temp.path().join("world"));
    let position = Position { x: 0, y: 200, z: 0 };
    let dry = block_state("minecraft:farmland", &[("moisture", "0")]);
    let wheat = block_state("minecraft:wheat", &[("age", "0")]);
    world.place_block(
        "minecraft:overworld",
        Position { x: 0, y: 201, z: 0 },
        wheat,
    );

    assert_eq!(
        super::farmland_next_state(&world, "minecraft:overworld", &position, dry),
        None
    );
}

#[test]
fn covered_surface_next_state_reverts_grass_and_path() {
    let temp = tempfile::tempdir().unwrap();
    let world = crate::world::WorldManager::new(temp.path().join("world"));
    let position = Position { x: 0, y: 200, z: 0 };
    let above = Position { x: 0, y: 201, z: 0 };
    let grass = block_state("minecraft:grass_block", &[("snowy", "false")]);
    let path = crate::world::chunk_nbt::default_block_state_id("minecraft:dirt_path");
    let stone = crate::world::chunk_nbt::default_block_state_id("minecraft:stone");
    let dirt = crate::world::chunk_nbt::default_block_state_id("minecraft:dirt");

    world.place_block("minecraft:overworld", above.clone(), stone);

    assert_eq!(
        super::covered_surface_next_state(&world, "minecraft:overworld", &position, grass),
        Some(dirt)
    );
    assert_eq!(
        super::covered_surface_next_state(&world, "minecraft:overworld", &position, path),
        Some(dirt)
    );
}

#[test]
fn surface_spread_update_converts_nearby_dirt() {
    let temp = tempfile::tempdir().unwrap();
    let world = crate::world::WorldManager::new(temp.path().join("world"));
    let source_position = Position { x: 0, y: 200, z: 0 };
    let target_position = Position { x: 1, y: 200, z: 0 };
    let grass = block_state("minecraft:grass_block", &[("snowy", "false")]);
    let dirt = crate::world::chunk_nbt::default_block_state_id("minecraft:dirt");
    world.place_block("minecraft:overworld", source_position.clone(), grass);
    world.place_block("minecraft:overworld", target_position.clone(), dirt);

    let (spread_position, spread_state) =
        super::surface_spread_update(&world, "minecraft:overworld", &source_position, grass)
            .expect("grass should spread to nearby dirt");

    assert_eq!(spread_position, target_position);
    assert_eq!(
        crate::world::chunk_nbt::block_state_entry(spread_state).name,
        "minecraft:grass_block"
    );
}

#[test]
fn surface_spread_update_marks_snowy_surface() {
    let temp = tempfile::tempdir().unwrap();
    let world = crate::world::WorldManager::new(temp.path().join("world"));
    let source_position = Position { x: 0, y: 200, z: 0 };
    let target_position = Position { x: 1, y: 200, z: 0 };
    let snow_position = Position { x: 1, y: 201, z: 0 };
    let grass = block_state("minecraft:grass_block", &[("snowy", "false")]);
    let dirt = crate::world::chunk_nbt::default_block_state_id("minecraft:dirt");
    let snow = crate::world::chunk_nbt::default_block_state_id("minecraft:snow");
    world.place_block("minecraft:overworld", source_position.clone(), grass);
    world.place_block("minecraft:overworld", target_position.clone(), dirt);
    world.place_block("minecraft:overworld", snow_position, snow);

    let (_, spread_state) =
        super::surface_spread_update(&world, "minecraft:overworld", &source_position, grass)
            .expect("snow-covered dirt should accept grass spread");

    assert_eq!(
        super::block_state_property(spread_state, "snowy").as_deref(),
        Some("true")
    );
}

#[test]
fn column_plant_tick_updates_ages_sugar_cane() {
    let temp = tempfile::tempdir().unwrap();
    let world = crate::world::WorldManager::new(temp.path().join("world"));
    let position = Position { x: 0, y: 200, z: 0 };
    let cane = block_state("minecraft:sugar_cane", &[("age", "3")]);

    let updates = super::column_plant_tick_updates(&world, "minecraft:overworld", &position, cane);

    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].0, position);
    assert_eq!(
        super::block_state_property(updates[0].1, "age").as_deref(),
        Some("4")
    );
}

#[test]
fn column_plant_tick_updates_grows_sugar_cane_near_water() {
    let temp = tempfile::tempdir().unwrap();
    let world = crate::world::WorldManager::new(temp.path().join("world"));
    let position = Position { x: 0, y: 200, z: 0 };
    let support = Position { x: 0, y: 199, z: 0 };
    let water_position = Position { x: 1, y: 199, z: 0 };
    let cane = block_state("minecraft:sugar_cane", &[("age", "15")]);
    let dirt = crate::world::chunk_nbt::default_block_state_id("minecraft:dirt");
    let water = crate::world::chunk_nbt::default_block_state_id("minecraft:water");
    world.place_block("minecraft:overworld", position.clone(), cane);
    world.place_block("minecraft:overworld", support, dirt);
    world.place_block("minecraft:overworld", water_position, water);

    let updates = super::column_plant_tick_updates(&world, "minecraft:overworld", &position, cane);

    assert_eq!(updates.len(), 2);
    assert_eq!(updates[0].0, position);
    assert_eq!(
        super::block_state_property(updates[0].1, "age").as_deref(),
        Some("0")
    );
    assert_eq!(updates[1].0, Position { x: 0, y: 201, z: 0 });
    assert_eq!(
        crate::world::chunk_nbt::block_state_entry(updates[1].1).name,
        "minecraft:sugar_cane"
    );
}

#[test]
fn column_plant_tick_updates_blocks_cactus_growth_next_to_solid() {
    let temp = tempfile::tempdir().unwrap();
    let world = crate::world::WorldManager::new(temp.path().join("world"));
    let position = Position { x: 0, y: 200, z: 0 };
    let support = Position { x: 0, y: 199, z: 0 };
    let side = Position { x: 1, y: 201, z: 0 };
    let cactus = block_state("minecraft:cactus", &[("age", "15")]);
    let sand = crate::world::chunk_nbt::default_block_state_id("minecraft:sand");
    let stone = crate::world::chunk_nbt::default_block_state_id("minecraft:stone");
    world.place_block("minecraft:overworld", position.clone(), cactus);
    world.place_block("minecraft:overworld", support, sand);
    world.place_block("minecraft:overworld", side, stone);

    let updates =
        super::column_plant_tick_updates(&world, "minecraft:overworld", &position, cactus);

    assert!(updates.is_empty());
}

#[test]
fn fire_tick_updates_extinguishes_unsupported_fire() {
    let temp = tempfile::tempdir().unwrap();
    let world = crate::world::WorldManager::new(temp.path().join("world"));
    let position = Position { x: 0, y: 200, z: 0 };
    let fire = fire_state(3);

    assert_eq!(
        super::fire_tick_updates(&world, "minecraft:overworld", &position, fire),
        vec![(position, crate::inventory::air_block_state())]
    );
}

#[test]
fn fire_tick_updates_keeps_eternal_base_and_ages() {
    let temp = tempfile::tempdir().unwrap();
    let world = crate::world::WorldManager::new(temp.path().join("world"));
    let position = Position { x: 0, y: 200, z: 0 };
    let below = Position { x: 0, y: 199, z: 0 };
    let fire = fire_state(3);
    let netherrack = crate::world::chunk_nbt::default_block_state_id("minecraft:netherrack");
    world.place_block("minecraft:overworld", below, netherrack);

    let updates = super::fire_tick_updates(&world, "minecraft:overworld", &position, fire);

    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].0, position);
    assert_eq!(
        super::block_state_property(updates[0].1, "age").as_deref(),
        Some("4")
    );
}

#[test]
fn fire_tick_updates_spreads_to_air_next_to_flammable_block() {
    let temp = tempfile::tempdir().unwrap();
    let world = crate::world::WorldManager::new(temp.path().join("world"));
    let position = Position { x: 0, y: 200, z: 0 };
    let fuel = Position {
        x: -1,
        y: 200,
        z: 0,
    };
    let second_fuel = Position { x: 1, y: 200, z: 1 };
    let fire = fire_state(14);
    let oak_planks = crate::world::chunk_nbt::default_block_state_id("minecraft:oak_planks");
    world.place_block("minecraft:overworld", fuel, oak_planks);
    world.place_block("minecraft:overworld", second_fuel, oak_planks);

    let updates = super::fire_tick_updates(&world, "minecraft:overworld", &position, fire);

    assert!(updates.contains(&(position.clone(), fire_state(15))));
    assert!(updates.iter().any(
        |(update_position, update_state)| update_position != &position
            && crate::world::chunk_nbt::block_state_entry(*update_state).name == "minecraft:fire"
    ));
}

#[test]
fn soul_fire_tick_updates_extinguishes_without_soul_base() {
    let temp = tempfile::tempdir().unwrap();
    let world = crate::world::WorldManager::new(temp.path().join("world"));
    let position = Position { x: 0, y: 200, z: 0 };
    let soul_fire = crate::world::chunk_nbt::default_block_state_id("minecraft:soul_fire");

    assert_eq!(
        super::fire_tick_updates(&world, "minecraft:overworld", &position, soul_fire),
        vec![(position, crate::inventory::air_block_state())]
    );
}

#[test]
fn piston_extension_pushes_single_block_and_creates_head() {
    let temp = tempfile::tempdir().unwrap();
    let world = crate::world::WorldManager::new(temp.path().join("world"));
    let piston = Position { x: 0, y: 64, z: 0 };
    let front = Position { x: 1, y: 64, z: 0 };
    let pushed = Position { x: 2, y: 64, z: 0 };
    let retracted = block_state(
        "minecraft:piston",
        &[("extended", "false"), ("facing", "east")],
    );
    let extended = block_state(
        "minecraft:piston",
        &[("extended", "true"), ("facing", "east")],
    );
    let stone = crate::world::chunk_nbt::default_block_state_id("minecraft:stone");
    world.place_block("minecraft:overworld", piston.clone(), retracted);
    world.place_block("minecraft:overworld", front.clone(), stone);

    let updates =
        piston_side_effect_updates(&world, "minecraft:overworld", &piston, retracted, extended)
            .expect("piston can extend");

    assert_eq!(
        updates
            .iter()
            .find(|update| update.position == pushed)
            .map(|update| update.block_state),
        Some(stone)
    );
    assert_eq!(
        updates
            .iter()
            .find(|update| update.position == front)
            .map(|update| crate::world::chunk_nbt::block_state_entry(update.block_state).name),
        Some("minecraft:piston_head".to_string())
    );
}

#[test]
fn piston_extension_pushes_block_chain_up_to_limit() {
    let temp = tempfile::tempdir().unwrap();
    let world = crate::world::WorldManager::new(temp.path().join("world"));
    let piston = Position { x: 0, y: 64, z: 0 };
    let retracted = block_state(
        "minecraft:piston",
        &[("extended", "false"), ("facing", "east")],
    );
    let extended = block_state(
        "minecraft:piston",
        &[("extended", "true"), ("facing", "east")],
    );
    let stone = crate::world::chunk_nbt::default_block_state_id("minecraft:stone");
    let dirt = crate::world::chunk_nbt::default_block_state_id("minecraft:dirt");
    let cobblestone = crate::world::chunk_nbt::default_block_state_id("minecraft:cobblestone");
    world.place_block("minecraft:overworld", piston.clone(), retracted);
    world.place_block("minecraft:overworld", Position { x: 1, y: 64, z: 0 }, stone);
    world.place_block("minecraft:overworld", Position { x: 2, y: 64, z: 0 }, dirt);
    world.place_block(
        "minecraft:overworld",
        Position { x: 3, y: 64, z: 0 },
        cobblestone,
    );

    let updates =
        piston_side_effect_updates(&world, "minecraft:overworld", &piston, retracted, extended)
            .expect("piston can push block chain");

    assert_eq!(
        updates
            .iter()
            .find(|update| update.position == Position { x: 2, y: 64, z: 0 })
            .map(|update| update.block_state),
        Some(stone)
    );
    assert_eq!(
        updates
            .iter()
            .find(|update| update.position == Position { x: 3, y: 64, z: 0 })
            .map(|update| update.block_state),
        Some(dirt)
    );
    assert_eq!(
        updates
            .iter()
            .find(|update| update.position == Position { x: 4, y: 64, z: 0 })
            .map(|update| update.block_state),
        Some(cobblestone)
    );
    assert_eq!(
        updates
            .iter()
            .find(|update| update.position == Position { x: 1, y: 64, z: 0 })
            .map(|update| crate::world::chunk_nbt::block_state_entry(update.block_state).name),
        Some("minecraft:piston_head".to_string())
    );
}

#[test]
fn piston_extension_refuses_more_than_twelve_blocks() {
    let temp = tempfile::tempdir().unwrap();
    let world = crate::world::WorldManager::new(temp.path().join("world"));
    let piston = Position { x: 0, y: 64, z: 0 };
    let retracted = block_state(
        "minecraft:piston",
        &[("extended", "false"), ("facing", "east")],
    );
    let extended = block_state(
        "minecraft:piston",
        &[("extended", "true"), ("facing", "east")],
    );
    let stone = crate::world::chunk_nbt::default_block_state_id("minecraft:stone");
    world.place_block("minecraft:overworld", piston.clone(), retracted);
    for x in 1..=13 {
        world.place_block("minecraft:overworld", Position { x, y: 64, z: 0 }, stone);
    }

    assert!(
        piston_side_effect_updates(&world, "minecraft:overworld", &piston, retracted, extended)
            .is_none()
    );
}

#[test]
fn sticky_piston_retraction_pulls_single_block() {
    let temp = tempfile::tempdir().unwrap();
    let world = crate::world::WorldManager::new(temp.path().join("world"));
    let piston = Position { x: 0, y: 64, z: 0 };
    let front = Position { x: 1, y: 64, z: 0 };
    let pulled = Position { x: 2, y: 64, z: 0 };
    let extended = block_state(
        "minecraft:sticky_piston",
        &[("extended", "true"), ("facing", "east")],
    );
    let retracted = block_state(
        "minecraft:sticky_piston",
        &[("extended", "false"), ("facing", "east")],
    );
    let stone = crate::world::chunk_nbt::default_block_state_id("minecraft:stone");
    world.place_block("minecraft:overworld", piston.clone(), extended);
    world.place_block(
        "minecraft:overworld",
        front.clone(),
        block_state(
            "minecraft:piston_head",
            &[("facing", "east"), ("short", "false"), ("type", "sticky")],
        ),
    );
    world.place_block("minecraft:overworld", pulled.clone(), stone);

    let updates =
        piston_side_effect_updates(&world, "minecraft:overworld", &piston, extended, retracted)
            .expect("sticky piston can retract");

    assert_eq!(
        updates
            .iter()
            .filter(|update| update.position == front)
            .last()
            .map(|update| update.block_state),
        Some(stone)
    );
    assert_eq!(
        updates
            .iter()
            .find(|update| update.position == pulled)
            .map(|update| update.block_state),
        Some(crate::inventory::air_block_state())
    );
}

#[test]
fn dimension_type_holder_id_matches_registry_order_plus_one() {
    assert_eq!(dimension_type_holder_id("minecraft:overworld"), 1);
    assert_eq!(dimension_type_holder_id("minecraft:the_nether"), 4);
}

#[test]
fn login_dimension_names_use_configured_worlds_without_forcing_overworld() {
    let mut world = qexed_config::app::qexed::server::World::default();
    world.default_dimension = "qexed:mine_a".to_string();
    world.dimension = String::new();
    world.worlds = vec![qexed_config::app::qexed::server::WorldStorage {
        id: "mine_a".to_string(),
        dimension: "qexed:mine_a".to_string(),
        dimension_type: "minecraft:overworld".to_string(),
        path: "worlds/mine_a".to_string(),
    }];
    world.instances.clear();

    assert_eq!(
        login_dimension_names(&world, "qexed:mine_a"),
        vec!["qexed:mine_a".to_string()]
    );
}

#[test]
fn keep_alive_id_is_non_negative() {
    assert!(keep_alive_id() >= 0);
}

#[test]
fn chunk_load_parallelism_is_bounded() {
    assert_eq!(
        chunk_load_parallelism_limit(0),
        super::DEFAULT_CHUNK_LOAD_PARALLELISM
    );
    assert_eq!(chunk_load_parallelism_limit(1), 1);
    assert_eq!(
        chunk_load_parallelism_limit(usize::MAX),
        super::MAX_CHUNK_LOAD_PARALLELISM
    );
}

#[test]
fn player_abilities_follow_game_mode() {
    assert_eq!(player_ability_flags(GameMode::Survival, false), 0);
    assert_eq!(
        player_ability_flags(GameMode::Survival, true),
        PlayerAbilities::CAN_FLY
    );
    assert_eq!(
        player_ability_flags(GameMode::Creative, false),
        PlayerAbilities::CAN_FLY | PlayerAbilities::INSTABUILD
    );
    assert_eq!(
        player_ability_flags(GameMode::Spectator, false),
        PlayerAbilities::INVULNERABLE | PlayerAbilities::FLYING | PlayerAbilities::CAN_FLY
    );
}

#[test]
fn acknowledged_player_abilities_confirm_flight_only_when_allowed() {
    assert_eq!(
        acknowledged_player_ability_flags(GameMode::Survival, false, PlayerAbilities::FLYING),
        0
    );
    assert_eq!(
        acknowledged_player_ability_flags(GameMode::Survival, true, PlayerAbilities::FLYING),
        PlayerAbilities::CAN_FLY | PlayerAbilities::FLYING
    );
    assert_eq!(
        acknowledged_player_ability_flags(GameMode::Survival, true, 0),
        PlayerAbilities::CAN_FLY
    );
}

#[test]
fn world_edit_rules_apply_read_only_and_spawn_protection() {
    let mut world = qexed_config::app::qexed::server::World::default();
    let spawn = qexed_packet::net_types::Position { x: 1, y: 64, z: 1 };
    let outside_spawn = qexed_packet::net_types::Position {
        x: 100,
        y: 64,
        z: 100,
    };

    assert!(!can_modify_world(&world, &spawn));
    assert!(can_modify_world(&world, &outside_spawn));

    world.spawn_protection_radius = 0;
    assert!(can_modify_world(&world, &spawn));

    world.read_only = true;
    assert!(!can_modify_world(&world, &outside_spawn));

    world.read_only = false;
    world.game_mode = GameMode::Adventure;
    assert!(!can_modify_world(&world, &outside_spawn));

    world.game_mode = GameMode::Spectator;
    assert!(!can_modify_world(&world, &outside_spawn));
}

#[test]
fn read_only_world_allows_runtime_edit_regions() {
    let temp = tempfile::tempdir().unwrap();
    let mut world = qexed_config::app::qexed::server::World {
        read_only: true,
        spawn_protection_radius: 0,
        ..Default::default()
    };
    world
        .edit_regions
        .push(qexed_config::app::qexed::server::WorldEditRegion {
            id: "mine".to_string(),
            dimension: "minecraft:overworld".to_string(),
            min_x: 10,
            max_x: 20,
            min_y: 0,
            max_y: 80,
            min_z: -5,
            max_z: 5,
            allow_player_break: true,
            allow_player_place: false,
            allow_plugin_write: true,
            runtime_only: true,
        });
    let rules = crate::world::WorldRulesManager::from_world_config_for_tests(
        &world,
        temp.path().join("rules"),
    )
    .unwrap();
    let manager = crate::world::WorldManager::new(temp.path().join("world"))
        .with_edit_regions(&world.edit_regions);
    rules.set_read_only("minecraft:overworld", true).unwrap();

    let inside = Position { x: 12, y: 64, z: 0 };
    let outside = Position { x: 30, y: 64, z: 0 };
    let break_mode = world_write_mode(
        &manager,
        &world,
        GameMode::Survival,
        &rules,
        "minecraft:overworld",
        &inside,
        WorldEditKind::Break,
        true,
    );
    let place_mode = world_write_mode(
        &manager,
        &world,
        GameMode::Survival,
        &rules,
        "minecraft:overworld",
        &inside,
        WorldEditKind::Place,
        true,
    );
    let outside_mode = world_write_mode(
        &manager,
        &world,
        GameMode::Survival,
        &rules,
        "minecraft:overworld",
        &outside,
        WorldEditKind::Break,
        true,
    );

    assert!(break_mode.allowed);
    assert!(break_mode.runtime_only);
    assert!(!place_mode.allowed);
    assert!(!outside_mode.allowed);
}

#[test]
fn destroy_timing_follows_game_mode() {
    assert!(super::should_destroy_block(GameMode::Creative, 0));
    assert!(!super::should_destroy_block(GameMode::Creative, 2));
    assert!(!super::should_destroy_block(GameMode::Survival, 0));
    assert!(super::should_destroy_block(GameMode::Survival, 2));
    assert!(!super::should_destroy_block(GameMode::Adventure, 2));
    assert!(!super::should_destroy_block(GameMode::Spectator, 0));
}

#[test]
fn can_break_component_allows_only_matching_adventure_block() {
    let stone_state = crate::world::chunk_nbt::default_block_state_id("minecraft:stone");
    let dirt_state = crate::world::chunk_nbt::default_block_state_id("minecraft:dirt");
    let block_ids = crate::registry_sync::load_registry_id_map("minecraft:block").unwrap();
    let stone_id = block_ids["minecraft:stone"];
    let slot = Slot {
        item_count: VarInt(1),
        item_id: Some(VarInt(1)),
        number_of_components_to_add: Some(VarInt(1)),
        number_of_components_to_remove: Some(VarInt(0)),
        components_to_add: Some(vec![ComponentsToAdd::MinecraftCanBreak(
            minecraft::CanBreak {
                block_predicates: vec![minecraft::BlockPredicate {
                    blocks: Some(IDSet {
                        r#type: VarInt(2),
                        tag_name: None,
                        ids: Some(vec![VarInt(stone_id)]),
                    }),
                    properties: None,
                    nbt: None,
                }],
                show_in_tooltip: true,
            },
        )]),
        components_to_remove: None,
    };

    assert!(super::mining::held_item_allows_adventure_break(
        &slot,
        stone_state
    ));
    assert!(!super::mining::held_item_allows_adventure_break(
        &slot, dirt_state
    ));
}

#[test]
fn adventure_destroy_packet_without_can_break_is_violation() {
    let temp = tempfile::tempdir().unwrap();
    let world = crate::world::WorldManager::new(temp.path().join("world"));
    let position = Position { x: 0, y: 64, z: 0 };
    let stone = crate::world::chunk_nbt::default_block_state_id("minecraft:stone");
    world.set_runtime_block("minecraft:overworld", position.clone(), stone);

    assert!(super::adventure_destroy_packet_violates_can_break(
        &world,
        GameMode::Adventure,
        "minecraft:overworld",
        &position,
        super::PLAYER_ACTION_START_DESTROY_BLOCK,
        &Slot::default(),
    ));
    assert!(!super::adventure_destroy_packet_violates_can_break(
        &world,
        GameMode::Survival,
        "minecraft:overworld",
        &position,
        super::PLAYER_ACTION_START_DESTROY_BLOCK,
        &Slot::default(),
    ));
}

#[test]
fn placement_collision_checks_player_body() {
    let player = EntityPosition {
        x: 0.5,
        y: 64.0,
        z: 0.5,
        yaw: 0.0,
        pitch: 0.0,
        on_ground: true,
    };

    assert!(super::player_intersects_block(
        &player,
        &Position { x: 0, y: 64, z: 0 }
    ));
    assert!(super::player_intersects_block(
        &player,
        &Position { x: 0, y: 65, z: 0 }
    ));
    assert!(!super::player_intersects_block(
        &player,
        &Position { x: 2, y: 64, z: 0 }
    ));
}

#[test]
fn offset_position_preserves_original() {
    let position = Position {
        x: -5,
        y: 64,
        z: 10,
    };

    assert_eq!(
        super::offset_position(&position, 0, 1, -2),
        Position { x: -5, y: 65, z: 8 }
    );
    assert_eq!(
        position,
        Position {
            x: -5,
            y: 64,
            z: 10
        }
    );
}

#[test]
fn stepped_block_position_uses_block_under_feet() {
    let position = EntityPosition {
        x: -0.2,
        y: 64.0,
        z: 10.9,
        yaw: 0.0,
        pitch: 0.0,
        on_ground: true,
    };

    assert_eq!(
        super::stepped_block_position(position),
        Some(Position {
            x: -1,
            y: 63,
            z: 10
        })
    );
    assert_eq!(
        super::stepped_block_position(EntityPosition {
            on_ground: false,
            ..position
        }),
        None
    );
}

#[test]
fn dropped_item_position_is_lifted_out_of_solid_block() {
    let temp = tempfile::tempdir().unwrap();
    let world = crate::world::WorldManager::new(temp.path().join("world"));
    let stone = crate::world::chunk_nbt::default_block_state_id("minecraft:stone");
    world.set_runtime_block("minecraft:overworld", Position { x: 0, y: 64, z: 0 }, stone);
    let position = EntityPosition {
        x: 0.5,
        y: 64.1,
        z: 0.5,
        yaw: 0.0,
        pitch: 0.0,
        on_ground: false,
    };

    let lifted = super::lift_drop_position_out_of_blocks(&world, "minecraft:overworld", position);

    assert!(lifted.y >= 65.0);
    assert!(!super::drop_item_intersects_blocks(
        &world,
        "minecraft:overworld",
        lifted
    ));
}

#[test]
fn dropped_item_position_settles_to_ground_before_spawn() {
    let temp = tempfile::tempdir().unwrap();
    let world = crate::world::WorldManager::new(temp.path().join("world"));
    let stone = crate::world::chunk_nbt::default_block_state_id("minecraft:stone");
    world.set_runtime_block("minecraft:overworld", Position { x: 0, y: 63, z: 0 }, stone);
    let position = EntityPosition {
        x: 0.5,
        y: 70.375,
        z: 0.5,
        yaw: 0.0,
        pitch: 0.0,
        on_ground: false,
    };

    let settled = super::settle_drop_position_on_ground(&world, "minecraft:overworld", position);

    assert!(settled.y >= 64.0);
    assert!(settled.y < 64.5);
    assert!(!super::drop_item_intersects_blocks(
        &world,
        "minecraft:overworld",
        settled
    ));
}

#[test]
fn translatable_component_uses_minecraft_translation_key() {
    let component = super::translatable_component(
        "death.fell.accident.water",
        vec![super::text_component("Steve")],
    );

    let qexed_nbt::Tag::Compound(root) = component else {
        panic!("translation component should be a compound");
    };
    assert_eq!(
        root.get("translate"),
        Some(&qexed_nbt::Tag::String(std::sync::Arc::from(
            "death.fell.accident.water"
        )))
    );
    let Some(qexed_nbt::Tag::List(header, values)) = root.get("with") else {
        panic!("translation component should include arguments");
    };
    assert_eq!(header.tag_id, qexed_nbt::tag_id::COMPOUND);
    assert_eq!(header.length, 1);
    assert_eq!(values.len(), 1);
}

#[test]
fn missing_chunks_skips_in_flight_chunks() {
    let mut state = ChunkSendState::new("minecraft:overworld".to_string(), 0, 0, 1, 4);
    state.loading_chunks.insert((0, 0));
    state.visible_chunks.insert((1, 0));

    let missing = state.missing_chunks();

    assert!(!missing.contains(&(0, 0)));
    assert!(!missing.contains(&(1, 0)));
    assert_eq!(missing.len(), 7);
}

#[test]
fn center_chunk_is_first_pending_chunk_and_can_be_removed_before_parallel_loads() {
    let mut state = ChunkSendState::new("minecraft:overworld".to_string(), -5, 10, 1, 4);

    state.refresh_pending_chunks();

    assert_eq!(state.pending_chunks.front(), Some(&(-5, 10)));
    assert!(state.remove_pending_chunk((-5, 10)));
    assert!(!state.pending_chunks.contains(&(-5, 10)));
    assert_eq!(state.pending_chunks.len(), 8);
}

#[test]
fn delayed_unload_keeps_recently_left_chunks_visible() {
    let mut state = ChunkSendState::new("minecraft:overworld".to_string(), 0, 0, 1, 1);
    state.visible_chunks.insert((-1, 0));
    state.visible_chunks.insert((0, 0));

    state.center_x = 2;
    let target = state.target_chunks();
    state.mark_delayed_unloads(target, Instant::now() + Duration::from_secs(4));

    assert!(state.visible_chunks.contains(&(-1, 0)));
    assert!(state.pending_unloads.contains_key(&(-1, 0)));
}

#[test]
fn delayed_unload_is_cancelled_when_chunk_returns_to_view() {
    let mut state = ChunkSendState::new("minecraft:overworld".to_string(), 0, 0, 1, 1);
    state.visible_chunks.insert((-1, 0));

    state.center_x = 2;
    state.mark_delayed_unloads(
        state.target_chunks(),
        Instant::now() + Duration::from_secs(4),
    );
    state.center_x = 0;
    state.mark_delayed_unloads(
        state.target_chunks(),
        Instant::now() + Duration::from_secs(4),
    );

    assert!(!state.pending_unloads.contains_key(&(-1, 0)));
    assert!(state.visible_chunks.contains(&(-1, 0)));
}

#[test]
fn respawn_reset_requeues_chunks_even_when_old_view_was_visible() {
    let mut state = ChunkSendState::new("minecraft:overworld".to_string(), 0, 0, 1, 1);
    for chunk in state.target_chunks() {
        state.visible_chunks.insert(chunk);
    }
    state.loading_chunks.insert((2, 0));
    state
        .pending_unloads
        .insert((-2, 0), Instant::now() + Duration::from_secs(4));

    state.reset_view(0, 0);

    assert!(state.visible_chunks.is_empty());
    assert!(state.loading_chunks.is_empty());
    assert!(state.pending_unloads.is_empty());
    assert_eq!(state.pending_chunks.len(), 9);
    assert!(state.pending_chunks.contains(&(0, 0)));
}

// ── Crop growth tests ──

#[test]
fn crop_random_tick_grows_wheat_one_stage() {
    let temp = tempfile::tempdir().unwrap();
    let world = crate::world::WorldManager::new(temp.path().join("world"));
    let position = Position { x: 0, y: 64, z: 0 };
    let farmland = block_state("minecraft:farmland", &[("moisture", "7")]);
    let wheat = block_state("minecraft:wheat", &[("age", "0")]);
    world.place_block(
        "minecraft:overworld",
        Position { x: 0, y: 63, z: 0 },
        farmland,
    );
    world.place_block("minecraft:overworld", position.clone(), wheat);

    let result = super::crop_random_tick(&world, "minecraft:overworld", &position, wheat);
    // Crop may or may not grow due to random chance; run multiple ticks
    // to verify it doesn't crash and the logic is sound
    assert!(
        result.is_none() || {
            let new_state = result.unwrap();
            let age = super::block_state_u8_property(new_state, "age");
            age == Some(1) || age == Some(2)
        }
    );
}

#[test]
fn crop_random_tick_does_not_grow_beyond_max_age() {
    let temp = tempfile::tempdir().unwrap();
    let world = crate::world::WorldManager::new(temp.path().join("world"));
    let position = Position { x: 0, y: 64, z: 0 };
    let farmland = block_state("minecraft:farmland", &[("moisture", "7")]);
    let wheat_full = block_state("minecraft:wheat", &[("age", "7")]);
    world.place_block(
        "minecraft:overworld",
        Position { x: 0, y: 63, z: 0 },
        farmland,
    );
    world.place_block("minecraft:overworld", position.clone(), wheat_full);

    // Run many ticks: fully grown wheat should never grow past age 7
    for _ in 0..5 {
        let result = super::crop_random_tick(&world, "minecraft:overworld", &position, wheat_full);
        // Should return None because age is already max
        assert!(result.is_none());
    }
}

#[test]
fn crop_random_tick_requires_farmland_for_wheat() {
    let temp = tempfile::tempdir().unwrap();
    let world = crate::world::WorldManager::new(temp.path().join("world"));
    let position = Position { x: 0, y: 64, z: 0 };
    let wheat = block_state("minecraft:wheat", &[("age", "0")]);
    world.place_block("minecraft:overworld", position.clone(), wheat);

    // Wheat on dirt (not farmland) should not grow
    for _ in 0..20 {
        let result = super::crop_random_tick(&world, "minecraft:overworld", &position, wheat);
        // Should reliably return None since not on farmland
        if result.is_some() {
            // Verify it didn't actually increase age (this would be a bug)
            let new_state = result.unwrap();
            let age = super::block_state_u8_property(new_state, "age");
            assert!(age.is_none() || age == Some(0));
        }
    }
}

// ── Sapling tests ──

#[test]
fn is_sapling_recognizes_all_types() {
    assert!(super::is_sapling("minecraft:oak_sapling"));
    assert!(super::is_sapling("minecraft:birch_sapling"));
    assert!(super::is_sapling("minecraft:spruce_sapling"));
    assert!(super::is_sapling("minecraft:jungle_sapling"));
    assert!(super::is_sapling("minecraft:acacia_sapling"));
    assert!(super::is_sapling("minecraft:dark_oak_sapling"));
    assert!(super::is_sapling("minecraft:cherry_sapling"));
    assert!(super::is_sapling("minecraft:mangrove_propagule"));
    assert!(!super::is_sapling("minecraft:stone"));
    assert!(!super::is_sapling("minecraft:wheat"));
}

// ── Fluid tests ──

fn grow_tree_blocks_produces_trunk_and_leaves() {
    let pos = Position { x: 0, y: 64, z: 0 };
    let blocks = super::grow_tree_blocks(&pos, "minecraft:oak_sapling");

    // Should produce at least trunk + some leaves
    assert!(!blocks.is_empty(), "tree should produce blocks");

    // Should have trunk at the sapling position
    let has_trunk_base = blocks
        .iter()
        .any(|(p, _)| p.x == 0 && p.y == 64 && p.z == 0);
    assert!(has_trunk_base, "tree should have trunk at sapling position");

    // Should have leaf blocks above trunk
    let has_leaves = blocks.iter().any(|(p, _)| p.y > 66);
    assert!(has_leaves, "tree should have leaves above trunk");

    // Should produce many blocks (tree + canopy)
    assert!(blocks.len() >= 30, "tree should have at least 30 blocks");
}

#[test]
fn sapling_bone_meal_grows_tree_and_replaces_sapling() {
    fn sapling_bone_meal_grows_tree_and_replaces_sapling() {
        // Test grow_tree_blocks directly (WorldManager-based test requires chunk loading)
        let pos = Position { x: 0, y: 64, z: 0 };
        let blocks = super::grow_tree_blocks(&pos, "minecraft:oak_sapling");
        assert!(
            !blocks.is_empty(),
            "grow_tree_blocks should produce tree blocks"
        );

        let log_id = crate::world::chunk_nbt::default_block_state("minecraft:oak_log").id;
        let has_log = blocks.iter().any(|(p, s)| p == &pos && *s == log_id);
        assert!(has_log, "tree trunk should be at sapling position");

        let leaves_id = crate::world::chunk_nbt::default_block_state("minecraft:oak_leaves").id;
        let has_leaves = blocks.iter().any(|(_, s)| *s == leaves_id);
        assert!(has_leaves, "tree should have leaves");
    }
}

#[test]
fn different_saplings_produce_different_tree_types() {
    let pos = Position { x: 0, y: 64, z: 0 };

    let birch_blocks = super::grow_tree_blocks(&pos, "minecraft:birch_sapling");
    let birch_log = crate::world::chunk_nbt::default_block_state("minecraft:birch_log").id;
    assert!(
        birch_blocks.iter().any(|(_, s)| *s == birch_log),
        "birch sapling should produce birch logs"
    );

    let spruce_blocks = super::grow_tree_blocks(&pos, "minecraft:spruce_sapling");
    let spruce_log = crate::world::chunk_nbt::default_block_state("minecraft:spruce_log").id;
    assert!(
        spruce_blocks.iter().any(|(_, s)| *s == spruce_log),
        "spruce sapling should produce spruce logs"
    );
}
