//! 光照桥接：`qexed_light::LightPayload`（纯数据形态）→ `MapChunk::light`
//! （`qexed_protocol` 网络形态）。
//!
//! 两边字段一一对应：`LightPayload` 保证"掩码置位数 == 数组数"且 lit/empty
//! 掩码互补覆盖 26 位（bit0 = 世界下方虚拟层，bit1..=24 = 24 个方块
//! section，bit25 = 世界上方虚拟层），因此桥只做类型转换，不重排任何
//! 位序 / 数组序：
//! - `Vec<u64>` 掩码 → `qexed_packet::net_types::Bitset`，去尾部全零词后直移
//!   （引擎内部掩码恒 ≥1 词，协议/静态形态下全空掩码不带词，线上语义一致）；
//! - `SectionLightArray = [u8; 2048]` → 协议 `LightArray`，字节原样直移
//!   （序列化时由协议侧自动加 VarInt(2048) 长度前缀）。

use qexed_light::{LightPayload, SectionLightArray};
use qexed_packet::net_types::Bitset;
use qexed_protocol::to_client::play::map_chunk::{Light, LightArray};

/// 把引擎输出的光照负载转换为 MapChunk 的 `Light` 结构（天光与方块光
/// 同源时使用；分两次计算请用 [light_payloads_to_light]）。
pub fn payload_to_light(payload: &LightPayload) -> Light {
    light_payloads_to_light(payload, payload)
}

/// 天光 / 方块光分两次计算（各得一个 `LightPayload`）时合成 `Light`：
/// 掩码与数组各自取自对应负载，仍不重排任何位序 / 数组序。
pub fn light_payloads_to_light(sky: &LightPayload, block: &LightPayload) -> Light {
    Light {
        sky_light_mask: to_bitset(sky.sky_light_mask.clone()),
        block_light_mask: to_bitset(block.block_light_mask.clone()),
        empty_sky_light_mask: to_bitset(sky.empty_sky_light_mask.clone()),
        empty_block_light_mask: to_bitset(block.empty_block_light_mask.clone()),
        sky_light_arrays: to_protocol_arrays(&sky.sky_light_arrays),
        block_light_arrays: to_protocol_arrays(&block.block_light_arrays),
    }
}

/// 掩码规范化：去掉尾部全零词。引擎内部位图初始即含 1 个零词，
/// 全空掩码因此形如 `vec![0]`；协议侧（`chunk::static_light`、原版服务器）
/// 全空掩码不带任何词，两种形态线上解码出的位图相同，统一为后者。
fn to_bitset(mut words: Vec<u64>) -> Bitset {
    while words.last() == Some(&0) {
        words.pop();
    }
    Bitset(words)
}

