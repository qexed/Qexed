//! 光照引擎：天光 + 方块光传播（对齐 MC 26.1.2 语义）。
//!
//! 架构（继承 v4 light.rs，参数化到 26.1.2 维度几何）：
//! 1. 列扫描：自顶向下，opacity=0 保持、否则 -max(1, opacity)；
//! 2. 邻居独立预计算：每个有数据的邻居 chunk 各自跑"列扫描 + 内部扩散"，
//!    得到与中心 chunk 边界相邻列的天光值（v4 的 fast_neighbourhood_sky_values）；
//! 3. 边界播种：以邻居预计算值为源向中心 chunk 引入（水平进入 -max(1,opacity)）；
//!    缺数据的邻居方向不播种（不臆造开天空）；
//! 4. BFS 扩散：向下无衰减（天空直射规则），水平/向上 -max(1, opacity)；
//! 5. 可选射线追踪（RayTrace）：从暗点向 9 个斜上方向追踪开天空。
//!
//! 方块光：亮度源自播种（光源格 = luminance）+ 邻居边界播种 + BFS
//! （所有方向 -max(1, opacity)）。
//!
//! 下界（has_skylight=false）：天光恒 0，不跑天光通道。

use std::collections::VecDeque;

use crate::data::{ChunkLightData, ChunkOpacity, LightNeighborhood, OpacityNeighborhood};
use crate::geometry::{DimensionGeometry, LightGeometry};

/// 天光计算策略（对接 qexed_world 的生成/热更新两种场景）。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SkyLightAlgorithm {
    /// 列扫描 + 邻域播种 + BFS（v4 Fast）。
    #[default]
    Fast,
    /// Fast 基础上追加斜向射线追踪（v4 RayTrace）。
    RayTrace,
}

/// 天光 + 方块光统一入口。
#[derive(Debug)]
pub struct LightEngine {
    dimension: DimensionGeometry,
    algorithm: SkyLightAlgorithm,
}

impl LightEngine {
    pub fn new(dimension: DimensionGeometry, algorithm: SkyLightAlgorithm) -> Self {
        Self { dimension, algorithm }
    }

    pub fn dimension(&self) -> &DimensionGeometry {
        &self.dimension
    }

    pub fn algorithm(&self) -> SkyLightAlgorithm {
        self.algorithm
    }

    pub fn geometry(&self) -> &LightGeometry {
        &self.dimension.geometry
    }

    /// 只算中心 chunk 的天光。
    /// 邻居方向没有数据时不播种（不臆造开天空）；中心 chunk 自身顶上
    /// 视为开天空（列扫描起点 15）。
    pub fn compute_sky_light(&self, opacity: &OpacityNeighborhood) -> ChunkLightData {
        let geometry = self.geometry();
        let mut out = ChunkLightData::empty(geometry);
        if !self.dimension.has_skylight {
            return out;
        }
        let Some(center) = opacity.center() else {
            return out;
        };

        // 1. 中心列扫描
        let mut queue = VecDeque::new();
        scan_sky_columns(center, out.sky_light_mut(), &mut queue, geometry);
        // 2. 邻居独立预计算 + 边界播种（缺邻居不播）
        let neighbour_sky = self.neighbourhood_sky_values(opacity);
        seed_sky_from_neighbours(opacity, &neighbour_sky, &mut out, &mut queue);
        // 3. BFS
        spread_sky(center, out.sky_light_mut(), queue, geometry);
        // 4. RayTrace 增强
        if self.algorithm == SkyLightAlgorithm::RayTrace {
            ray_trace_sky(opacity, &neighbour_sky, &mut out, geometry);
        }
        out
    }

