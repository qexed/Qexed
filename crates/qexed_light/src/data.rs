//! 光照数据层：单个 chunk 的天光/方块光缓存、3x3 邻域、方块光学属性默认表。
//!
//! 26.1.2 的光照仍按 section（16`3）组织：每个光照数组 2048 字节半字节
//! 打包（低半字节 = 偶数下标方块）。本模块只存"整 chunk 的一维展开"
//! （长度 = chunk_light_len），section 切分在导出 [`LightPayload`] 时进行，
//! 这样传播计算可以整 chunk 连续进行，不用跨 section 边界特判。

use crate::error::LightError;
use crate::geometry::{DimensionGeometry, LightGeometry};

/// 一个 chunk 的光照数据（天光 + 方块光），长度均为 chunk_light_len。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkLightData {
    geometry: LightGeometry,
    sky_light: Vec<u8>,
    block_light: Vec<u8>,
}

impl ChunkLightData {
    /// 全 0 光照。
    pub fn empty(geometry: &LightGeometry) -> Self {
        let len = geometry.chunk_light_len;
        Self {
            geometry: *geometry,
            sky_light: vec![0; len],
            block_light: vec![0; len],
        }
    }

    pub fn geometry(&self) -> &LightGeometry {
        &self.geometry
    }

    pub fn sky_light(&self) -> &[u8] {
        &self.sky_light
    }

    pub fn block_light(&self) -> &[u8] {
        &self.block_light
    }

    pub fn sky_light_mut(&mut self) -> &mut [u8] {
        &mut self.sky_light
    }

    pub fn block_light_mut(&mut self) -> &mut [u8] {
        &mut self.block_light
    }

    /// 读取天光（越界坐标按 0 处理，供邻域查询兜底）。
    pub fn sky_at(&self, x: usize, y: i32, z: usize) -> u8 {
        if x >= 16
            || z >= 16
            || !(self.geometry.world_min_y..=self.geometry.world_max_y).contains(&y)
        {
            return 0;
        }
        self.sky_light[self.geometry.block_index(x, y, z)]
    }

    /// 读取方块光。
    pub fn block_at(&self, x: usize, y: i32, z: usize) -> u8 {
        if x >= 16
            || z >= 16
            || !(self.geometry.world_min_y..=self.geometry.world_max_y).contains(&y)
        {
            return 0;
        }
        self.block_light[self.geometry.block_index(x, y, z)]
    }

    /// 写天光（越界忽略）。
    pub fn set_sky(&mut self, x: usize, y: i32, z: usize, level: u8) {
        if !(self.geometry.world_min_y..=self.geometry.world_max_y).contains(&y) {
            return;
        }
        let index = self.geometry.block_index(x, y, z);
        self.sky_light[index] = level.min(15);
    }

    /// 写方块光（越界忽略）。
    pub fn set_block(&mut self, x: usize, y: i32, z: usize, level: u8) {
        if !(self.geometry.world_min_y..=self.geometry.world_max_y).contains(&y) {
            return;
        }
        let index = self.geometry.block_index(x, y, z);
        self.block_light[index] = level.min(15);
    }

    /// 按一维下标写天光（引擎内部用）。
    pub(crate) fn set_sky_by_index(&mut self, index: usize, level: u8) {
        self.sky_light[index] = level.min(15);
    }

    /// 按一维下标写方块光（引擎内部用）。
    pub(crate) fn set_block_by_index(&mut self, index: usize, level: u8) {
        self.block_light[index] = level.min(15);
    }
}

/// 3x3 chunk 邻域：中心 chunk 必须有，邻居可缺（缺的方向按"无数据"处理，
/// 天光从缺邻居方向进入按 15 计，方块光按 0 计——与 v4 语义一致）。
#[derive(Debug, Clone)]
pub struct LightNeighborhood {
    dimension: DimensionGeometry,
    /// chunks[(dz+1)*3 + (dx+1)]，None = 缺邻居。
    chunks: [Option<ChunkLightData>; 9],
}

impl LightNeighborhood {
    /// 只有中心 chunk 的邻域。
    pub fn single(center: ChunkLightData) -> Self {
        let dimension = DimensionGeometry::new_from_geometry(center.geometry());
        let mut chunks: [Option<ChunkLightData>; 9] = std::array::from_fn(|_| None);
        chunks[4] = Some(center);
        Self { dimension, chunks }
    }

    /// 放入邻居（offset -1..=1；越界忽略）。
    pub fn set(&mut self, offset_x: i32, offset_z: i32, data: ChunkLightData) {
        if let Some(index) = Self::chunk_index(offset_x, offset_z) {
            self.chunks[index] = Some(data);
        }
    }

    /// (dx, dz) → chunks 下标；超出 3x3 返回 None。
    pub fn chunk_index(offset_x: i32, offset_z: i32) -> Option<usize> {
        ((-1..=1).contains(&offset_x) && (-1..=1).contains(&offset_z))
            .then_some(((offset_z + 1) * 3 + (offset_x + 1)) as usize)
    }

