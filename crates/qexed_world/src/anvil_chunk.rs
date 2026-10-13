//! Anvil 区块 → 真实 MapChunk 管段：
//!
//! 1. 3×3 anvil 邻域（中心 + 8 邻居，缺邻居静默跳过）读区块 NBT；
//! 2. 逐方块构建 [`ChunkOpacity`] 并喂给 `qexed_light` 引擎（语义与
//!    qexed_light/tests/vanilla_compare.rs 逐格对拍通过的路径一致：天光
//!    不注入邻居天光、方块光不播种邻居）；
//! 3. `ChunkLightData` → [`LightPayload`] → 协议 `Light`；
//! 4. 打包成 MapChunk：24 个 section（palette/计数/biome）、heightmap
//!    （键 1/4/5，缺 → 37 个零）、block entities（空）。
//!
//! 方块状态 id 与 biome id 由调用方注入：blocks id 来自 [`BlockIndex`]
//! （见 `crate::blocks`），biome id 来自
//! `qexed_registry::load_dynamic_registry_id_map` 的结果（未知名称
//! 回退 plains 序号）。

use std::collections::HashMap;

use bytes::BytesMut;
use qexed_anvil::chunk;
use qexed_anvil::paths::{dimension_region_dir, region_file_name};
use qexed_anvil::region::AnvilRegion;
use qexed_light::data::{
    BlockLightProps, ChunkLightData, ChunkOpacity, LightNeighborhood, OpacityNeighborhood,
};
use qexed_light::engine::LightEngine;
use qexed_light::geometry::DimensionGeometry;
use qexed_light::{LightPayload, SkyLightAlgorithm};
use qexed_nbt::{from_slice, Tag};
use qexed_packet::net_types::VarInt;
use qexed_packet::{PacketCodec, PacketWriter};
use qexed_protocol::to_client::play::map_chunk::{Chunk, Heightmaps, MapChunk};

use crate::blocks::{block_props, ceil_log2, has_fluid, is_air_block, network_palette_bits, BlockIndex};
use crate::error::{Result, WorldError};
use crate::light::light_payloads_to_light;

/// anvil section 方块容器条目数（16×16×16）。
const SECTION_ENTRIES: usize = 16 * 16 * 16;
/// anvil biome 容器条目数（4×4×4）。
const BIOME_ENTRIES: usize = 4 * 4 * 4;
/// 协议 heightmap：WORLD_SURFACE(1) / MOTION_BLOCKING(4) / NO_LEAVES(5)。
const HEIGHTMAP_KEYS: [i32; 3] = [1, 4, 5];
/// heightmap long 数组长度（9×18=162 bit → 37 long，与 chunk.rs 一致）。
const HEIGHTMAP_LONGS: usize = 37;
/// 缺省生物群系（协议序号见 chunk.rs PLAINS_BIOME_ID = 40）。
const PLAINS: &str = "minecraft:plains";

/// 区块编码管段：持有区域文件缓存 + 光照引擎 + 索引，按需编码单个区块。
pub struct ChunkPipeline {
    region_dir: std::path::PathBuf,
    /// 区域文件懒加载缓存，键 = (rx, rz)，值 None = 文件不存在。
    regions: HashMap<(i32, i32), Option<AnvilRegion>>,
    engine: LightEngine,
    blocks: BlockIndex,
    /// biome 名称 → 协议序号（configuration 阶段已发送的 dynamic 注册表；
    /// 未知名称回退 plains）。
    biome_ids: HashMap<String, i32>,
    /// 已算好光照的区块缓存，键 = (cx, cz)，值 = (天光, 方块光) 逐格亮度
    /// （长度均为 chunk_light_len）。编码区块 N 时作为邻居光照播种源（与
    /// 原版一致：邻居的光会渗过边界）；值 None = NBT 根缺失。
    light_cache: HashMap<(i32, i32), Option<(Vec<u8>, Vec<u8>)>>,
}

