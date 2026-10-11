//! 光照输出：把整 chunk 的一维天光/方块光切成协议/存档共用的
//! section 数组 + 掩码形式（MC 26.1.2 的 LightUpdate / MapChunk.Light 布局，
//! 也与 Anvil SkyLight/BlockLight NBT 一致），供 qexed_world 打包。
//!
//! 网络形态：
//! - sky_light_mask / block_light_mask：非全 0 section 的位图
//! - empty_sky_light_mask / empty_block_light_mask：全 0 section 的位图
//! - sky_light_arrays / block_light_arrays：按 mask 位序排列的 2048 字节数组
//!
//! 本模块只产出纯数据（掩码 u64 + 数组 Vec<[u8; 2048]>），不依赖协议 crate，
//! qexed_world 负责包一层转成 Bitset / LightArray。

use crate::data::ChunkLightData;
use crate::geometry::LightGeometry;

/// 单个光照 section 的半字节打包数组（2048 字节；低半字节 = 偶数下标）。
pub type SectionLightArray = [u8; 2048];

/// 一个 chunk 的完整光照输出（网络/存档通用形态）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LightPayload {
    /// 非空天光 section 位图（bit i = 光照 section i，i 从 min_light_section_y 起）。
    pub sky_light_mask: Vec<u64>,
    /// 非空方块光 section 位图。
    pub block_light_mask: Vec<u64>,
    /// 全 0 天光 section 位图。
    pub empty_sky_light_mask: Vec<u64>,
    /// 全 0 方块光 section 位图。
    pub empty_block_light_mask: Vec<u64>,
    /// 天光数组（按 sky_light_mask 位序）。
    pub sky_light_arrays: Vec<SectionLightArray>,
    /// 方块光数组（按 block_light_mask 位序）。
    pub block_light_arrays: Vec<SectionLightArray>,
}

impl LightPayload {
    /// 从整 chunk 天光/方块光切 section + 生成掩码。
    pub fn from_chunk_light(light: &ChunkLightData) -> Self {
        let geometry = light.geometry();
        Self::from_raw(geometry, light.sky_light(), light.block_light())
    }

    /// 从原始一维数组构造（长度必须等于 chunk_light_len）。
    pub fn from_raw(geometry: &LightGeometry, sky: &[u8], block: &[u8]) -> Self {
        assert_eq!(sky.len(), geometry.chunk_light_len, "sky light length mismatch");
        assert_eq!(block.len(), geometry.chunk_light_len, "block light length mismatch");

        let (sky_mask, sky_empty, sky_arrays) = split_sections(geometry, sky);
        let (block_mask, block_empty, block_arrays) = split_sections(geometry, block);
        Self {
            sky_light_mask: mask_words(sky_mask),
            block_light_mask: mask_words(block_mask),
            empty_sky_light_mask: mask_words(sky_empty),
            empty_block_light_mask: mask_words(block_empty),
            sky_light_arrays: sky_arrays,
            block_light_arrays: block_arrays,
        }
    }

    /// 全满（每 section 都是 level）的输出（Static/Fixed 模式用）。
    pub fn filled(geometry: &LightGeometry, sky_level: u8, block_level: u8) -> Self {
        let (sky_mask, sky_empty, sky_arrays) = filled_sections(geometry, sky_level);
        let (block_mask, block_empty, block_arrays) = filled_sections(geometry, block_level);
        Self {
            sky_light_mask: mask_words(sky_mask),
            block_light_mask: mask_words(block_mask),
            empty_sky_light_mask: mask_words(sky_empty),
            empty_block_light_mask: mask_words(block_empty),
            sky_light_arrays: sky_arrays,
            block_light_arrays: block_arrays,
        }
    }

    /// 天光 section 数组按 section 下标的视图（空返回 None；调试/测试用）。
    pub fn sky_section(&self, section_index: usize) -> Option<&SectionLightArray> {
        section_by_mask(&self.sky_light_mask, &self.sky_light_arrays, section_index)
    }