    /// 带既有天光邻域的完整计算（qexed_world 已缓存光照时优先用邻居光照值）。
    pub fn compute_sky_light_with_neighbors(
        &self,
        opacity: &OpacityNeighborhood,
        light: &LightNeighborhood,
    ) -> ChunkLightData {
        let geometry = self.geometry();
        let mut out = ChunkLightData::empty(geometry);
        if !self.dimension.has_skylight {
            return out;
        }
        let Some(center_opacity) = opacity.center() else {
            return out;
        };

        // 1. 中心列扫描
        let mut queue = VecDeque::new();
        scan_sky_columns(center_opacity, out.sky_light_mut(), &mut queue, geometry);
        // 2. 邻居播种：有光照值用光照值，没有的 chunk 方向退回独立预计算值
        let neighbour_sky = self.neighbourhood_sky_values(opacity);
        seed_sky_from_light(opacity, light, &neighbour_sky, &mut out, &mut queue);
        // 3. BFS
        spread_sky(center_opacity, out.sky_light_mut(), queue, geometry);
        // 4. RayTrace 增强
        if self.algorithm == SkyLightAlgorithm::RayTrace {
            ray_trace_sky(opacity, &neighbour_sky, &mut out, geometry);
        }
        out
    }

    /// 方块光（亮度源 + 邻域播种 + BFS）。任何维度都计算。
    pub fn compute_block_light(
        &self,
        opacity: &OpacityNeighborhood,
        light: Option<&LightNeighborhood>,
    ) -> ChunkLightData {
        let geometry = self.geometry();
        let mut out = ChunkLightData::empty(geometry);
        let Some(center) = opacity.center() else {
            return out;
        };

        let mut queue = VecDeque::new();
        // 1. 亮度源自播种
        for index in 0..geometry.chunk_light_len {
            let l = center.luminance()[index];
            if l > 0 {
                out.set_block_by_index(index, l.min(15));
                queue.push_back(index);
            }
        }
        // 2. 邻居边界播种（有光照邻居才播）
        if let Some(light) = light {
            seed_block_from_light(opacity, light, &mut out, &mut queue);
        }
        // 3. BFS
        spread_block(center, out.block_light_mut(), queue, geometry);
        out
    }

    /// 9 个邻居 chunk 各自独立算天光（列扫描 + 单 chunk 内 BFS）。
    /// 返回 [Option<Vec<u8>>; 9]，与 OpacityNeighborhood 同布局；
    /// None = 该方向缺数据。
    fn neighbourhood_sky_values(&self, opacity: &OpacityNeighborhood) -> NeighbourSkyValues {
        let geometry = self.geometry();
        let mut out: NeighbourSkyValues = std::array::from_fn(|_| None);
        for dz in -1..=1 {
            for dx in -1..=1 {
                let index = LightNeighborhood::chunk_index(dx, dz).expect("in-range offset");
                let Some(chunk) = opacity.chunk(dx, dz) else {
                    continue;
                };
                let mut values = vec![0_u8; geometry.chunk_light_len];
                let mut queue = VecDeque::new();
                scan_sky_columns(chunk, &mut values, &mut queue, geometry);
                spread_sky(chunk, &mut values, queue, geometry);
                out[index] = Some(values);
            }
        }
        out
    }
}

/// 邻居独立天光值（chunks 布局同 3x3 邻域）。
type NeighbourSkyValues = [Option<Vec<u8>>; 9];

// ─────────────────────────────────────────────
// 天光：列扫描
// ─────────────────────────────────────────────

fn scan_sky_columns(
    chunk: &ChunkOpacity,
    sky: &mut [u8],
    queue: &mut VecDeque<usize>,
    geometry: &LightGeometry,
) {
    for x in 0..16 {
        for z in 0..16 {
            let mut light: u8 = 15;
            for y in (geometry.world_min_y..=geometry.world_max_y).rev() {
                let index = geometry.block_index(x, y, z);
                let opacity = chunk.opacity()[index].min(15);
                if opacity > 0 {
                    light = light.saturating_sub(opacity.max(1));
                }
                if light > sky[index] {
                    sky[index] = light;
                    queue.push_back(index);
                }
                if light == 0 {
                    break;
                }
            }
        }
    }
}