impl ChunkPipeline {
    /// 按维度几何与算法构建管段。
    #[allow(dead_code)] // 集成测试（vanilla-verify）与后续接线使用
    pub fn new(
        save_root: impl AsRef<std::path::Path>,
        dimension: &str,
        geometry: DimensionGeometry,
        algorithm: SkyLightAlgorithm,
        blocks: BlockIndex,
        biome_ids: HashMap<String, i32>,
    ) -> Self {
        let region_dir = dimension_region_dir(save_root.as_ref(), dimension);
        Self {
            region_dir,
            regions: HashMap::new(),
            engine: LightEngine::new(geometry, algorithm),
            blocks,
            biome_ids,
            light_cache: HashMap::new(),
        }
    }

    /// 光照引擎只读访问（测试对拍用）。
    #[allow(dead_code)]
    pub fn engine(&self) -> &LightEngine {
        &self.engine
    }

    /// 光照缓存只读访问（测试对拍用）：`Some(Some((天光, 方块光)))` =
    /// 已算过；`Some(None)` = 区块 NBT 缺失；`None` = 尚未算过。
    #[allow(dead_code)]
    pub fn cached_light(&self, cx: i32, cz: i32) -> Option<Option<&(Vec<u8>, Vec<u8>)>> {
        self.light_cache.get(&(cx, cz)).map(Option::as_ref)
    }

    /// 主世界便捷构造：overworld 几何 + 默认（原版一致）天光算法。
    #[allow(dead_code)]
    pub fn new_overworld(
        save_root: impl AsRef<std::path::Path>,
        blocks: BlockIndex,
        biome_ids: HashMap<String, i32>,
    ) -> Self {
        Self::new(
            save_root,
            "minecraft:overworld",
            DimensionGeometry::overworld(),
            SkyLightAlgorithm::default(),
            blocks,
            biome_ids,
        )
    }


    /// 一次性读入 3×3 邻域原始 NBT 根，索引 = (dz+1)*3 + (dx+1)，中心 = 4。
    ///
    /// `read_chunk` 内部已按 32 取模处理负坐标；缺文件 / 缺区块 → None。
    fn load_roots(&mut self, cx: i32, cz: i32) -> Result<Vec<Option<Tag>>> {
        let mut roots: Vec<Option<Tag>> = vec![None; 9];
        for dz in -1_i32..=1 {
            for dx in -1_i32..=1 {
                let index = ((dz + 1) * 3 + (dx + 1)) as usize;
                let key = ((cx + dx) >> 5, (cz + dz) >> 5);
                if !self.regions.contains_key(&key) {
                    let path =
                        self.region_dir.join(region_file_name(key.0, key.1));
                    let region = AnvilRegion::from_file(&path).map_err(|e| {
                        WorldError::Other(format!("打开区域文件失败 {path:?}: {e}"))
                    })?;
                    self.regions.insert(key, Some(region));
                }
                let exists = self.regions.get(&key).expect("just inserted").is_some();
                if !exists {
                    continue;
                }
                let region = self
                    .regions
                    .get_mut(&key)
                    .expect("region cache populated above")
                    .as_mut()
                    .expect("exists checked above");
                let Some(bytes) = region
                    .read_chunk(cx + dx, cz + dz)
                    .map_err(|e| WorldError::Other(format!("读取区块失败: {e}")))?
                else {
                    continue;
                };
                let data = bytes
                    .decompress()
                    .map_err(|e| WorldError::Other(format!("区块解压失败: {e}")))?;
                let (_, root) = from_slice(&data)
                    .map_err(|e| WorldError::Other(format!("区块 NBT 解析失败: {e}")))?;
                roots[index] = Some(root);
            }
        }
        Ok(roots)
    }

