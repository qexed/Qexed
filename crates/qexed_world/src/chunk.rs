//! 区块包构造：空区块（超平占位）的协议字节。
//!
//! 格式对齐 MC 26.1.2（协议 775），交叉验证自：
//! - v4 `world/light.rs::empty_chunk_packet`（已在真人客户端验证过的公式）
//! - Pumpkin `net/java/chunk_data/v1_18.rs::write_chunk_data`（26.1+ 分支）
//!
//! 关键协议点（26.1+）：
//! - section 头部为 **两个 i16**：非空气方块数 + 液体方块数
//! - 单值 palette（bits_per_entry = 0）**不携带数据数组**（1.21.5 起移除长度前缀）
//! - heightmaps 为 `[key: VarInt][len: VarInt][longs]` ×3 的 map（1.21.5 起非 NBT）
//! - 光照掩码覆盖 26 层：bit0 = 世界下方，bit1..=24 = 24 个 section，bit25 = 世界上方

use bytes::BytesMut;
use qexed_packet::net_types::{Bitset, VarInt};
use qexed_packet::{PacketCodec, PacketWriter};
use qexed_protocol::to_client::play::map_chunk::{
    Chunk, Heightmaps, Light, LightArray, MapChunk, LIGHT_ARRAY_BYTES,
};

/// 单个 section 的边长（恒 16）。
pub const SECTION_HEIGHT: i32 = 16;
/// 主世界最低方块 Y（1.18+）。
pub const WORLD_MIN_Y: i32 = -64;
/// 主世界总高度。
pub const WORLD_HEIGHT: i32 = 384;
/// 主世界 section 数：384 / 16 = 24。
pub const SECTION_COUNT: usize = (WORLD_HEIGHT / SECTION_HEIGHT) as usize;
/// 光照层数 = section 数 + 世界上下各一层。
pub const LIGHT_SECTION_COUNT: usize = SECTION_COUNT + 2;

/// 空气方块的 block state id（协议全局序号，来自 blocks.json 报告）。
const AIR_STATE_ID: i32 = 0;
/// 默认生物群系 minecraft:plains 的协议序号。
///
/// configuration 阶段的 dynamic 注册表（worldgen/biome）按 entry id 字母序发送，
/// plains 位于第 40 位；该值由
/// `qexed_registry::load_dynamic_registry_id_map` 的排序规则保证。
pub const PLAINS_BIOME_ID: i32 = 40;

/// 构造一个全空（空气 + plains 群系 + 静态满级天光）的区块包。
pub fn empty_chunk_packet(chunk_x: i32, chunk_z: i32) -> MapChunk {
    MapChunk {
        chunk_x,
        chunk_z,
        data: Chunk {
            heightmaps: empty_heightmaps(),
            data: empty_section_bytes(),
            block_entities: Vec::new(),
        },
        light: static_light(),
    }
}

/// 空 heightmaps：WORLD_SURFACE(1) / MOTION_BLOCKING(4) / NO_LEAVES(5)，各 37 个 long。
///
/// 全零表示“高度为 0”；配合满级天光，客户端不会因缺高度图而拒绝渲染。
pub fn empty_heightmaps() -> Vec<Heightmaps> {
    [1, 4, 5]
        .into_iter()
        .map(|type_id| Heightmaps {
            type_id: VarInt(type_id),
            data: vec![0; 37],
        })
        .collect()
}

/// 24 个空 section 的原始字节（不含外层长度前缀，由 `Chunk.data` 的 Vec 编码加）。
pub fn empty_section_bytes() -> Vec<u8> {
    let mut buf = BytesMut::new();
    let mut writer = PacketWriter::new(&mut buf);
    for _ in 0..SECTION_COUNT {
        write_empty_section(&mut writer).expect("empty section serialization is infallible");
    }
    buf.to_vec()
}