/// 数组原样直移（`[u8; 2048]` 是 Copy，逐个取引用转协议包装）。
fn to_protocol_arrays(arrays: &[SectionLightArray]) -> Vec<LightArray> {
    arrays.iter().map(|array| LightArray(*array)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;
    use qexed_light::DimensionGeometry;
    use qexed_packet::{Packet as _, PacketReader, PacketWriter};
    use qexed_protocol::to_client::play::map_chunk::{Chunk, MapChunk};

    /// 与 chunk.rs tests 的 round_trip 同款：serialize → deserialize。
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

    /// 静态满级天光路径：`static_light`（手写掩码 + 26×0xFF 数组）与
    /// `LightPayload::filled(geometry, 15, 0)` 过桥后应完全一致。
    #[test]
    fn filled_payload_matches_static_light() {
        let geometry = &DimensionGeometry::overworld().geometry;
        let payload = LightPayload::filled(geometry, 15, 0);
        let bridged = payload_to_light(&payload);
        let expected = crate::chunk::static_light();

        assert_eq!(bridged.sky_light_mask, expected.sky_light_mask);
        assert_eq!(bridged.block_light_mask, expected.block_light_mask);
        assert_eq!(bridged.empty_sky_light_mask, expected.empty_sky_light_mask);
        assert_eq!(bridged.empty_block_light_mask, expected.empty_block_light_mask);
        assert_eq!(bridged.sky_light_arrays, expected.sky_light_arrays);
        assert_eq!(bridged.block_light_arrays, expected.block_light_arrays);
    }

    #[test]
    fn full_sky_payload_round_trips() {
        let geometry = &DimensionGeometry::overworld().geometry;
        let packet = MapChunk {
            chunk_x: 3,
            chunk_z: -4,
            data: Chunk {
                heightmaps: crate::chunk::empty_heightmaps(),
                data: crate::chunk::empty_section_bytes(),
                block_entities: Vec::new(),
            },
            light: payload_to_light(&LightPayload::filled(geometry, 15, 0)),
        };

        let decoded = round_trip(&packet);
        assert_eq!(decoded.chunk_x, 3);
        assert_eq!(decoded.chunk_z, -4);
        assert_eq!(decoded.light, packet.light);
    }

    /// 双负载合成路径：天光满级 / 方块光单列光柱，`light_payloads_to_light`
    /// 应与单负载桥各字段一致。
    #[test]
    fn split_payloads_bridge_matches_single() {
        let geometry = &DimensionGeometry::overworld().geometry;
        let sky = LightPayload::filled(geometry, 15, 0);
        let mut block_raw = vec![0_u8; geometry.chunk_light_len];
        for y in geometry.world_min_y..=geometry.world_max_y {
            block_raw[geometry.block_index(0, y, 0)] = 7;
        }
        let zeros = vec![0_u8; geometry.chunk_light_len];
        let block = LightPayload::from_raw(geometry, &zeros, &block_raw);

        let split = light_payloads_to_light(&sky, &block);
        let whole = Light {
            sky_light_mask: to_bitset(sky.sky_light_mask.clone()),
            block_light_mask: to_bitset(block.block_light_mask.clone()),
            empty_sky_light_mask: to_bitset(sky.empty_sky_light_mask.clone()),
            empty_block_light_mask: to_bitset(block.empty_block_light_mask.clone()),
            sky_light_arrays: split.sky_light_arrays.clone(),
            block_light_arrays: split.block_light_arrays.clone(),
        };
        assert_eq!(split, whole);
        // 光柱掩码进入 block 侧，天光掩码不受影响。
        assert_eq!(split.block_light_mask.0, vec![(1_u64 << 26) - 2]);
        assert_eq!(split.sky_light_mask.0, vec![(1_u64 << 26) - 1]);
    }

    #[test]
    fn single_column_block_light_round_trips() {
        // 中心 (8,8) 一根全高 14 级光柱：24 个实层 + 上方虚拟层复制 = 25 个
        // 非空数组；下方虚拟层恒空（bit0 落 empty 掩码）。
        let geometry = &DimensionGeometry::overworld().geometry;
        let sky = vec![0_u8; geometry.chunk_light_len];
        let mut block = vec![0_u8; geometry.chunk_light_len];
        for y in geometry.world_min_y..=geometry.world_max_y {
            block[geometry.block_index(8, y, 8)] = 14;
        }
        let payload = LightPayload::from_raw(geometry, &sky, &block);
        assert_eq!(payload.block_light_mask, vec![(1_u64 << 26) - 2]);
        assert_eq!(payload.empty_block_light_mask, vec![1_u64]);
        assert_eq!(payload.block_light_arrays.len(), 25);

        let packet = MapChunk {
            chunk_x: 0,
            chunk_z: 0,
            data: Chunk {
                heightmaps: crate::chunk::empty_heightmaps(),
                data: crate::chunk::empty_section_bytes(),
                block_entities: Vec::new(),
            },
            light: payload_to_light(&payload),
        };
        let decoded = round_trip(&packet);
        assert_eq!(decoded.light, packet.light);

        // 按掩码位序还原：arrays[23] = bit24（最高实层 y 304..=319），
        // arrays[24] = bit25（上方虚拟层 = 最高实层复制）。
        // index = 15*256 + 8*16 + 8 = 3976 → 字节 1988：低 nibble（index 3976）
        // = 14，高 nibble（index 3977 = 同行 x=9，无光）= 0。
        assert_eq!(decoded.light.block_light_arrays[23].0[1988], 0x0E);
        assert_eq!(decoded.light.block_light_arrays[24].0[1988], 0x0E);
        // 同行 x=0（index 3968 → 字节 1984）两 nibble 皆无光。
        assert_eq!(decoded.light.block_light_arrays[23].0[1984], 0x00);
        // 引擎视角逐格校验同一根光柱。世界内坐标取值正常；320 越出方块
        // 区间，API 恒 0（上方虚拟层复制语义由上面 arrays[24] 断言覆盖）。
        assert_eq!(payload.block_at(geometry, 8, 319, 8), 14);
        assert_eq!(payload.block_at(geometry, 8, 320, 8), 0);
        assert_eq!(payload.block_at(geometry, 8, -65, 8), 0);
        assert_eq!(payload.block_at(geometry, 0, 100, 0), 0);
    }
}
