//! 存档迁移：旧版（1.13–1.17，`Level` 包装布局）区块 -> 1.18+ 现代布局。
//!
//! 迁移是**有损方向明确**的操作：结构按现代格式重组（解包 `Level`、
//! section 存储容器化、生物群系名称化），未知字段尽量保留。已在真实
//! 1.17 存档上验证：迁移后的区块方块数据与源一致。

use std::collections::HashMap;
use std::sync::Arc;

use qexed_nbt::{Tag, tag_id};

use crate::chunk::{
    self, SectionBlocks, biome_states_tag, block_states_tag, int_field,
    data_version, list_tag, section_blocks_spanning, string_field,
};
use crate::error::AnvilError;

/// 1.18 的首个 DataVersion（布局分界参考）。
pub const DATA_VERSION_1_18: i32 = 2860;

/// 迁移选项。
#[derive(Debug, Clone)]
pub struct MigrateOptions {
    /// 迁移后的 DataVersion（默认 3337，即 1.19.4）。
    pub target_data_version: i32,
    /// 生物群系解析失败时的兜底（默认 minecraft:plains）。
    pub fallback_biome: String,
    /// 现代区块是否跳过（默认 true：已是现代格式就不再动）。
    pub skip_modern: bool,
}

impl Default for MigrateOptions {
    fn default() -> Self {
        Self {
            target_data_version: 3337,
            fallback_biome: "minecraft:plains".to_string(),
            skip_modern: true,
        }
    }
}

/// 单区块迁移结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationOutcome {
    /// 旧版区块已转换为现代布局。
    Migrated,
    /// 1.12 扁平 ID 区块已通过 flattening 升级。
    MigratedPreFlattening,
    /// 已是现代布局，按选项跳过（或原地返回）。
    AlreadyModern,
    /// 1.12 及更早的扁平 ID 区块，无法迁移。
    UnsupportedLegacy,
}

/// 旧版（1.13–1.17）数字生物群系 ID -> 现代标识符。
///
/// 表来自 Minecraft Wiki 的 1.16/1.17 ID 约定（0–50 主世界 + 下界/末地常用项）。
pub fn legacy_biome_name(id: i32) -> Option<&'static str> {
    Some(match id {
        0 => "minecraft:ocean",
        1 => "minecraft:plains",
        2 => "minecraft:desert",
        3 => "minecraft:windswept_hills",
        4 => "minecraft:forest",
        5 => "minecraft:taiga",
        6 => "minecraft:swamp",
        7 => "minecraft:river",
        8 => "minecraft:frozen_river",
        9 => "minecraft:snowy_plains",
        10 => "minecraft:mushroom_fields",
        11 => "minecraft:beach",
        12 => "minecraft:jungle",
        13 => "minecraft:sparse_jungle",
        14 => "minecraft:deep_ocean",
        15 => "minecraft:stony_shore",
        16 => "minecraft:snowy_beach",
        17 => "minecraft:birch_forest",
        18 => "minecraft:dark_forest",
        19 => "minecraft:snowy_taiga",
        20 => "minecraft:old_growth_pine_taiga",
        21 => "minecraft:savanna",
        22 => "minecraft:savanna_plateau",
        23 => "minecraft:badlands",
        24 => "minecraft:wooded_badlands",
        25 => "minecraft:small_end_islands",
        26 => "minecraft:end_barrens",
        27 => "minecraft:end_highlands",
        28 => "minecraft:end_midlands",
        29 => "minecraft:the_end",
        30 => "minecraft:frozen_ocean",
        31 => "minecraft:deep_frozen_ocean",
        32 => "minecraft:cold_ocean",
        33 => "minecraft:deep_cold_ocean",
        34 => "minecraft:warm_ocean",
        35 => "minecraft:deep_warm_ocean",
        36 => "minecraft:legacy_frozen_ocean",
        37 => "minecraft:the_void",
        38 => "minecraft:nether_wastes",
        39 => "minecraft:soul_sand_valley",
        40 => "minecraft:crimson_forest",
        41 => "minecraft:warped_forest",
        42 => "minecraft:basalt_deltas",
        _ => return None,
    })
}

