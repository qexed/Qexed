use qexed_packet::{Packet, PacketCodec};

use super::{
    LIGHT_SECTION_COUNT, WORLD_MAX_Y, WorldLightAlgorithm, WorldLightMode, WorldManager,
    block_light_dampening_index, empty_chunk_section_bytes, generator, light_for_mode,
    section_count, sky_light_from_dampening, sky_light_from_neighbourhood,
};
use qexed_protocol::to_client::play::map_chunk::LIGHT_ARRAY_BYTES;
use std::sync::Arc;

#[test]
fn empty_chunk_has_all_overworld_sections() {
    let bytes = empty_chunk_section_bytes();
    assert_eq!(bytes.len(), section_count() as usize * 8);
}

#[test]
fn empty_network_chunk_serializes() {
    let manager = WorldManager::new("world");
    let chunk = manager.network_chunk("minecraft:overworld", 0, 0).unwrap();
    let mut payload = bytes::BytesMut::new();
    let mut writer = qexed_packet::PacketWriter::new(&mut payload);

    chunk.serialize(&mut writer).unwrap();

    assert!(!payload.is_empty());
}

#[test]
fn generated_flat_chunk_is_used_when_save_chunk_is_missing() {
    let dir = tempfile::tempdir().unwrap();
    let manager = WorldManager::with_generator(
        dir.path(),
        WorldLightMode::Static,
        WorldLightAlgorithm::Fast,
        None,
        true,
        Arc::new(generator::VanillaFlatGenerator::from_preset(
            "minecraft:classic_flat",
        )),
    );
    let chunk = manager.network_chunk("minecraft:overworld", 0, 0).unwrap();
    let grass = super::chunk_nbt::default_block_state_id("minecraft:grass_block");
    let grass_position = qexed_packet::net_types::Position {
        x: 0,
        y: super::WORLD_MIN_Y + 3,
        z: 0,
    };
    let air_position = qexed_packet::net_types::Position {
        x: 0,
        y: super::WORLD_MIN_Y + 4,
        z: 0,
    };

    assert_eq!(
        manager.block_state_at("minecraft:overworld", &grass_position),
        Some(grass)
    );
    assert_eq!(
        manager.block_state_at("minecraft:overworld", &air_position),
        None
    );
    assert!(chunk.data.data.len() > section_count() as usize * 8);
}

#[test]
fn static_light_sends_full_sky_light_layers() {
    let light = light_for_mode(WorldLightMode::Static);

    assert_eq!(light.sky_light_arrays.len(), LIGHT_SECTION_COUNT);
    assert!(
        light
            .sky_light_arrays
            .iter()
            .all(|layer| layer.0.len() == LIGHT_ARRAY_BYTES)
    );
    assert!(
        light
            .sky_light_arrays
            .iter()
            .flat_map(|layer| layer.0)
            .all(|value| value == 0xff)
    );
    assert!(light.block_light_arrays.is_empty());
}

#[test]
fn fixed_light_sends_constant_sky_and_block_light() {
    let light = light_for_mode(WorldLightMode::Fixed(7));

    assert_eq!(light.sky_light_arrays.len(), LIGHT_SECTION_COUNT);
    assert_eq!(light.block_light_arrays.len(), LIGHT_SECTION_COUNT);
    assert!(
        light
            .sky_light_arrays
            .iter()
            .flat_map(|layer| layer.0)
            .all(|value| value == 0x77)
    );
    assert!(
        light
            .block_light_arrays
            .iter()
            .flat_map(|layer| layer.0)
            .all(|value| value == 0x77)
    );
}

#[test]
fn light_layers_serialize_with_minecraft_byte_array_lengths() {
    let light = light_for_mode(WorldLightMode::Static);
    let mut payload = bytes::BytesMut::new();
    let mut writer = qexed_packet::PacketWriter::new(&mut payload);

    light.serialize(&mut writer).unwrap();

    let masks_len = 9 + 1 + 1 + 9;
    let per_layer_len_prefix = 2;
    let sky_arrays_len = 1 + LIGHT_SECTION_COUNT * (per_layer_len_prefix + LIGHT_ARRAY_BYTES);
    let block_arrays_len = 1;
    assert_eq!(payload.len(), masks_len + sky_arrays_len + block_arrays_len);
}