// ─────────────────────────────────────────────
// 天光：邻域播种
// ─────────────────────────────────────────────

/// 无光照邻居：用邻居独立预计算值作源；缺邻居方向不播种。
fn seed_sky_from_neighbours(
    opacity: &OpacityNeighborhood,
    neighbour_sky: &NeighbourSkyValues,
    out: &mut ChunkLightData,
    queue: &mut VecDeque<usize>,
) {
    let geometry = out.geometry();
    for y in geometry.world_min_y..=geometry.world_max_y {
        for z in 0..16 {
            seed_sky_cell(opacity, neighbour_sky, out, queue, 0, y, z, -1, y, z as i32);
            seed_sky_cell(opacity, neighbour_sky, out, queue, 15, y, z, 16, y, z as i32);
        }
        for x in 0..16 {
            seed_sky_cell(opacity, neighbour_sky, out, queue, x, y, 0, x as i32, y, -1);
            seed_sky_cell(opacity, neighbour_sky, out, queue, x, y, 15, x as i32, y, 16);
        }
    }
}

fn seed_sky_cell(
    opacity: &OpacityNeighborhood,
    neighbour_sky: &NeighbourSkyValues,
    out: &mut ChunkLightData,
    queue: &mut VecDeque<usize>,
    local_x: usize,
    y: i32,
    local_z: usize,
    src_x: i32,
    src_y: i32,
    src_z: i32,
) {
    let source = sky_value_at(neighbour_sky, src_x, src_y, src_z, out.geometry());
    if source <= 1 {
        return;
    }
    let index = out.geometry().block_index(local_x, y, local_z);
    let cost = opacity
        .opacity_at(local_x as i32, y, local_z as i32)
        .unwrap_or(0)
        .min(15)
        .max(1);
    let candidate = source.saturating_sub(cost);
    if candidate > out.sky_light()[index] {
        out.set_sky_by_index(index, candidate);
        queue.push_back(index);
    }
}

/// 有光照邻居：优先光照值；缺光照值的 chunk 方向退回独立预计算值。
fn seed_sky_from_light(
    opacity: &OpacityNeighborhood,
    light: &LightNeighborhood,
    neighbour_sky: &NeighbourSkyValues,
    out: &mut ChunkLightData,
    queue: &mut VecDeque<usize>,
) {
    let geometry = out.geometry();
    for y in geometry.world_min_y..=geometry.world_max_y {
        for z in 0..16 {
            seed_sky_light_cell(opacity, light, neighbour_sky, out, queue, 0, y, z, -1, y, z as i32);
            seed_sky_light_cell(opacity, light, neighbour_sky, out, queue, 15, y, z, 16, y, z as i32);
        }
        for x in 0..16 {
            seed_sky_light_cell(opacity, light, neighbour_sky, out, queue, x, y, 0, x as i32, y, -1);
            seed_sky_light_cell(opacity, light, neighbour_sky, out, queue, x, y, 15, x as i32, y, 16);
        }
    }
}

fn seed_sky_light_cell(
    opacity: &OpacityNeighborhood,
    light: &LightNeighborhood,
    neighbour_sky: &NeighbourSkyValues,
    out: &mut ChunkLightData,
    queue: &mut VecDeque<usize>,
    local_x: usize,
    y: i32,
    local_z: usize,
    src_x: i32,
    src_y: i32,
    src_z: i32,
) {
    let source = light
        .sky_at(src_x, src_y, src_z)
        .filter(|&v| v > 0)
        .or_else(|| {
            let fallback = sky_value_at(neighbour_sky, src_x, src_y, src_z, out.geometry());
            (fallback > 0).then_some(fallback)
        });
    let Some(source) = source else {
        return;
    };
    if source <= 1 {
        return;
    }
    let index = out.geometry().block_index(local_x, y, local_z);
    let cost = opacity
        .opacity_at(local_x as i32, y, local_z as i32)
        .unwrap_or(0)
        .min(15)
        .max(1);
    let candidate = source.saturating_sub(cost);
    if candidate > out.sky_light()[index] {
        out.set_sky_by_index(index, candidate);
        queue.push_back(index);
    }
}