/// 迁移一个区块到现代（1.18+）布局。
///
/// 升级链自动衔接：
/// - 1.12 扁平 ID：先 [`crate::upgrade::upgrade_chunk_1_12`]（flattening 到 1.13 布局），
///   再走 1.13→1.18 迁移，返回 [`MigrationOutcome::MigratedPreFlattening`]
/// - 1.13–1.17：解包 `Level`、section 容器化、生物群系名称化等，返回 `Migrated`
/// - 现代：按 `skip_modern` 跳过或原样返回
pub fn migrate_chunk(
    root: &Tag,
    options: &MigrateOptions,
) -> Result<(Tag, MigrationOutcome), AnvilError> {
    let fields = chunk::root_compound(root)?;

    // 1.12 判定：Level.Sections 里存在 Blocks/Data 扁平存储 -> 两段升级
    if is_pre_flattening(fields) {
        let (upgraded, _stats) =
            crate::upgrade::upgrade_chunk_1_12(root, options.target_data_version)?;
        return Ok((upgraded, MigrationOutcome::MigratedPreFlattening));
    }

    let Some(Tag::Compound(level)) = fields.get("Level") else {
        // 现代布局
        return Ok((
            root.clone(),
            MigrationOutcome::AlreadyModern,
        ));
    };
    let level = level.as_ref();
    // 1.13–1.15（DataVersion < 2529）：调色板索引跨 long 打包
    let spanning = data_version(fields).map(|v| v < 2529).unwrap_or(false);

    let mut out: HashMap<String, Tag> = HashMap::new();

    // 根级保留 DataVersion（更新到目标版本）
    out.insert(
        "DataVersion".to_string(),
        Tag::Int(options.target_data_version),
    );

    // Level 内字段提升到根级（旧版字段名换成现代等价名）
    for (key, value) in level {
        match key.as_str() {
            "TileEntities" => {
                out.insert("block_entities".to_string(), value.clone());
            }
            "TileTicks" => {
                out.insert("block_ticks".to_string(), value.clone());
            }
            "LiquidTicks" => {
                out.insert("fluid_ticks".to_string(), value.clone());
            }
            "Sections" | "Biomes" => {} // 单独处理
            "Status" => {
                // 补命名空间：旧版 "full" -> "minecraft:full"
                let status = string_field(level, "Status").unwrap_or("minecraft:full");
                let namespaced = if status.contains(':') {
                    status.to_string()
                } else {
                    format!("minecraft:{status}")
                };
                out.insert("Status".to_string(), Tag::String(Arc::from(namespaced)));
            }
            _ => {
                out.insert(key.clone(), value.clone());
            }
        }
    }

    // xPos/zPos 必须存在（现代格式要求）
    out.entry("xPos".to_string()).or_insert(Tag::Int(0));
    out.entry("zPos".to_string()).or_insert(Tag::Int(0));
    // yPos：现代格式必需。旧版没有整体 yPos，取 section 列表的最小 Y。
    let sections = legacy_sections(level);
    let min_section_y = sections
        .iter()
        .filter_map(|(y, _)| *y)
        .min()
        .unwrap_or(-4);
    out.insert("yPos".to_string(), Tag::Int(min_section_y));

    // 迁移 sections
    let biomes = legacy_biomes(level, &sections);
    let mut modern_sections = Vec::with_capacity(sections.len());
    for (section_y, section) in &sections {
        let Some(section_y) = *section_y else { continue };
        modern_sections.push(migrate_section(section_y, section, &biomes, spanning, options)?);
    }
    modern_sections.sort_by_key(|section: &Tag| match section {
        Tag::Compound(fields) => int_field(fields, "Y").unwrap_or(i32::MAX),
        _ => i32::MAX,
    });
    out.insert(
        "sections".to_string(),
        list_tag(tag_id::COMPOUND, modern_sections),
    );

    Ok((Tag::Compound(Arc::new(out)), MigrationOutcome::Migrated))
}