#[test]
fn sky_light_dampens_through_water_column() {
    let mut dampening = vec![0; 16 * super::OVERWORLD_HEIGHT as usize * 16];
    dampening[block_light_dampening_index(0, WORLD_MAX_Y, 0)] = 1;
    let light = sky_light_from_dampening(&dampening, WorldLightAlgorithm::Fast);

    assert_eq!(
        light.sky_light_mask.0[0].count_ones() as usize,
        light.sky_light_arrays.len()
    );
    assert_eq!(
        nibble_at(&light.sky_light_arrays[LIGHT_SECTION_COUNT - 2], 0),
        15
    );
    assert_eq!(
        nibble_at(&light.sky_light_arrays[LIGHT_SECTION_COUNT - 3], 0),
        14
    );
}

#[test]
fn sky_light_spreads_under_solid_block() {
    let y = 80;
    let mut dampening = vec![0; 16 * super::OVERWORLD_HEIGHT as usize * 16];
    dampening[block_light_dampening_index(8, y + 1, 8)] = 15;
    let light = sky_light_from_dampening(&dampening, WorldLightAlgorithm::Fast);
    let layers = layers_by_mask(&light);
    let under_block = light_at(&layers, 8, y, 8);
    let adjacent_open = light_at(&layers, 7, y, 8);

    assert!(under_block > 0);
    assert!(under_block < adjacent_open);
}

#[test]
fn ray_trace_sky_light_can_enter_from_diagonal_opening() {
    let y = 80;
    let mut dampening = vec![0; 16 * super::OVERWORLD_HEIGHT as usize * 16];
    for block_y in y + 1..=WORLD_MAX_Y {
        dampening[block_light_dampening_index(8, block_y, 8)] = 15;
    }
    let fast = sky_light_from_dampening(&dampening, WorldLightAlgorithm::Fast);
    let traced = sky_light_from_dampening(&dampening, WorldLightAlgorithm::RayTrace);
    let fast_layers = layers_by_mask(&fast);
    let traced_layers = layers_by_mask(&traced);

    assert!(light_at(&traced_layers, 8, y, 8) > light_at(&fast_layers, 8, y, 8));
}

#[test]
fn ray_trace_sky_light_uses_neighbouring_chunk_openings() {
    let y = 80;
    let mut center = vec![0; super::CHUNK_DAMPENING_LEN];
    let east = vec![0; super::CHUNK_DAMPENING_LEN];
    let mut blocked_east = vec![0; super::CHUNK_DAMPENING_LEN];
    for block_y in y + 1..=WORLD_MAX_Y {
        for x in 0..16 {
            for z in 0..16 {
                center[block_light_dampening_index(x, block_y, z)] = 15;
                blocked_east[block_light_dampening_index(x, block_y, z)] = 15;
            }
        }
    }

    let mut blocked = super::LightDampeningNeighborhood::single(&center);
    blocked.set(1, 0, blocked_east);
    let blocked = sky_light_from_neighbourhood(&blocked, WorldLightAlgorithm::RayTrace);
    let mut neighbourhood = super::LightDampeningNeighborhood::single(&center);
    neighbourhood.set(1, 0, east);
    let traced = sky_light_from_neighbourhood(&neighbourhood, WorldLightAlgorithm::RayTrace);
    let blocked_layers = layers_by_mask(&blocked);
    let traced_layers = layers_by_mask(&traced);

    assert!(light_at(&traced_layers, 15, y, 8) > light_at(&blocked_layers, 15, y, 8));
}

#[test]
fn fast_sky_light_uses_neighbouring_chunk_edges() {
    let y = 80;
    let mut center = vec![0; super::CHUNK_DAMPENING_LEN];
    let east = vec![0; super::CHUNK_DAMPENING_LEN];
    let mut blocked_east = vec![0; super::CHUNK_DAMPENING_LEN];
    for block_y in y + 1..=WORLD_MAX_Y {
        for x in 0..16 {
            for z in 0..16 {
                center[block_light_dampening_index(x, block_y, z)] = 15;
                blocked_east[block_light_dampening_index(x, block_y, z)] = 15;
            }
        }
    }

    let mut blocked = super::LightDampeningNeighborhood::single(&center);
    blocked.set(1, 0, blocked_east);
    let blocked = sky_light_from_neighbourhood(&blocked, WorldLightAlgorithm::Fast);
    let mut neighbourhood = super::LightDampeningNeighborhood::single(&center);
    neighbourhood.set(1, 0, east);
    let fast = sky_light_from_neighbourhood(&neighbourhood, WorldLightAlgorithm::Fast);
    let blocked_layers = layers_by_mask(&blocked);
    let fast_layers = layers_by_mask(&fast);

    assert!(light_at_or_zero(&fast_layers, 15, y, 8) > light_at_or_zero(&blocked_layers, 15, y, 8));
}