/// 从邻居独立预计算值中取邻域坐标的天光。
fn sky_value_at(
    neighbour_sky: &NeighbourSkyValues,
    x: i32,
    y: i32,
    z: i32,
    geometry: &LightGeometry,
) -> u8 {
    if !(geometry.world_min_y..=geometry.world_max_y).contains(&y) {
        return 0;
    }
    let Some(index) = LightNeighborhood::chunk_index(x.div_euclid(16), z.div_euclid(16)) else {
        return 0;
    };
    let Some(values) = neighbour_sky[index].as_ref() else {
        return 0;
    };
    values[geometry.block_index(x.rem_euclid(16) as usize, y, z.rem_euclid(16) as usize)]
}

// ─────────────────────────────────────────────
// 天光：BFS 扩散（单 chunk 内）
// ─────────────────────────────────────────────

fn spread_sky(
    chunk: &ChunkOpacity,
    sky: &mut [u8],
    queue: VecDeque<usize>,
    geometry: &LightGeometry,
) {
    let mut queue = queue;
    while let Some(index) = queue.pop_front() {
        let (x, y, z) = unpack_index(index, geometry);
        let source = sky[index];
        if source <= 1 {
            continue;
        }
        for (nx, ny, nz) in neighbours(x, y, z, geometry) {
            let n_index = geometry.block_index(nx, ny, nz);
            let n_opacity = chunk.opacity()[n_index].min(15);
            // 向下且透明：无衰减（天空直射规则）；其余 -max(1, opacity)
            let cost = if ny == y - 1 && n_opacity == 0 { 0 } else { n_opacity.max(1) };
            let candidate = source.saturating_sub(cost);
            if candidate > sky[n_index] {
                sky[n_index] = candidate;
                queue.push_back(n_index);
            }
        }
    }
}

// ─────────────────────────────────────────────
// 天光：射线追踪增强
// ─────────────────────────────────────────────

fn ray_trace_sky(
    opacity: &OpacityNeighborhood,
    _neighbour_sky: &NeighbourSkyValues,
    out: &mut ChunkLightData,
    geometry: &LightGeometry,
) {
    const RAYS: [(i32, i32, i32); 9] = [
        (0, 1, 0),
        (-1, 1, 0), (1, 1, 0),
        (0, 1, -1), (0, 1, 1),
        (-1, 1, -1), (-1, 1, 1),
        (1, 1, -1), (1, 1, 1),
    ];
    let mut queue = VecDeque::new();
    for x in 0..16 {
        for z in 0..16 {
            for y in geometry.world_min_y..=geometry.world_max_y {
                let index = geometry.block_index(x, y, z);
                let old = out.sky_light()[index];
                let mut best = old;
                for &(dx, dy, dz) in &RAYS {
                    best = best.max(trace_sky_ray(opacity, x as i32, y, z as i32, dx, dy, dz));
                    if best == 15 {
                        break;
                    }
                }
                if best > old {
                    out.set_sky_by_index(index, best);
                    queue.push_back(index);
                }
            }
        }
    }
    if let Some(center) = opacity.center() {
        spread_sky(center, out.sky_light_mut(), queue, geometry);
    }
}

/// 沿 (dx,1,dz) 逐格向上追：累计不透明度；出邻域/出 Y 上界 = 开天空。
fn trace_sky_ray(
    opacity: &OpacityNeighborhood,
    x: i32,
    y: i32,
    z: i32,
    dx: i32,
    _dy: i32,
    dz: i32,
) -> u8 {
    let geometry = opacity
        .center()
        .map(|c| *c.geometry())
        .expect("center opacity required");
    let mut cx = x;
    let mut cy = y;
    let mut cz = z;
    let mut loss: u8 = 0;
    loop {
        cx += dx;
        cy += 1;
        cz += dz;
        if cy > geometry.world_max_y {
            return 15_u8.saturating_sub(loss);
        }
        let Some(op) = opacity.opacity_at(cx, cy, cz) else {
            return 15_u8.saturating_sub(loss);
        };
        let op = op.min(15);
        loss = loss.saturating_add(op.max(1));
        if loss >= 15 {
            return 0;
        }
        // 途经有预计算天光值的格：从该格折返的值不优于继续向上追（开天空
        // 15-loss 已是上界），这里直接继续追，保持行为可预测。
    }
}