    pub fn center(&self) -> Option<&ChunkLightData> {
        self.chunks[4].as_ref()
    }

    pub fn center_mut(&mut self) -> Option<&mut ChunkLightData> {
        self.chunks[4].as_mut()
    }

    pub fn chunk(&self, offset_x: i32, offset_z: i32) -> Option<&ChunkLightData> {
        self.chunks[Self::chunk_index(offset_x, offset_z)?].as_ref()
    }

    pub fn dimension(&self) -> &DimensionGeometry {
        &self.dimension
    }

    pub fn geometry(&self) -> &LightGeometry {
        &self.dimension.geometry
    }

    /// 邻域坐标（x/z 可为 -16..31）取天光；坐标缺数据时返回 None。
    pub fn sky_at(&self, x: i32, y: i32, z: i32) -> Option<u8> {
        let chunk = self.chunk(x.div_euclid(16), z.div_euclid(16))?;
        Some(chunk.sky_at(x.rem_euclid(16) as usize, y, z.rem_euclid(16) as usize))
    }

    /// 邻域坐标取方块光。
    pub fn block_at(&self, x: i32, y: i32, z: i32) -> Option<u8> {
        let chunk = self.chunk(x.div_euclid(16), z.div_euclid(16))?;
        Some(chunk.block_at(x.rem_euclid(16) as usize, y, z.rem_euclid(16) as usize))
    }
}

impl DimensionGeometry {
    /// 从纯几何反推维度描述（邻域构造用；name/ambient_light 用默认值）。
    pub(crate) fn new_from_geometry(geometry: &LightGeometry) -> Self {
        Self {
            geometry: *geometry,
            has_skylight: true,
            ambient_light: 0.0,
            name: "custom",
        }
    }
}

/// 方块光学属性（26.1.2 语义）：
/// - opacity（不透明度）：阻挡光的"厚度"。空气 0，普通实心方块 15，
///   水/树叶等按 vanilla light_block 属性取中间值。
/// - luminance（亮度）：光源等级（火把 14、萤石 15、岩浆 15 ……）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockLightProps {
    pub opacity: u8,
    pub luminance: u8,
}

impl Default for BlockLightProps {
    fn default() -> Self {
        Self {
            opacity: 0,
            luminance: 0,
        }
    }
}

impl BlockLightProps {
    pub const AIR: Self = Self {
        opacity: 0,
        luminance: 0,
    };
    pub const SOLID: Self = Self {
        opacity: 15,
        luminance: 0,
    };
}

/// 一 chunk 的方块光学属性缓存（长度 = chunk_light_len，下标同 block_index）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkOpacity {
    geometry: LightGeometry,
    opacity: Vec<u8>,
    luminance: Vec<u8>,
}

impl ChunkOpacity {
    pub fn empty(geometry: &LightGeometry) -> Self {
        let len = geometry.chunk_light_len;
        Self {
            geometry: *geometry,
            opacity: vec![0; len],
            luminance: vec![0; len],
        }
    }

    /// 用统一属性填充整个 chunk（如生成阶段的"全空气"基线）。
    pub fn filled(geometry: &LightGeometry, props: BlockLightProps) -> Self {
        let len = geometry.chunk_light_len;
        Self {
            geometry: *geometry,
            opacity: vec![props.opacity; len],
            luminance: vec![props.luminance; len],
        }
    }

    pub fn geometry(&self) -> &LightGeometry {
        &self.geometry
    }

    pub fn opacity(&self) -> &[u8] {
        &self.opacity
    }

    pub fn luminance(&self) -> &[u8] {
        &self.luminance
    }

    pub fn set(
        &mut self,
        x: usize,
        y: i32,
        z: usize,
        props: BlockLightProps,
    ) -> Result<(), LightError> {
        if x >= 16
            || z >= 16
            || !(self.geometry.world_min_y..=self.geometry.world_max_y).contains(&y)
        {
            return Err(LightError::BlockOutOfBounds { x, y, z });
        }
        let index = self.geometry.block_index(x, y, z);
        self.opacity[index] = props.opacity.min(15);
        self.luminance[index] = props.luminance.min(15);
        Ok(())
    }

    pub fn opacity_at(&self, x: usize, y: i32, z: usize) -> Option<u8> {
        if x >= 16
            || z >= 16
            || !(self.geometry.world_min_y..=self.geometry.world_max_y).contains(&y)
        {
            return None;
        }
        Some(self.opacity[self.geometry.block_index(x, y, z)])
    }

    pub fn luminance_at(&self, x: usize, y: i32, z: usize) -> Option<u8> {
        if x >= 16
            || z >= 16
            || !(self.geometry.world_min_y..=self.geometry.world_max_y).contains(&y)
        {
            return None;
        }
        Some(self.luminance[self.geometry.block_index(x, y, z)])
    }
}