#[test]
fn world_manager_writes_region_chunks() {
    let dir = tempfile::tempdir().unwrap();
    let manager = WorldManager::new(dir.path());
    let chunk = super::region::ChunkData::zlib(b"chunk").unwrap();

    manager
        .write_region_chunk("minecraft:overworld", 0, 0, chunk)
        .unwrap();

    let loaded = manager
        .load_region_chunk("minecraft:overworld", 0, 0)
        .unwrap()
        .unwrap();
    assert_eq!(loaded.decompress().unwrap(), b"chunk");
}

#[test]
fn world_manager_loads_block_state_from_saved_region() {
    let dir = tempfile::tempdir().unwrap();
    let manager = WorldManager::new(dir.path());
    let stone = super::chunk_nbt::default_block_state_id("minecraft:stone");
    let position = qexed_packet::net_types::Position {
        x: -17,
        y: 12,
        z: 35,
    };
    let chunk =
        super::chunk_nbt::set_block_state_in_region(-2, 2, None, &position, stone, None).unwrap();

    manager
        .write_region_chunk("minecraft:overworld", -2, 2, chunk)
        .unwrap();

    assert_eq!(
        manager.block_state_at("minecraft:overworld", &position),
        Some(stone)
    );
}

#[test]
fn world_manager_persists_placed_blocks_to_region() {
    let dir = tempfile::tempdir().unwrap();
    let manager = WorldManager::new(dir.path());
    let stone = super::chunk_nbt::default_block_state_id("minecraft:stone");
    let position = qexed_packet::net_types::Position { x: 3, y: 70, z: 4 };

    manager.place_block("minecraft:overworld", position.clone(), stone);

    let reloaded = WorldManager::new(dir.path());
    assert_eq!(
        reloaded.block_state_at("minecraft:overworld", &position),
        Some(stone)
    );
}

#[test]
fn world_manager_bulk_persists_placed_blocks_across_chunks() {
    let dir = tempfile::tempdir().unwrap();
    let manager = WorldManager::new(dir.path());
    let stone = super::chunk_nbt::default_block_state_id("minecraft:stone");
    let positions = [
        qexed_packet::net_types::Position { x: 15, y: 70, z: 0 },
        qexed_packet::net_types::Position { x: 16, y: 70, z: 0 },
        qexed_packet::net_types::Position {
            x: -1,
            y: 71,
            z: -1,
        },
    ];

    let updates = manager
        .place_blocks(
            "minecraft:overworld",
            positions.iter().cloned().map(|position| (position, stone)),
        )
        .unwrap();

    assert_eq!(updates.len(), positions.len());
    let reloaded = WorldManager::new(dir.path());
    for position in &positions {
        assert_eq!(
            reloaded.block_state_at("minecraft:overworld", position),
            Some(stone)
        );
    }
}

#[test]
fn world_manager_serializes_parallel_region_writes() {
    let dir = tempfile::tempdir().unwrap();
    let manager = WorldManager::new(dir.path());
    let stone = super::chunk_nbt::default_block_state_id("minecraft:stone");
    let first = qexed_packet::net_types::Position { x: 3, y: 70, z: 4 };
    let second = qexed_packet::net_types::Position { x: 19, y: 71, z: 4 };
    let first_writer = manager.clone();
    let second_writer = manager.clone();
    let first_position = first.clone();
    let second_position = second.clone();

    let first_thread = std::thread::spawn(move || {
        first_writer.place_block("minecraft:overworld", first_position, stone);
    });
    let second_thread = std::thread::spawn(move || {
        second_writer.place_block("minecraft:overworld", second_position, stone);
    });

    first_thread.join().unwrap();
    second_thread.join().unwrap();

    let reloaded = WorldManager::new(dir.path());
    assert_eq!(
        reloaded.block_state_at("minecraft:overworld", &first),
        Some(stone)
    );
    assert_eq!(
        reloaded.block_state_at("minecraft:overworld", &second),
        Some(stone)
    );
}