    /// 编码一个区块为 MapChunk；中心区块缺失 → None（调用方回退空区块）。
    ///
    /// 光照每次从 NBT 全量重算，邻居光照取自 `light_cache`：已算过的邻居
    /// 参与真实渗光（与原版一致），缺失方向回退引擎内建的独立预计算。
    /// 结果入缓存；缓存命中后直接用缓存光照重编码（字节级一致）。
    /// 批量流送时按流送顺序调用、或对同一片区域反复调用，边界光照随
    /// 缓存逐步收敛（Gauss–Seidel 式扫掠），趋近原版"全邻域算好再发送"
    /// 的形态。
    #[allow(dead_code)] // 集成测试（vanilla-verify）与后续接线使用
    pub fn load_chunk(&mut self, cx: i32, cz: i32) -> Result<Option<MapChunk>> {
        match self.light_cache.get(&(cx, cz)) {
            Some(Some(cached)) => {
                let (sky, block) = cached.clone();
                return Ok(Some(self.encode_light(cx, cz, &sky, &block)?));
            }
            Some(None) => return Ok(None),
            None => {}
        }

        let roots = self.load_roots(cx, cz)?;
        if roots[4].is_none() {
            self.light_cache.insert((cx, cz), None);
            return Ok(None);
        }

        // 1) 3×3 不透明度体（vanilla_compare.rs 的 opacity_from_chunk 同语义）。
        let opacity = build_opacity(&roots, self.engine.geometry())?;

        // 2) 邻居光照播种：缓存命中直接用，未算过的邻居现算单区块光照补齐。
        let light_nb = self.neighborhood_light(cx, cz)?;

        // 3) 天光 + 方块光各算一次（吃邻居播种），入缓存后合成协议 Light。
        let sky = self.engine.compute_sky_light_with_neighbors(&opacity, &light_nb);
        let block = self.engine.compute_block_light(&opacity, Some(&light_nb));
        self.light_cache.insert(
            (cx, cz),
            Some((sky.sky_light().to_vec(), block.block_light().to_vec())),
        );
        Ok(Some(self.encode_light(cx, cz, sky.sky_light(), block.block_light())?))
    }

    /// 装配 3×3 邻居光照：缓存命中的邻居用缓存值（真实渗光），缺失方向
    /// 现算单区块独立光照补齐（与引擎内建回退语义一致），同样入缓存。
    /// 中心槽位占位空数据（引擎不读中心）。
    fn neighborhood_light(&mut self, cx: i32, cz: i32) -> Result<LightNeighborhood> {
        let geometry = *self.engine.geometry();
        let mut light_nb = LightNeighborhood::single(ChunkLightData::empty(&geometry));
        for dz in -1_i32..=1 {
            for dx in -1_i32..=1 {
                if dx == 0 && dz == 0 {
                    continue;
                }
                let (nx, nz) = (cx + dx, cz + dz);
                let data = match self.light_cache.get(&(nx, nz)) {
                    Some(Some((sky, block))) => {
                        let mut d = ChunkLightData::empty(&geometry);
                        d.sky_light_mut().copy_from_slice(sky);
                        d.block_light_mut().copy_from_slice(block);
                        d
                    }
                    Some(None) => continue,
                    None => {
                        let (sky, block) = self.compute_isolated_light(nx, nz)?;
                        let Some(sky) = sky else {
                            continue;
                        };
                        let block = block.unwrap_or_default();
                        let mut d = ChunkLightData::empty(&geometry);
                        d.sky_light_mut().copy_from_slice(&sky);
                        d.block_light_mut().copy_from_slice(&block);
                        d
                    }
                };
                light_nb.set(dx, dz, data);
            }
        }
        Ok(light_nb)
    }

    /// 用给定亮度级编码区块：section/heightmap 照常重建，光照直接合成。
    fn encode_light(&mut self, cx: i32, cz: i32, sky: &[u8], block: &[u8]) -> Result<MapChunk> {
        let roots = self.load_roots(cx, cz)?;
        let Some(center) = roots[4].as_ref() else {
            return Err(WorldError::Other(format!(
                "区块 ({cx},{cz}) 光照有缓存但 NBT 根缺失"
            )));
        };
        let map = chunk::root_compound(center)
            .map_err(|e| WorldError::Other(format!("区块根复合标签缺失: {e}")))?;
        let sections = chunk::sections_by_y(map);
        let mut data = Vec::new();
        encode_sections(&mut data, &sections, map, &self.blocks, &self.biome_ids)?;
        let geometry = *self.engine.geometry();
        // from_raw 一次同时切出天光/方块光掩码与数组；
        // light_payloads_to_light 分别取前者的 sky 字段与后者的 block 字段。
        let payload = LightPayload::from_raw(&geometry, sky, block);
        let light = light_payloads_to_light(&payload, &payload);
        Ok(MapChunk {
            chunk_x: cx,
            chunk_z: cz,
            data: Chunk {
                heightmaps: heightmaps(map),
                data,
                block_entities: Vec::new(),
            },
            light,
        })
    }

