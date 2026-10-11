//! 光照几何常量（对齐 MC 26.1.2 的 dimension_type 数据）。
//!
//! 26.1.2 vanilla 维度几何（cache/mojang/26.1.2/data/minecraft/dimension_type）：
//! - overworld      min_y=-64  height=384  has_skylight=true
//! - overworld_caves min_y=-64 height=384  has_skylight=true
//! - the_nether     min_y=0    height=256  has_skylight=false（恒暗维度）
//! - the_end        min_y=0    height=256  has_skylight=true（天光按高度直接给满）
//!
//! 光照数据的节点（section）网格在方块区间之外还包上下各一层：
//! light section 数量 = height/16 + 2，索引 0 是 minY 以下、最后一个在 maxY 之上。
//! 协议侧（qexed_protocol 的 LightUpdate/MapChunk.Light）与 Anvil 存档侧
//! （SkyLight/BlockLight NBT 列表）都遵循同一布局。

/// 一个光照节点（section）的边长：16 方块。
pub const SECTION_HEIGHT: i32 = 16;
/// 一个节点内的方块数：16`3 = 4096。
pub const SECTION_BLOCKS: usize = 16 * 16 * 16;

/// 一维光照数组（半字节打包）的字节数：4096/2。
pub const LIGHT_ARRAY_BYTES: usize = SECTION_BLOCKS / 2;

/// 光照等级上限（半字节）。
pub const MAX_LIGHT_LEVEL: u8 = 15;

/// 维度无关的几何派生值（由 [`DimensionGeometry`] 计算，见下）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LightGeometry {
    /// 方块区间的最低 Y（含）。
    pub world_min_y: i32,
    /// 方块区间的最高 Y（含）。
    pub world_max_y: i32,
    /// 方块 section 数 = height / 16。
    pub world_section_count: usize,
    /// 光照 section 数 = 方块 section 数 + 2（上下各一层虚拟层）。
    pub light_section_count: usize,
    /// 最下方光照 section 对应的 section Y（= world_min_y/16 - 1）。
    pub min_light_section_y: i32,
    /// 最上方光照 section 对应的 section Y（= world_max_y/16 + 1）。
    pub max_light_section_y: i32,
    /// 整个 chunk 光照立方体的方块数（16 x height x 16），天光/方块光缓存的长度。
    pub chunk_light_len: usize,
}

impl LightGeometry {
    /// section Y 是否落在光照网格（含上下虚拟层）内。
    pub fn contains_light_section(&self, section_y: i32) -> bool {
        (self.min_light_section_y..=self.max_light_section_y).contains(&section_y)
    }

    /// section Y → 光照数组下标；越界返回 None。
    pub fn light_section_index(&self, section_y: i32) -> Option<usize> {
        if !self.contains_light_section(section_y) {
            return None;
        }
        Some((section_y - self.min_light_section_y) as usize)
    }

    /// 光照数组下标 → section Y（供序列化/调试反查）。
    pub fn light_section_y(&self, index: usize) -> Option<i32> {
        if index >= self.light_section_count {
            return None;
        }
        Some(self.min_light_section_y + index as i32)
    }

    /// 方块世界 Y → 所在光照 section 的下标；方块区间外返回 None。
    pub fn light_section_index_for_block_y(&self, y: i32) -> Option<usize> {
        if !(self.world_min_y..=self.world_max_y).contains(&y) {
            return None;
        }
        self.light_section_index(y.div_euclid(SECTION_HEIGHT))
    }

    /// 世界坐标 → chunk 内天光/方块光缓存的一维下标。
    ///
    /// 布局与 v4 一致：((y - world_min_y) * 16 + z) * 16 + x（行主序，x 最内）。
    pub fn block_index(&self, x: usize, y: i32, z: usize) -> usize {
        debug_assert!(x < 16);
        debug_assert!(z < 16);
        debug_assert!((self.world_min_y..=self.world_max_y).contains(&y));
        ((y - self.world_min_y) as usize * 16 + z) * 16 + x
    }

}

/// 维度的光照几何与天光语义，来自 26.1.2 的 dimension_type。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DimensionGeometry {
    pub geometry: LightGeometry,
    /// dimension_type 的 has_skylight：false 时整列天光恒 0（下界）。
    pub has_skylight: bool,
    /// dimension_type 的 ambient_light：下界 0.1 / 末地 0.25。
    /// 仅影响渲染端环境光，光照网络仍按 0 计（vanilla 行为）。
    pub ambient_light: f32,
    /// 维度注册名（minecraft:overworld 等，日志/调试用）。
    pub name: &'static str,
}