// ─────────────────────────────────────────────
// 方块光
// ─────────────────────────────────────────────

fn seed_block_from_light(
    opacity: &OpacityNeighborhood,
    light: &LightNeighborhood,
    out: &mut ChunkLightData,
    queue: &mut VecDeque<usize>,
) {
    let geometry = out.geometry();
    for y in geometry.world_min_y..=geometry.world_max_y {
        for z in 0..16 {
            seed_block_cell(opacity, light, out, queue, 0, y, z, -1, y, z as i32);
            seed_block_cell(opacity, light, out, queue, 15, y, z, 16, y, z as i32);
        }
        for x in 0..16 {
            seed_block_cell(opacity, light, out, queue, x, y, 0, x as i32, y, -1);
            seed_block_cell(opacity, light, out, queue, x, y, 15, x as i32, y, 16);
        }
    }
}

fn seed_block_cell(
    opacity: &OpacityNeighborhood,
    light: &LightNeighborhood,
    out: &mut ChunkLightData,
    queue: &mut VecDeque<usize>,
    local_x: usize,
    y: i32,
    local_z: usize,
    src_x: i32,
    src_y: i32,
    src_z: i32,
) {
    let Some(source) = light.block_at(src_x, src_y, src_z) else {
        return;
    };
    if source <= 1 {
        return;
    }
    let index = out.geometry().block_index(local_x, y, local_z);
    let cost = opacity
        .opacity_at(local_x as i32, y, local_z as i32)
        .unwrap_or(0)
        .min(15)
        .max(1);
    let candidate = source.saturating_sub(cost);
    if candidate > out.block_light()[index] {
        out.set_block_by_index(index, candidate);
        queue.push_back(index);
    }
}

fn spread_block(
    chunk: &ChunkOpacity,
    block: &mut [u8],
    queue: VecDeque<usize>,
    geometry: &LightGeometry,
) {
    let mut queue = queue;
    while let Some(index) = queue.pop_front() {
        let (x, y, z) = unpack_index(index, geometry);
        let source = block[index];
        if source <= 1 {
            continue;
        }
        for (nx, ny, nz) in neighbours(x, y, z, geometry) {
            let n_index = geometry.block_index(nx, ny, nz);
            let n_opacity = chunk.opacity()[n_index].min(15);
            let candidate = source.saturating_sub(n_opacity.max(1));
            if candidate > block[n_index] {
                block[n_index] = candidate;
                queue.push_back(n_index);
            }
        }
    }
}

// ─────────────────────────────────────────────
// 坐标工具
// ─────────────────────────────────────────────

fn unpack_index(index: usize, geometry: &LightGeometry) -> (usize, i32, usize) {
    let x = index % 16;
    let rest = index / 16;
    let z = rest % 16;
    let y_offset = (rest / 16) as i32;
    (x, geometry.world_min_y + y_offset, z)
}

fn neighbours(x: usize, y: i32, z: usize, geometry: &LightGeometry) -> impl Iterator<Item = (usize, i32, usize)> {
    [
        (x > 0).then(|| (x - 1, y, z)),
        (x < 15).then(|| (x + 1, y, z)),
        (y > geometry.world_min_y).then(|| (x, y - 1, z)),
        (y < geometry.world_max_y).then(|| (x, y + 1, z)),
        (z > 0).then(|| (x, y, z - 1)),
        (z < 15).then(|| (x, y, z + 1)),
    ]
    .into_iter()
    .flatten()
}