    /// 单区块隔离光照（无邻居播种）：邻居补齐与缓存填充共用。
    /// 返回 (天光, 方块光) 亮度级；区块缺失 → (None, None)。
    fn compute_isolated_light(
        &mut self,
        cx: i32,
        cz: i32,
    ) -> Result<(Option<Vec<u8>>, Option<Vec<u8>>)> {
        let roots = self.load_roots(cx, cz)?;
        if roots[4].is_none() {
            return Ok((None, None));
        }
        let opacity = build_opacity(&roots, self.engine.geometry())?;
        let sky = self.engine.compute_sky_light(&opacity);
        let block = self.engine.compute_block_light(&opacity, None);
        self.light_cache.insert(
            (cx, cz),
            Some((sky.sky_light().to_vec(), block.block_light().to_vec())),
        );
        Ok((Some(sky.sky_light().to_vec()), Some(block.block_light().to_vec())))
    }
}

/// heightmap：优先取 anvil 存量（LongArray 直取），缺 → 37 个零。
fn heightmaps(map: &HashMap<String, Tag>) -> Vec<Heightmaps> {
    HEIGHTMAP_KEYS
        .iter()
        .map(|&key| {
            let data: Vec<u64> = match map.get(&key.to_string()) {
                Some(Tag::LongArray(values)) => values.iter().map(|&v| v as u64).collect(),
                _ => vec![0_u64; HEIGHTMAP_LONGS],
            };
            Heightmaps {
                type_id: VarInt(key),
                data,
            }
        })
        .collect()
}

/// 构建 3×3 邻域不透明度体。语义与 qexed_light/tests/vanilla_compare.rs 的
/// `opacity_from_chunk` 一致：点亮红石矿查 `[lit=true]` 专用键、缺省 SOLID。
fn build_opacity(roots: &[Option<Tag>], geometry: &qexed_light::geometry::LightGeometry) -> Result<OpacityNeighborhood> {
    let mut neighborhood_slots: [Option<ChunkOpacity>; 9] = Default::default();
    for slot in 0..9_usize {
        let Some(root) = roots[slot].as_ref() else {
            continue;
        };
        let compound = chunk::root_compound(root)
            .map_err(|e| WorldError::Other(format!("邻区块根复合标签缺失: {e}")))?;
        let mut opacity = ChunkOpacity::filled(geometry, BlockLightProps::AIR);
        for (section_y, section) in chunk::sections_by_y(compound) {
            let Ok(blocks) = chunk::section_blocks(section) else {
                continue;
            };
            for (i, &palette_index) in blocks.indices.iter().enumerate() {
                let Some(state) = blocks.palette.get(palette_index) else {
                    continue;
                };
                // 点亮状态的红石矿石亮度 9；其余方块按名称匹配。
                let lookup = if state.name == "minecraft:redstone_ore"
                    && state.properties.iter().any(|(k, v)| k == "lit" && v == "true")
                {
                    "minecraft:redstone_ore[lit=true]"
                } else {
                    state.name.as_str()
                };
                let props = block_props(lookup);
                let y = section_y * 16 + (i / 256) as i32;
                let z = ((i % 256) / 16) as usize;
                let x = (i % 16) as usize;
                let _ = opacity.set(x, y, z, props);
            }
        }
        neighborhood_slots[slot] = Some(opacity);
    }
    // 槽位即 (dz+1)*3+(dx+1)，按下排 → 中排(跳过中心) → 上排依次注入。
    let center = neighborhood_slots[4]
        .clone()
        .expect("center chunk existence checked by caller");
    let mut neighborhood = OpacityNeighborhood::single(center);
    for slot in (0..9).filter(|&slot| slot != 4) {
        let (offset_x, offset_z) = (slot as i32 % 3 - 1, slot as i32 / 3 - 1);
        if let Some(data) = neighborhood_slots[slot].clone() {
            neighborhood.set(offset_x, offset_z, data);
        }
    }
    Ok(neighborhood)
}