    /// 方块光 section 数组视图。
    pub fn block_section(&self, section_index: usize) -> Option<&SectionLightArray> {
        section_by_mask(&self.block_light_mask, &self.block_light_arrays, section_index)
    }

    /// 某坐标的天光等级（section 缺失 = 0；调试/断言用）。
    pub fn sky_at(&self, geometry: &LightGeometry, x: usize, y: i32, z: usize) -> u8 {
        let Some(section_index) = geometry.light_section_index_for_block_y(y) else {
            return 0;
        };
        let Some(layer) = self.sky_section(section_index) else {
            return 0;
        };
        nibble_at(layer, section_block_index(x, y, z))
    }

    /// 某坐标的方块光等级。
    pub fn block_at(&self, geometry: &LightGeometry, x: usize, y: i32, z: usize) -> u8 {
        let Some(section_index) = geometry.light_section_index_for_block_y(y) else {
            return 0;
        };
        let Some(layer) = self.block_section(section_index) else {
            return 0;
        };
        nibble_at(layer, section_block_index(x, y, z))
    }
}

/// 掩码 u64 列表（bit set 长度按协议 = ceil(light_section_count/64)，不足补 0 词）。
fn mask_words(mask: u128_style::Mask) -> Vec<u64> {
    mask.0.to_vec()
}

/// 内部位图容器（light_section_count 最大 26，单 u128 够用；导出时转 u64 词）。
mod u128_style {
    pub struct Mask(pub Vec<u64>);

    impl Mask {
        pub fn new() -> Self {
            Self(vec![0])
        }

        pub fn set(&mut self, bit: usize) {
            let word = bit / 64;
            while self.0.len() <= word {
                self.0.push(0);
            }
            self.0[word] |= 1_u64 << (bit % 64);
        }
    }
}

fn split_sections(
    geometry: &LightGeometry,
    light: &[u8],
) -> (u128_style::Mask, u128_style::Mask, Vec<SectionLightArray>) {
    let mut non_empty = u128_style::Mask::new();
    let mut empty = u128_style::Mask::new();
    let mut arrays = Vec::new();

    // 只遍历方块 section（虚拟上下层单独处理）
    for section_y in geometry.min_light_section_y..=geometry.max_light_section_y {
        let Some(section_index) = geometry.light_section_index(section_y) else {
            continue;
        };
        // 虚拟层（min-1 / max+1）：天光维度上层恒满 15、下层恒 0；这里按
        // "与相邻实层一致"处理——上层取最高实层的值，下层取 0（空）。
        let section = if section_y < geometry.world_min_y / 16 {
            // 最下方虚拟层：恒空
            None
        } else if section_y > geometry.world_max_y / 16 {
            // 最上方虚拟层：取最高实层天光（末地/主世界上方开天空）
            let top_y = geometry.world_max_y;
            let source_range = (top_y - 15)..=top_y;
            Some(pack_section_rows(geometry, light, source_range))
        } else {
            let base_y = section_y * 16;
            let range = base_y..=base_y + 15;
            Some(pack_section_rows(geometry, light, range))
        };

        match section {
            Some(array) => {
                let is_zero = array.iter().all(|&b| b == 0);
                if is_zero {
                    empty.set(section_index);
                } else {
                    non_empty.set(section_index);
                    arrays.push(array);
                }
            }
            None => {
                empty.set(section_index);
            }
        }
    }
    (non_empty, empty, arrays)
}

/// 把 light 中 [y_range] 的 16 行打包成一个 section 数组。
fn pack_section_rows(
    geometry: &LightGeometry,
    light: &[u8],
    y_range: std::ops::RangeInclusive<i32>,
) -> SectionLightArray {
    let mut array = [0_u8; 2048];
    for (row, y) in y_range.enumerate() {
        for z in 0..16 {
            for x in 0..16 {
                let level = light[geometry.block_index(x, y, z)].min(15);
                set_nibble(&mut array, row * 256 + z * 16 + x, level);
            }
        }
    }
    array
}