impl DimensionGeometry {
    /// 由 min_y / height / has_skylight 构造（min_y 必须是 16 的倍数）。
    pub fn new(name: &'static str, min_y: i32, height: i32, has_skylight: bool, ambient_light: f32) -> Self {
        assert!(height > 0 && height % SECTION_HEIGHT == 0, "height must be a positive multiple of 16");
        assert!(min_y % SECTION_HEIGHT == 0, "min_y must be a multiple of 16");
        let world_section_count = (height / SECTION_HEIGHT) as usize;
        let min_light_section_y = min_y / SECTION_HEIGHT - 1;
        let geometry = LightGeometry {
            world_min_y: min_y,
            world_max_y: min_y + height - 1,
            world_section_count,
            light_section_count: world_section_count + 2,
            min_light_section_y,
            max_light_section_y: (min_y + height) / SECTION_HEIGHT,
            chunk_light_len: 16 * world_section_count * 16 * 16,
        };
        Self { geometry, has_skylight, ambient_light, name }
    }

    /// 26.1.2 主世界：min_y=-64 height=384 has_skylight=true。
    pub fn overworld() -> Self {
        Self::new("minecraft:overworld", -64, 384, true, 0.0)
    }

    /// 26.1.2 下界：min_y=0 height=256 has_skylight=false。
    pub fn the_nether() -> Self {
        Self::new("minecraft:the_nether", 0, 256, false, 0.1)
    }

    /// 26.1.2 末地：min_y=0 height=256 has_skylight=true。
    pub fn the_end() -> Self {
        Self::new("minecraft:the_end", 0, 256, true, 0.25)
    }
}

impl Default for DimensionGeometry {
    fn default() -> Self {
        Self::overworld()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overworld_geometry_matches_26_1_2() {
        let dim = DimensionGeometry::overworld();
        let g = &dim.geometry;
        assert_eq!(g.world_min_y, -64);
        assert_eq!(g.world_max_y, 319);
        assert_eq!(g.world_section_count, 24);
        assert_eq!(g.light_section_count, 26);
        assert_eq!(g.min_light_section_y, -5);
        assert_eq!(g.max_light_section_y, 20);
        assert_eq!(g.chunk_light_len, 98304);
        assert!(dim.has_skylight);
    }

    #[test]
    fn nether_geometry_matches_26_1_2() {
        let dim = DimensionGeometry::the_nether();
        let g = &dim.geometry;
        assert_eq!(g.world_min_y, 0);
        assert_eq!(g.world_max_y, 255);
        assert_eq!(g.world_section_count, 16);
        assert_eq!(g.light_section_count, 18);
        assert_eq!(g.min_light_section_y, -1);
        assert!(!dim.has_skylight);
    }

    #[test]
    fn section_index_round_trip() {
        let dim = DimensionGeometry::overworld();
        let g = &dim.geometry;
        assert_eq!(g.light_section_index(-5), Some(0));
        assert_eq!(g.light_section_index(-4), Some(1));
        assert_eq!(g.light_section_index(20), Some(25));
        assert_eq!(g.light_section_index(-6), None);
        assert_eq!(g.light_section_index(21), None);
        assert_eq!(g.light_section_y(0), Some(-5));
        assert_eq!(g.light_section_y(25), Some(20));
        // 方块 Y → section
        assert_eq!(g.light_section_index_for_block_y(-64), Some(1));
        assert_eq!(g.light_section_index_for_block_y(319), Some(24));
        assert_eq!(g.light_section_index_for_block_y(-65), None);
        assert_eq!(g.light_section_index_for_block_y(320), None);
    }

    #[test]
    fn block_index_layout_matches_v4() {
        let g = &DimensionGeometry::overworld().geometry;
        assert_eq!(g.block_index(0, -64, 0), 0);
        assert_eq!(g.block_index(15, -64, 0), 15);
        assert_eq!(g.block_index(0, -63, 0), 256);
        assert_eq!(g.block_index(0, 319, 0), 383 * 256);
        assert_eq!(g.block_index(15, 319, 15), 98303);
    }
}