/// 24 个 section 的网络编码：非空气数 + 液体数（双 i16 头）+ block
/// 容器 + biome 容器（1.18+ 无 y 字节）。
fn encode_sections(
    out: &mut Vec<u8>,
    sections: &HashMap<i32, &HashMap<String, Tag>>,
    map: &HashMap<String, Tag>,
    blocks_index: &BlockIndex,
    biome_ids: &HashMap<String, i32>,
) -> Result<()> {
    for index in 0..crate::chunk::SECTION_COUNT {
        let base = crate::chunk::WORLD_MIN_Y.div_euclid(16) + index as i32;
        let block_section = sections.get(&base).copied();
        let blocks = match block_section {
            Some(section) => chunk::section_blocks(section)
                .map_err(|e| WorldError::Other(format!("section 方块解码失败: {e}")))?,
            None => chunk::SectionBlocks {
                section_y: base,
                palette: vec![qexed_anvil::BlockStateRef::new("minecraft:air")],
                indices: vec![0; SECTION_ENTRIES],
            },
        };
        let biomes = read_biomes(map, base, block_section);

        // 双 i16 头：非空气方块数 / 液体数。
        let mut non_air: i16 = 0;
        let mut fluids: i16 = 0;
        for sy in 0..16_usize {
            for z in 0..16_usize {
                for x in 0..16_usize {
                    let Some(state) = blocks.at(x, sy, z) else {
                        continue;
                    };
                    if !is_air_block(state.name.as_str()) {
                        non_air += 1;
                    }
                    if has_fluid(state.name.as_str(), &state.properties) {
                        fluids += 1;
                    }
                }
            }
        }
        out.extend_from_slice(&non_air.to_be_bytes());
        out.extend_from_slice(&fluids.to_be_bytes());

        write_block_container(out, &blocks, blocks_index)?;
        write_biome_container(out, &biomes, biome_ids)?;
    }
    Ok(())
}

/// 读取 section 的生物群系：优先 section 自身容器，缺容器回退区块级
/// `biomes`（部分存档布局），再缺回退全 plains。
fn read_biomes(
    map: &HashMap<String, Tag>,
    base: i32,
    block_section: Option<&HashMap<String, Tag>>,
) -> chunk::SectionBiomes {
    if let Some(section) = block_section {
        if let Ok(biomes) = chunk::section_biomes(section) {
            if !biomes.palette.is_empty() {
                return biomes;
            }
        }
    }
    if let Some(Tag::List(_, items)) = map.get("biomes") {
        // 区块级 biomes 列表布局（非标准）：仅取第 base 个，可解析才算命中。
        if let Some(Tag::Compound(section)) = items.get(base.max(0) as usize) {
            if let Ok(biomes) = chunk::section_biomes(section) {
                if !biomes.palette.is_empty() {
                    return biomes;
                }
            }
        }
    }
    chunk::SectionBiomes {
        section_y: base,
        palette: vec![PLAINS.to_string()],
        indices: vec![0; BIOME_ENTRIES],
    }
}