fn filled_sections(
    geometry: &LightGeometry,
    level: u8,
) -> (u128_style::Mask, u128_style::Mask, Vec<SectionLightArray>) {
    let mut non_empty = u128_style::Mask::new();
    let mut empty = u128_style::Mask::new();
    let mut arrays = Vec::new();
    let packed = pack_level(level);

    for section_index in 0..geometry.light_section_count {
        if level == 0 {
            empty.set(section_index);
        } else {
            non_empty.set(section_index);
            arrays.push(packed);
        }
    }
    (non_empty, empty, arrays)
}

fn pack_level(level: u8) -> SectionLightArray {
    let level = level.min(15);
    let packed = (level & 0x0f) | ((level & 0x0f) << 4);
    [packed; 2048]
}

fn section_by_mask<'a>(
    mask: &'a [u64],
    arrays: &'a [SectionLightArray],
    section_index: usize,
) -> Option<&'a SectionLightArray> {
    let word = section_index / 64;
    let bit = 1_u64 << (section_index % 64);
    if mask.get(word).copied().unwrap_or(0) & bit == 0 {
        return None;
    }
    // arrays 按 mask 位序：数前面的置位数
    let mut array_index = 0;
    for w in 0..word {
        array_index += mask.get(w).copied().unwrap_or(0).count_ones() as usize;
    }
    array_index += (mask.get(word).copied().unwrap_or(0) & (bit - 1)).count_ones() as usize;
    arrays.get(array_index)
}

fn section_block_index(x: usize, y: i32, z: usize) -> usize {
    let local_y = y.rem_euclid(16) as usize;
    (local_y * 16 + z) * 16 + x
}

fn set_nibble(array: &mut SectionLightArray, index: usize, level: u8) {
    let byte = index / 2;
    if index % 2 == 0 {
        array[byte] = (array[byte] & 0xf0) | level;
    } else {
        array[byte] = (array[byte] & 0x0f) | (level << 4);
    }
}

fn nibble_at(array: &SectionLightArray, index: usize) -> u8 {
    let byte = array[index / 2];
    if index % 2 == 0 {
        byte & 0x0f
    } else {
        byte >> 4
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::DimensionGeometry;

    #[test]
    fn filled_payload_masks_and_arrays() {
        let geometry = &DimensionGeometry::overworld().geometry;
        let payload = LightPayload::filled(geometry, 15, 0);
        // 天光：26 个 section 全非空
        assert_eq!(payload.sky_light_arrays.len(), geometry.light_section_count);
        assert_eq!(payload.sky_light_mask[0].count_ones() as usize, geometry.light_section_count);
        assert!(payload.empty_sky_light_mask.iter().all(|&w| w == 0));
        // 方块光：全 0 → 全空掩码
        assert!(payload.block_light_arrays.is_empty());
        assert_eq!(
            payload.empty_block_light_mask[0].count_ones() as usize,
            geometry.light_section_count
        );
        // 数组内容检查
        assert!(payload.sky_light_arrays.iter().all(|a| a.iter().all(|&b| b == 0xff)));
    }

    #[test]
    fn raw_payload_section_lookup() {
        let geometry = &DimensionGeometry::overworld().geometry;
        let mut sky = vec![0_u8; geometry.chunk_light_len];
        // y=0 平面天光 7
        for z in 0..16 {
            for x in 0..16 {
                sky[geometry.block_index(x, 0, z)] = 7;
            }
        }
        let payload = LightPayload::from_raw(geometry, &sky, &vec![0; geometry.chunk_light_len]);
        // y=0..15 在 section 0 → light index = 0-(-5) = 5
        assert_eq!(payload.sky_at(geometry, 5, 0, 5), 7);
        assert_eq!(payload.sky_at(geometry, 5, 1, 5), 0, "同 section 内 y=1 未点亮");
        assert_eq!(payload.sky_at(geometry, 5, 16, 5), 0, "上一 section 未点亮");
        // 该 section 掩码置位
        assert!(payload.sky_light_mask[0] & (1 << 5) != 0);
        // 其余 section 全空
        assert_eq!(payload.sky_light_mask[0].count_ones(), 1);
    }
}