/// 旧版 section -> 现代 section。
fn migrate_section(
    section_y: i32,
    section: &HashMap<String, Tag>,
    biomes: &LegacyBiomes,
    spanning: bool,
    options: &MigrateOptions,
) -> Result<Tag, AnvilError> {
    let mut out: HashMap<String, Tag> = HashMap::new();
    out.insert("Y".to_string(), Tag::Byte(section_y as i8));

    // 光照等其他字段保留
    for (key, value) in section {
        if matches!(key.as_str(), "Y" | "Palette" | "BlockStates") {
            continue;
        }
        out.insert(key.clone(), value.clone());
    }

    // 方块存储：读旧版 -> 写新版容器
    let blocks: SectionBlocks = section_blocks_spanning(section, spanning)?;
    if section.contains_key("Palette") {
        out.insert(
            "block_states".to_string(),
            block_states_tag(&blocks.palette, &blocks.indices)?,
        );
    }

    // 生物群系：旧版 int[1024] -> 该 section 中心层的主导值 -> 名称调色板
    let biome_name = biomes
        .dominant_at(section_y)
        .and_then(legacy_biome_name)
        .map(str::to_string)
        .unwrap_or_else(|| options.fallback_biome.clone());
    out.insert("biomes".to_string(), biome_states_tag(&[biome_name], None));

    Ok(Tag::Compound(Arc::new(out)))
}

/// 旧版 `Biomes` 数组的包装（1.13–1.14 byte[256]；1.15–1.17 int[1024] 及自定义高度的变长）。
struct LegacyBiomes {
    /// 线性数组（16x16xN）；空 = 没有 Biomes 字段。
    data: Vec<i32>,
    /// 区块最低世界高度（sections 的最小 Y * 16；缺省 0）。
    min_y: i32,
    /// 区块最高世界高度（(最大 section Y + 1) * 16；缺省 256）。
    max_y: i32,
}

impl LegacyBiomes {
    fn from_level(level: &HashMap<String, Tag>, sections: &[(Option<i32>, &HashMap<String, Tag>)]) -> Self {
        let data = match level.get("Biomes") {
            Some(Tag::IntArray(values)) => values.to_vec(),
            // 1.13-1.14 用 byte[256]（每列一个）
            Some(Tag::ByteArray(bytes)) => bytes.iter().map(|&b| i32::from(b)).collect(),
            _ => Vec::new(),
        };
        let ys: Vec<i32> = sections.iter().filter_map(|(y, _)| *y).collect();
        let min_y = ys.iter().min().map(|y| y * 16).unwrap_or(0);
        let max_y = ys.iter().max().map(|y| (y + 1) * 16).unwrap_or(256);
        Self { data, min_y, max_y }
    }

    /// 取某个 section 的主导生物群系。
    ///
    /// 数组为 16×16×N 单元（N = len/256）。单元在 [min_y, max_y) 上均匀分布；
    /// 取 section 中心所在单元层的 256 格主导值。byte[256]（N=1）时全高度同一个值。
    fn dominant_at(&self, section_y: i32) -> Option<i32> {
        if self.data.is_empty() {
            return None;
        }
        let columns = 256usize;
        let cells = self.data.len() / columns;
        if cells == 0 {
            return None;
        }
        let height = (self.max_y - self.min_y).max(1) as usize;
        let cell_height = (height / cells).max(1);
        let center_y = section_y * 16 + 8;
        let layer = ((center_y - self.min_y).clamp(0, height as i32 - 1) as usize / cell_height).min(cells - 1);
        let mut counts: HashMap<i32, usize> = HashMap::new();
        for z in 0..16usize {
            for x in 0..16usize {
                let index = (layer * 16 + z) * 16 + x;
                if let Some(&id) = self.data.get(index) {
                    *counts.entry(id).or_insert(0) += 1;
                }
            }
        }
        counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(id, _)| id)
    }
}
fn legacy_biomes(level: &HashMap<String, Tag>, sections: &[(Option<i32>, &HashMap<String, Tag>)]) -> LegacyBiomes {
    LegacyBiomes::from_level(level, sections)
}