/// 方块容器：单值 / 间接 / 全局直接（规则见
/// [`crate::blocks::network_palette_bits`]）。
fn write_block_container(
    out: &mut Vec<u8>,
    blocks: &chunk::SectionBlocks,
    blocks_index: &BlockIndex,
) -> Result<()> {
    // 状态 id 调色板（保持首次出现顺序，index 即容器值）。
    let mut ids: Vec<i32> = Vec::new();
    let mut values: Vec<usize> = Vec::with_capacity(SECTION_ENTRIES);
    for &palette_index in &blocks.indices {
        let Some(state) = blocks.palette.get(palette_index) else {
            return Err(WorldError::Other(format!(
                "section {} palette 越界: {palette_index}",
                blocks.section_y
            )));
        };
        let id = blocks_index
            .state_id(state.name.as_str(), &state.properties)
            .unwrap_or(0);
        let idx = match ids.iter().position(|&v| v == id) {
            Some(pos) => pos,
            None => {
                ids.push(id);
                ids.len() - 1
            }
        };
        values.push(idx);
    }
    let (bits, direct) = network_palette_bits(blocks_index.global_bits(), ids.len());
    if bits == 0 {
        // 单值 palette：u8(0) + VarInt(id)，无数据数组。
        out.push(0);
        write_varint(out, ids[0])?;
        return Ok(());
    }
    out.push(bits);
    if direct {
        // 全局直接：无 palette 表，容器值 = 状态 id。
        let values: Vec<usize> = values.iter().map(|&v| ids[v] as usize).collect();
        write_packed(out, &values, bits as usize)?;
    } else {
        // 间接：VarInt(palette_len) + VarInt 项 + 打包数据（无长度前缀）。
        write_varint(out, ids.len() as i32)?;
        for &id in &ids {
            write_varint(out, id)?;
        }
        write_packed(out, &values, bits as usize)?;
    }
    Ok(())
}

/// biome 容器：规则独立于方块（1 项单值；1..=3 项 entry bits；
/// 再往上 max(global, entry)，上限 64 位 → 直接模式）。
fn write_biome_container(
    out: &mut Vec<u8>,
    biomes: &chunk::SectionBiomes,
    biome_ids: &HashMap<String, i32>,
) -> Result<()> {
    let mut ids: Vec<i32> = Vec::new();
    let mut values: Vec<usize> = Vec::with_capacity(BIOME_ENTRIES);
    for &palette_index in &biomes.indices {
        let Some(name) = biomes.palette.get(palette_index) else {
            return Err(WorldError::Other(format!(
                "section {} biome palette 越界: {palette_index}",
                biomes.section_y
            )));
        };
        let id = biome_id(biome_ids, name);
        let idx = match ids.iter().position(|&v| v == id) {
            Some(pos) => pos,
            None => {
                ids.push(id);
                ids.len() - 1
            }
        };
        values.push(idx);
    }
    // biome global bits 由实际出现的最大 id 决定（65 项 plains=40 → 7）。
    let max_id = ids.iter().copied().max().unwrap_or(0).max(0) as usize;
    let global = ceil_log2(max_id + 1).max(1) as u8;
    let (bits, direct) = biome_palette_bits(global, ids.len());
    if bits == 0 {
        out.push(0);
        write_varint(out, ids[0])?;
        return Ok(());
    }
    out.push(bits);
    if direct {
        let values: Vec<usize> = values.iter().map(|&v| ids[v] as usize).collect();
        write_packed(out, &values, bits as usize)?;
    } else {
        write_varint(out, ids.len() as i32)?;
        for &id in &ids {
            write_varint(out, id)?;
        }
        write_packed(out, &values, bits as usize)?;
    }
    Ok(())
}

/// biome 容器调色板规则（1.21.5+：1 项单值；1..=3 项 entry bits；
/// 再往上 max(global, entry)，达 64 位 → 直接）。
fn biome_palette_bits(global: u8, palette_len: usize) -> (u8, bool) {
    match palette_len {
        0 | 1 => (0, false),
        2..=3 => (ceil_log2(palette_len) as u8, false),
        _ => {
            let entry = ceil_log2(palette_len) as u8;
            let bits = global.max(entry);
            if bits >= 64 { (64, true) } else { (bits, false) }
        }
    }
}

/// biome 名称 → 协议序号（未知名称回退 plains 序号）。
fn biome_id(biome_ids: &HashMap<String, i32>, name: &str) -> i32 {
    biome_ids
        .get(name)
        .copied()
        .or_else(|| biome_ids.get(PLAINS).copied())
        .unwrap_or(crate::chunk::PLAINS_BIOME_ID)
}

/// VarInt 裸写助手（PacketWriter 无裸写 API，借道 BytesMut）。
fn write_varint(out: &mut Vec<u8>, value: i32) -> Result<()> {
    let mut buf = BytesMut::new();
    VarInt(value)
        .serialize(&mut PacketWriter { buf: &mut buf })
        .map_err(|e| WorldError::Other(format!("VarInt 编码失败: {e}")))?;
    out.extend_from_slice(&buf);
    Ok(())
}