/// 单个空 section：0 非空气 + 0 液体 + 空气单值 palette + plains 单值 palette。
pub fn write_empty_section(writer: &mut PacketWriter) -> Result<(), qexed_packet::error::PacketError> {
    0_i16.serialize(writer)?; // 非空气方块数
    0_i16.serialize(writer)?; // 液体方块数（26.1 新增）
    write_single_value_palette(writer, AIR_STATE_ID)?;
    write_single_value_palette(writer, PLAINS_BIOME_ID)?;
    Ok(())
}

/// 单值 palette：bits_per_entry = 0 + VarInt 注册表 id。
///
/// 26.1+ 协议下单值 palette 不再携带数据数组（1.21.5 起移除长度前缀）。
fn write_single_value_palette(
    writer: &mut PacketWriter,
    registry_id: i32,
) -> Result<(), qexed_packet::error::PacketError> {
    0_u8.serialize(writer)?;
    VarInt(registry_id).serialize(writer)?;
    Ok(())
}

/// 静态光照：天光全 26 层满级（15），方块光全空。
///
/// 满级天光让夜晚不黑、无实体误刷；真实光照由 `qexed_light` 路线接管。
pub fn static_light() -> Light {
    let full = Bitset(vec![(1_u64 << LIGHT_SECTION_COUNT) - 1]);
    Light {
        sky_light_mask: full.clone(),
        block_light_mask: Bitset::default(),
        empty_sky_light_mask: Bitset::default(),
        empty_block_light_mask: full,
        // 数组数必须与掩码置位数严格一致（客户端按掩码逐位消费数组，
        // 少一个就会 NoSuchElementException——v4 配方是 26 位掩码 + 26 个数组）
        sky_light_arrays: vec![bright_array(); LIGHT_SECTION_COUNT],
        block_light_arrays: Vec::new(),
    }
}

/// 全 15 级的光照数组（每 nibble 15 → 字节 0xFF）。
fn bright_array() -> LightArray {
    LightArray([0xFF; LIGHT_ARRAY_BYTES])
}

#[cfg(test)]
mod tests {
    use super::*;
    use qexed_packet::{Packet as _, PacketReader};

    fn round_trip(packet: &MapChunk) -> MapChunk {
        let mut buf = BytesMut::new();
        let mut writer = PacketWriter::new(&mut buf);
        packet.serialize(&mut writer).expect("serialize chunk");
        let mut frozen = buf.freeze();
        let mut reader = PacketReader::new(&mut frozen);
        let mut decoded = MapChunk::default();
        decoded.deserialize(&mut reader).expect("deserialize chunk");
        decoded
    }

    #[test]
    fn empty_section_bytes_size_matches_format() {
        // 每 section：2(i16) + 2(i16) + 1(u8 bits) + 1(VarInt air) + 1(u8 bits) + 1(VarInt plains)
        assert_eq!(empty_section_bytes().len(), SECTION_COUNT * 8);
    }

    #[test]
    fn empty_chunk_packet_round_trips() {
        let packet = empty_chunk_packet(3, -5);
        let decoded = round_trip(&packet);
        assert_eq!((decoded.chunk_x, decoded.chunk_z), (3, -5));
        assert_eq!(decoded.data.data.len(), SECTION_COUNT * 8);
        assert_eq!(decoded.data.block_entities.len(), 0);
        // heightmaps：3 项，键 1/4/5，各 37 long
        assert_eq!(decoded.data.heightmaps.len(), 3);
        assert_eq!(decoded.data.heightmaps[0].type_id, VarInt(1));
        assert_eq!(decoded.data.heightmaps[2].data.len(), 37);
        // 光照：天光 26 层掩码 + 26 个数组（含世界下/上边界层）
        assert_eq!(decoded.light.sky_light_arrays.len(), LIGHT_SECTION_COUNT);
        assert_eq!(decoded.light.block_light_arrays.len(), 0);
    }

    #[test]
    fn light_masks_cover_26_sections() {
        let light = static_light();
        assert_eq!(light.sky_light_mask.0, vec![(1_u64 << 26) - 1]);
        assert_eq!(light.empty_block_light_mask.0, vec![(1_u64 << 26) - 1]);
        assert!(light.block_light_mask.0.is_empty());
        assert!(light.empty_sky_light_mask.0.is_empty());
    }
}