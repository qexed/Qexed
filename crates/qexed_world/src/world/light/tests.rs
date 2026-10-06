//! v4 `world/tests.rs` 中光照部分的迁移（v4 文件其余测试依赖
//! WorldManager / generator trait，归 world-core 任务，见 world/tests.rs）。

use super::{
    CHUNK_DAMPENING_LEN, LIGHT_SECTION_COUNT, LIGHT_ARRAY_BYTES, Light, WorldLightAlgorithm,
    WorldLightMode, block_light_dampening_index, light_for_mode, section_count,
    sky_light_from_dampening, sky_light_from_neighbourhood, LightDampeningNeighborhood,
};
use qexed_packet::{Packet, PacketCodec, PacketWriter};
use qexed_protocol::to_client::play::light_update::LightUpdateData;

use crate::world::{WORLD_MAX_Y, OVERWORLD_HEIGHT};

#[test]
fn empty_network_chunk_serializes() {
    let chunk = super::empty_chunk_packet(0, 0, WorldLightMode::Static);
    let mut payload = bytes::BytesMut::new();
    let mut writer = PacketWriter::new(&mut payload);

    chunk.serialize(&mut writer).unwrap();

    assert!(!payload.is_empty());
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
            .flat_map(|layer| layer.0.iter().copied())
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
            .flat_map(|layer| layer.0.iter().copied())
            .all(|value| value == 0x77)
    );
    assert!(
        light
            .block_light_arrays
            .iter()
            .flat_map(|layer| layer.0.iter().copied())
            .all(|value| value == 0x77)
    );
}

#[test]
fn light_layers_serialize_with_minecraft_byte_array_lengths() {
    let update: LightUpdateData = light_for_mode(WorldLightMode::Static).into();
    let mut payload = bytes::BytesMut::new();
    let mut writer = PacketWriter::new(&mut payload);

    update.serialize(&mut writer).unwrap();

    // v4 断言（map_chunk::Light）：mask(9+1+1+9) + sky(1 + 26*(2+2048)) + block(1)。
    // v6 LightUpdateData 的 Bitset 仍是 long 数组前缀，布局一致。
    let masks_len = 9 + 1 + 1 + 9;
    let per_layer_len_prefix = 2;
    let sky_arrays_len = 1 + LIGHT_SECTION_COUNT * (per_layer_len_prefix + LIGHT_ARRAY_BYTES);
    let block_arrays_len = 1;
    assert_eq!(payload.len(), masks_len + sky_arrays_len + block_arrays_len);
}

#[test]
fn level_chunk_light_encoding_uses_26_3_bitset_bytes() {
    // 26.3：区块包内嵌光照的 BIT_SET 是 VarInt 长度 + 字节串（非 long 数组）。
    let packet: qexed_protocol::to_client::play::level_chunk_with_light::LightUpdatePacketData =
        light_for_mode(WorldLightMode::Static).into();

    // 26 层 → bit 0..26 → 4 字节；非空 mask 全 1。
    assert_eq!(packet.sky_y_mask.0.len(), 4);
    assert_eq!(packet.sky_y_mask.0, vec![0xff, 0xff, 0xff, 0x03]);
    assert!(packet.empty_sky_y_mask.0.is_empty());
    assert_eq!(packet.sky_updates.len(), LIGHT_SECTION_COUNT);
    assert!(packet.sky_updates.iter().all(|layer| layer.0.len() == LIGHT_ARRAY_BYTES));
    // block level = 0 → block mask 为空、层为空。
    assert!(packet.block_y_mask.0.is_empty());
    assert!(packet.block_updates.is_empty());
}