#[test]
fn world_manager_uses_nether_and_end_region_paths() {
    let dir = tempfile::tempdir().unwrap();
    let manager = WorldManager::new(dir.path());
    let stone = super::chunk_nbt::default_block_state_id("minecraft:stone");
    let nether_position = qexed_packet::net_types::Position { x: 1, y: 64, z: 1 };
    let end_position = qexed_packet::net_types::Position { x: 2, y: 64, z: 2 };

    manager.place_block("minecraft:the_nether", nether_position.clone(), stone);
    manager.place_block("minecraft:the_end", end_position.clone(), stone);

    assert!(dir.path().join("DIM-1/region/r.0.0.mca").exists());
    assert!(dir.path().join("DIM1/region/r.0.0.mca").exists());
    assert_eq!(
        manager.block_state_at("minecraft:the_nether", &nether_position),
        Some(stone)
    );
    assert_eq!(
        manager.block_state_at("minecraft:the_end", &end_position),
        Some(stone)
    );
}

#[test]
fn read_only_world_rejects_region_writes_and_block_changes() {
    let dir = tempfile::tempdir().unwrap();
    let manager = WorldManager::with_light_mode(
        dir.path(),
        WorldLightMode::Static,
        WorldLightAlgorithm::Fast,
        None,
        true,
    );
    let chunk = super::region::ChunkData::zlib(b"chunk").unwrap();
    let position = qexed_packet::net_types::Position { x: 1, y: 64, z: 1 };

    assert!(
        manager
            .write_region_chunk("minecraft:overworld", 0, 0, chunk)
            .is_err()
    );
    manager.place_block("minecraft:overworld", position.clone(), 1);

    assert_eq!(
        manager.block_state_at("minecraft:overworld", &position),
        None
    );
}

#[test]
fn ending_last_world_session_clears_chunk_light_cache() {
    let manager = WorldManager::new("world");
    let session = manager.begin_session();
    let epoch = manager.cache_epoch();

    manager.remember_chunk_light_dampening(
        "minecraft:overworld",
        0,
        0,
        vec![0; super::CHUNK_DAMPENING_LEN],
        Some(epoch),
    );
    assert_eq!(manager.cached_light_chunk_count(), 1);

    drop(session);

    assert_eq!(manager.cached_light_chunk_count(), 0);
    manager.remember_chunk_light_dampening(
        "minecraft:overworld",
        1,
        0,
        vec![0; super::CHUNK_DAMPENING_LEN],
        Some(epoch),
    );
    assert_eq!(manager.cached_light_chunk_count(), 0);
}

fn nibble_at(layer: &qexed_protocol::to_client::play::map_chunk::LightArray, index: usize) -> u8 {
    let value = layer.0[index / 2];
    if index % 2 == 0 {
        value & 0x0f
    } else {
        value >> 4
    }
}

fn layers_by_mask(
    light: &qexed_protocol::to_client::play::map_chunk::Light,
) -> Vec<Option<&qexed_protocol::to_client::play::map_chunk::LightArray>> {
    let mut layers = vec![None; LIGHT_SECTION_COUNT];
    let mask = light.sky_light_mask.0.first().copied().unwrap_or_default();
    let mut array_index = 0;
    for (section_index, slot) in layers.iter_mut().enumerate() {
        if mask & (1_u64 << section_index) != 0 {
            *slot = light.sky_light_arrays.get(array_index);
            array_index += 1;
        }
    }
    layers
}

fn light_at(
    layers: &[Option<&qexed_protocol::to_client::play::map_chunk::LightArray>],
    x: usize,
    y: i32,
    z: usize,
) -> u8 {
    let section_index = y.div_euclid(super::SECTION_HEIGHT) - super::MIN_LIGHT_SECTION_Y;
    let layer = layers[section_index as usize].expect("expected light layer");
    let local_y = y.rem_euclid(super::SECTION_HEIGHT) as usize;
    let block_index = local_y * 16 * 16 + z * 16 + x;
    nibble_at(layer, block_index)
}

fn light_at_or_zero(
    layers: &[Option<&qexed_protocol::to_client::play::map_chunk::LightArray>],
    x: usize,
    y: i32,
    z: usize,
) -> u8 {
    let section_index = y.div_euclid(super::SECTION_HEIGHT) - super::MIN_LIGHT_SECTION_Y;
    let Some(layer) = layers[section_index as usize] else {
        return 0;
    };
    let local_y = y.rem_euclid(super::SECTION_HEIGHT) as usize;
    let block_index = local_y * 16 * 16 + z * 16 + x;
    nibble_at(layer, block_index)
}