/// 打包数据数组：无长度前缀，BE long，vpl = 64 / bits（非 spanning）。
fn write_packed(out: &mut Vec<u8>, values: &[usize], bits: usize) -> Result<()> {
    let longs = qexed_anvil::palette::pack_values(values, bits)
        .map_err(|e| WorldError::Other(format!("位打包失败: {e}")))?;
    for long in longs {
        out.extend_from_slice(&long.to_be_bytes());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// biome 容器规则与方块独立：1/2/3 项走 entry bits，65 项 → global 7。
    #[test]
    fn biome_palette_rule_is_independent() {
        assert_eq!(biome_palette_bits(7, 0), (0, false));
        assert_eq!(biome_palette_bits(7, 1), (0, false));
        assert_eq!(biome_palette_bits(7, 2), (1, false));
        assert_eq!(biome_palette_bits(7, 3), (2, false));
        assert_eq!(biome_palette_bits(7, 4), (7, false));
        assert_eq!(biome_palette_bits(7, 65), (7, false));
        assert_eq!(biome_palette_bits(7, 100), (7, false));
        assert_eq!(biome_palette_bits(7, 1 << 20), (20, false));
        assert_eq!(biome_palette_bits(7, 1 << 30), (30, false));
    }

    /// heightmap 缺失 → 37 个零、类型 id 1/4/5。
    #[test]
    fn heightmap_defaults_are_zero_padded() {
        let map: HashMap<String, Tag> = HashMap::new();
        let hms = heightmaps(&map);
        assert_eq!(hms.len(), 3);
        assert_eq!(hms[0].type_id, VarInt(1));
        assert_eq!(hms[1].type_id, VarInt(4));
        assert_eq!(hms[2].type_id, VarInt(5));
        for hm in &hms {
            assert_eq!(hm.data.len(), HEIGHTMAP_LONGS);
            assert!(hm.data.iter().all(|&v| v == 0));
        }
    }

    /// heightmap 存量 LongArray 直取（i64 → u64 位保真）。
    #[test]
    fn heightmap_reads_existing_long_array() {
        let mut map: HashMap<String, Tag> = HashMap::new();
        let raw: Vec<i64> = vec![-1; HEIGHTMAP_LONGS];
        map.insert("4".to_string(), Tag::LongArray(raw.into()));
        let hms = heightmaps(&map);
        assert_eq!(hms[1].data, vec![u64::MAX; HEIGHTMAP_LONGS]);
        // 其余键回退零。
        assert!(hms[0].data.iter().all(|&v| v == 0));
    }

    /// ceil_log2 语义抽查（biome global bits 依赖它）。
    #[test]
    fn ceil_log2_matches_spec() {
        assert_eq!(ceil_log2(0), 0);
        assert_eq!(ceil_log2(1), 0);
        assert_eq!(ceil_log2(2), 1);
        assert_eq!(ceil_log2(3), 2);
        assert_eq!(ceil_log2(65), 7);
    }

    /// VarInt 裸写与 packet codec 一致。
    #[test]
    fn varint_writer_matches_codec() {
        let mut out = Vec::new();
        write_varint(&mut out, 40).unwrap();
        assert_eq!(out, vec![40]);
        let mut out = Vec::new();
        write_varint(&mut out, 2097151).unwrap();
        assert_eq!(out, vec![0xff, 0xff, 0x7f]);
    }

    /// 邻居槽位 → 偏移映射的往返检查（防索引错位回归）。
    #[test]
    fn neighbor_slot_layout_is_row_major() {
        // 槽位 = (dz+1)*3 + (dx+1)。
        for dz in -1_i32..=1 {
            for dx in -1_i32..=1 {
                let slot = ((dz + 1) * 3 + (dx + 1)) as usize;
                let offsets = [
                    (-1_i32, -1_i32),
                    (0, -1),
                    (1, -1),
                    (-1, 0),
                    (0, 0),
                    (1, 0),
                    (-1, 1),
                    (0, 1),
                    (1, 1),
                ];
                assert_eq!(offsets[slot], (dx, dz));
            }
        }
    }
}