#[test]
fn sky_light_dampens_through_water_column() {
    let mut dampening = vec![0; 16 * OVERWORLD_HEIGHT as usize * 16];
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
    let mut dampening = vec![0; 16 * OVERWORLD_HEIGHT as usize * 16];
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
    let mut dampening = vec![0; 16 * OVERWORLD_HEIGHT as usize * 16];
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
    let mut center = vec![0; CHUNK_DAMPENING_LEN];
    let east = vec![0; CHUNK_DAMPENING_LEN];
    let mut blocked_east = vec![0; CHUNK_DAMPENING_LEN];
    for block_y in y + 1..=WORLD_MAX_Y {
        for x in 0..16 {
            for z in 0..16 {
                center[block_light_dampening_index(x, block_y, z)] = 15;
                blocked_east[block_light_dampening_index(x, block_y, z)] = 15;
            }
        }
    }

    let mut blocked = LightDampeningNeighborhood::single(&center);
    blocked.set(1, 0, blocked_east);
    let blocked = sky_light_from_neighbourhood(&blocked, WorldLightAlgorithm::RayTrace);
    let mut neighbourhood = LightDampeningNeighborhood::single(&center);
    neighbourhood.set(1, 0, east);
    let traced = sky_light_from_neighbourhood(&neighbourhood, WorldLightAlgorithm::RayTrace);
    let blocked_layers = layers_by_mask(&blocked);
    let traced_layers = layers_by_mask(&traced);

    assert!(light_at(&traced_layers, 15, y, 8) > light_at(&blocked_layers, 15, y, 8));
}

#[test]
fn fast_sky_light_uses_neighbouring_chunk_edges() {
    let y = 80;
    let mut center = vec![0; CHUNK_DAMPENING_LEN];
    let east = vec![0; CHUNK_DAMPENING_LEN];
    let mut blocked_east = vec![0; CHUNK_DAMPENING_LEN];
    for block_y in y + 1..=WORLD_MAX_Y {
        for x in 0..16 {
            for z in 0..16 {
                center[block_light_dampening_index(x, block_y, z)] = 15;
                blocked_east[block_light_dampening_index(x, block_y, z)] = 15;
            }
        }
    }

    let mut blocked = LightDampeningNeighborhood::single(&center);
    blocked.set(1, 0, blocked_east);
    let blocked = sky_light_from_neighbourhood(&blocked, WorldLightAlgorithm::Fast);
    let mut neighbourhood = LightDampeningNeighborhood::single(&center);
    neighbourhood.set(1, 0, east);
    let fast = sky_light_from_neighbourhood(&neighbourhood, WorldLightAlgorithm::Fast);
    let blocked_layers = layers_by_mask(&blocked);
    let fast_layers = layers_by_mask(&fast);

    assert!(
        light_at_or_zero(&fast_layers, 15, y, 8) > light_at_or_zero(&blocked_layers, 15, y, 8)
    );
}

#[test]
fn empty_chunk_section_bytes_matches_section_count() {
    assert_eq!(
        super::empty_chunk_section_bytes().len(),
        section_count() as usize * 8
    );
}

#[test]
fn empty_heightmaps_cover_three_types() {
    let heightmaps = super::empty_heightmaps();
    assert_eq!(heightmaps.len(), 3);
}

fn nibble_at(layer: &qexed_protocol::to_client::play::light_update::LightArray, index: usize) -> u8 {
    let value = layer.0[index / 2];
    if index % 2 == 0 {
        value & 0x0f
    } else {
        value >> 4
    }
}

fn layers_by_mask(
    light: &Light,
) -> Vec<Option<&qexed_protocol::to_client::play::light_update::LightArray>> {
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
    layers: &[Option<&qexed_protocol::to_client::play::light_update::LightArray>],
    x: usize,
    y: i32,
    z: usize,
) -> u8 {
    let section_index = y.div_euclid(crate::world::SECTION_HEIGHT)
        - crate::world::MIN_LIGHT_SECTION_Y;
    let layer = layers[section_index as usize].expect("expected light layer");
    let local_y = y.rem_euclid(crate::world::SECTION_HEIGHT) as usize;
    let block_index = local_y * 16 * 16 + z * 16 + x;
    nibble_at(layer, block_index)
}

fn light_at_or_zero(
    layers: &[Option<&qexed_protocol::to_client::play::light_update::LightArray>],
    x: usize,
    y: i32,
    z: usize,
) -> u8 {
    let section_index = y.div_euclid(crate::world::SECTION_HEIGHT)
        - crate::world::MIN_LIGHT_SECTION_Y;
    let Some(layer) = layers[section_index as usize] else {
        return 0;
    };
    let local_y = y.rem_euclid(crate::world::SECTION_HEIGHT) as usize;
    let block_index = local_y * 16 * 16 + z * 16 + x;
    nibble_at(layer, block_index)
}