fn legacy_sections(level: &HashMap<String, Tag>) -> Vec<(Option<i32>, &HashMap<String, Tag>)> {
    let Some(Tag::List(_, items)) = level.get("Sections") else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| match item {
            Tag::Compound(section) => Some((int_field(section, "Y"), section.as_ref())),
            _ => None,
        })
        .collect()
}

fn is_pre_flattening(fields: &HashMap<String, Tag>) -> bool {
    let Some(Tag::Compound(level)) = fields.get("Level") else {
        return false;
    };
    let Some(Tag::List(_, items)) = level.get("Sections") else {
        return false;
    };
    items.iter().any(|item| match item {
        Tag::Compound(section) => section.contains_key("Blocks") || section.contains_key("Data"),
        _ => false,
    })
}
// ---------------------------------------------------------------------------
// 区域文件 / 世界级迁移
// ---------------------------------------------------------------------------

/// 迁移统计。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrationStats {
    /// 迁移为现代布局的区块数。
    pub migrated: usize,
    /// 已是现代布局的区块数。
    pub modern: usize,
    /// 1.12 及更早、无法迁移的区块数。
    pub unsupported: usize,
    /// 解析失败被跳过的区块数（保持原样，不丢数据）。
    pub failed: usize,
}

impl MigrationStats {
    pub fn total(&self) -> usize {
        self.migrated + self.modern + self.unsupported + self.failed
    }
}

/// 就地迁移一个区域文件（.mca）：读全部区块 -> 迁移 -> 重新写回。
///
/// 解析失败的区块保持原样（不重写，不丢数据）；返回统计。
pub fn migrate_region_file(
    path: impl AsRef<std::path::Path>,
    options: &MigrateOptions,
) -> Result<MigrationStats, AnvilError> {
    use crate::region::{AnvilRegion, ChunkData};

    let path = path.as_ref();
    let mut region = AnvilRegion::from_file(path)?;
    let mut stats = MigrationStats::default();

    for (lx, lz) in region.chunk_coords() {
        let Some(data) = region.read_chunk(lx, lz)? else { continue };
        let Ok(raw) = data.decompress() else {
            stats.failed += 1;
            continue;
        };
        let Ok((_, root)) = qexed_nbt::from_slice_lossy(&raw) else {
            stats.failed += 1;
            continue;
        };

        match migrate_chunk(&root, options) {
            Ok((migrated, MigrationOutcome::Migrated)) => {
                let bytes = qexed_nbt::to_vec("", &migrated)?;
                region.write_chunk(lx, lz, ChunkData::zlib(&bytes)?)?;
                stats.migrated += 1;
            }
            Ok((_, MigrationOutcome::MigratedPreFlattening)) => stats.migrated += 1,
            Ok((_, MigrationOutcome::AlreadyModern)) => stats.modern += 1,
            Ok((_, MigrationOutcome::UnsupportedLegacy)) => stats.unsupported += 1,
            Err(_) => stats.failed += 1,
        }
    }

    region.save()?;
    Ok(stats)
}

/// 迁移一个维度的全部区域文件。
///
/// 返回 (处理的文件数, 统计合计)。
pub fn migrate_dimension(
    save_root: &std::path::Path,
    dimension: &str,
    options: &MigrateOptions,
) -> Result<(usize, MigrationStats), AnvilError> {
    use crate::paths;

    let files = paths::list_region_files(save_root, dimension)?;
    let mut total = MigrationStats::default();
    let mut count = 0;
    for file in files {
        let stats = migrate_region_file(&file, options)?;
        total.migrated += stats.migrated;
        total.modern += stats.modern;
        total.unsupported += stats.unsupported;
        total.failed += stats.failed;
        count += 1;
    }
    Ok((count, total))
}