/// 3x3 邻域的不透明度视图（与 [`LightNeighborhood`] 同布局）。
#[derive(Debug, Clone)]
pub struct OpacityNeighborhood {
    chunks: [Option<ChunkOpacity>; 9],
}

impl OpacityNeighborhood {
    pub fn single(center: ChunkOpacity) -> Self {
        let mut chunks: [Option<ChunkOpacity>; 9] = std::array::from_fn(|_| None);
        chunks[4] = Some(center);
        Self { chunks }
    }

    pub fn set(&mut self, offset_x: i32, offset_z: i32, data: ChunkOpacity) {
        if let Some(index) = LightNeighborhood::chunk_index(offset_x, offset_z) {
            self.chunks[index] = Some(data);
        }
    }

    pub fn center(&self) -> Option<&ChunkOpacity> {
        self.chunks[4].as_ref()
    }

    pub fn chunk(&self, offset_x: i32, offset_z: i32) -> Option<&ChunkOpacity> {
        self.chunks[LightNeighborhood::chunk_index(offset_x, offset_z)?].as_ref()
    }

    /// 邻域坐标取不透明度；缺 chunk / 越界 Y 返回 None（调用方按开空间处理）。
    pub fn opacity_at(&self, x: i32, y: i32, z: i32) -> Option<u8> {
        let chunk = self.chunk(x.div_euclid(16), z.div_euclid(16))?;
        chunk.opacity_at(x.rem_euclid(16) as usize, y, z.rem_euclid(16) as usize)
    }

    /// 邻域坐标取亮度源；缺 chunk / 越界 Y 返回 None。
    pub fn luminance_at(&self, x: i32, y: i32, z: i32) -> Option<u8> {
        let chunk = self.chunk(x.div_euclid(16), z.div_euclid(16))?;
        chunk.luminance_at(x.rem_euclid(16) as usize, y, z.rem_euclid(16) as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_light_data_round_trip() {
        let dim = DimensionGeometry::overworld();
        let mut data = ChunkLightData::empty(&dim.geometry);
        assert_eq!(data.sky_light().len(), dim.geometry.chunk_light_len);
        data.set_sky(3, 0, 4, 9);
        data.set_block(3, 0, 4, 14);
        assert_eq!(data.sky_at(3, 0, 4), 9);
        assert_eq!(data.block_at(3, 0, 4), 14);
        // 越界写入被忽略
        data.set_sky(0, 999, 0, 15);
        assert_eq!(data.sky_at(0, 999, 0), 0);
        // 等级钳制
        data.set_sky(1, 0, 1, 20);
        assert_eq!(data.sky_at(1, 0, 1), 15);
    }

    #[test]
    fn neighborhood_layout() {
        let dim = DimensionGeometry::overworld();
        let mut nb = LightNeighborhood::single(ChunkLightData::empty(&dim.geometry));
        // 中心缺 → None；邻居坐标查询跨 chunk
        assert!(nb.center().is_some());
        let mut east = ChunkLightData::empty(&dim.geometry);
        east.set_sky(0, 0, 0, 7);
        nb.set(1, 0, east);
        // 中心 chunk x=15 的东侧邻居是 east chunk x=0（天光 7）
        assert_eq!(nb.sky_at(16, 0, 0), Some(7));
        assert_eq!(nb.sky_at(17, 0, 0), Some(0)); // 仍在东邻 chunk 内（未点亮处）
        assert_eq!(nb.sky_at(32, 0, 0), None); // 超出 3x3
        assert_eq!(LightNeighborhood::chunk_index(0, 0), Some(4));
        assert_eq!(LightNeighborhood::chunk_index(1, 1), Some(8));
        assert_eq!(LightNeighborhood::chunk_index(-1, -1), Some(0));
        assert_eq!(LightNeighborhood::chunk_index(2, 0), None);
    }

    #[test]
    fn opacity_neighborhood_cross_chunk() {
        let g = &DimensionGeometry::overworld().geometry;
        let mut center = ChunkOpacity::filled(g, BlockLightProps::AIR);
        center.set(15, 0, 0, BlockLightProps::SOLID).unwrap();
        let mut nb = OpacityNeighborhood::single(center);
        let mut west = ChunkOpacity::filled(g, BlockLightProps::AIR);
        west.set(
            0,
            0,
            0,
            BlockLightProps {
                opacity: 3,
                luminance: 0,
            },
        )
        .unwrap();
        nb.set(-1, 0, west);
        // 中心 x=0 的西侧是 west chunk x=15（空气）
        assert_eq!(nb.opacity_at(-1, 0, 0), Some(0));
        // west chunk 自己的 x=0 距中心两 chunk，不可见
        assert_eq!(nb.opacity_at(-16, 0, 0), Some(3));
        assert_eq!(nb.opacity_at(15, 0, 0), Some(15));
    }
}
