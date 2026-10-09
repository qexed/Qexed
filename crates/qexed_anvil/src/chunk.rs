//! 区块 NBT 结构操作：section 定位、方块/生物群系读写（基于名称引用，不依赖
//! 任何服务端方块注册表）。
//!
//! 区块根 NBT 形如：
//! ```text
//! { DataVersion, xPos, zPos, yPos, Status, sections: [ { Y, block_states: {palette, data}, biomes: {...} } ], ... }
//! ```
//! palette 条目是 `{ Name: "minecraft:stone", Properties: {...} }`（方块）或
//! 纯字符串（生物群系）。本模块以 `BlockStateRef` 表达 palette 条目，读写往返
//! 保持字段语义不变。

use std::collections::HashMap;
use std::sync::Arc;

use qexed_nbt::{ListHeader, Tag, tag_id};

use crate::error::AnvilError;
use crate::palette::{
    BIOME_ENTRY_COUNT, BLOCK_ENTRY_COUNT, PaletteKind, container, long_array, pack_values,
    storage_bits, unpack_indices, unpack_indices_spanning,
};

/// 世界最低 section（y=-64 对应 section -4；1.18+ 的 vanilla 布局）。
pub const MIN_SECTION_Y: i32 = -4;
/// 一个 section 的高度（16 格）。
pub const SECTION_HEIGHT: i32 = 16;

/// palette 里的方块状态引用：名称 + 可选属性。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockStateRef {
    pub name: String,
    pub properties: Vec<(String, String)>,
}

impl BlockStateRef {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            properties: Vec::new(),
        }
    }

    pub fn with_properties(mut self, properties: Vec<(String, String)>) -> Self {
        self.properties = properties;
        self
    }

    fn from_nbt(entry: &HashMap<String, Tag>) -> Self {
        let name = string_field(entry, "Name").unwrap_or("minecraft:air").to_string();
        let properties = entry
            .get("Properties")
            .and_then(|tag| match tag {
                Tag::Compound(map) => Some(map),
                _ => None,
            })
            .map(|map| {
                let mut props: Vec<(String, String)> = map
                    .iter()
                    .filter_map(|(key, value)| match value {
                        Tag::String(value) => Some((key.clone(), value.to_string())),
                        _ => None,
                    })
                    .collect();
                props.sort_by(|left, right| left.0.cmp(&right.0));
                props
            })
            .unwrap_or_default();
        Self { name, properties }
    }

    fn to_nbt(&self) -> Tag {
        let mut fields = HashMap::new();
        fields.insert("Name".to_string(), Tag::String(Arc::from(self.name.as_str())));
        if !self.properties.is_empty() {
            fields.insert(
                "Properties".to_string(),
                Tag::Compound(Arc::new(
                    self
                        .properties
                        .iter()
                        .cloned()
                        .map(|(name, value)| (name, Tag::String(Arc::from(value))))
                        .collect(),
                )),
            );
        }
        Tag::Compound(Arc::new(fields))
    }
}

/// 一个 section 解码后的方块状态序列（4096 项，按 y*256 + z*16 + x 排列）。
#[derive(Debug, Clone)]
pub struct SectionBlocks {
    pub section_y: i32,
    pub palette: Vec<BlockStateRef>,
    pub indices: Vec<usize>,
}

impl SectionBlocks {
    /// 取某个局部坐标的方块引用（越界返回 None）。
    pub fn at(&self, local_x: usize, local_y: usize, local_z: usize) -> Option<&BlockStateRef> {
        let index = (local_y * 16 + local_z) * 16 + local_x;
        self.palette.get(*self.indices.get(index)?)
    }
}

/// 一个 section 解码后的生物群系序列（64 项，4x4x4）。
#[derive(Debug, Clone)]
pub struct SectionBiomes {
    pub section_y: i32,
    pub palette: Vec<String>,
    pub indices: Vec<usize>,
}

/// 取区块根 compound（类型不匹配时报错）。
pub fn root_compound(root: &Tag) -> Result<&HashMap<String, Tag>, AnvilError> {
    match root {
        Tag::Compound(map) => Ok(map),
        _ => Err(AnvilError::NotACompound),
    }
}

/// sections 列表按 Y 值索引。
///
/// 同时支持两种布局：1.18+ 的根级 `sections`，与 1.13~1.17 的
/// `Level.Sections`（旧版区块根外层包一个 "Level" compound）。
pub fn sections_by_y<'a>(
    root: &'a HashMap<String, Tag>,
) -> HashMap<i32, &'a HashMap<String, Tag>> {
    let mut sections = HashMap::new();
    let list = root
        .get("sections")
        .or_else(|| legacy_level(root).and_then(|level| level.get("Sections")));
    let Some(Tag::List(_, items)) = list else {
        return sections;
    };

    for item in items.iter() {
        let Tag::Compound(section) = item else {
            continue;
        };
        let Some(y) = int_field(section, "Y") else {
            continue;
        };
        sections.insert(y, section.as_ref());
    }

    sections
}

/// 旧版（1.13~1.17）区块根的 `Level` 内层 compound。
fn legacy_level<'a>(root: &'a HashMap<String, Tag>) -> Option<&'a HashMap<String, Tag>> {
    match root.get("Level") {
        Some(Tag::Compound(level)) => Some(level),
        _ => None,
    }
}

/// 区块数据版本（`DataVersion`；根或 Level 里都可查）。
pub fn data_version(root: &HashMap<String, Tag>) -> Option<i32> {
    int_field(root, "DataVersion")
        .or_else(|| legacy_level(root).and_then(|level| int_field(level, "DataVersion")))
}

/// 世界坐标 -> section 内索引（`y*256 + z*16 + x`，局部量取模 16）。
pub fn block_index(x: i32, y: i32, z: i32) -> usize {
    let local_x = x.rem_euclid(16) as usize;
    let local_y = y.rem_euclid(16) as usize;
    let local_z = z.rem_euclid(16) as usize;
    (local_y * 16 + local_z) * 16 + local_x
}

/// 读取某个 section 的方块状态（palette + 展开的索引）。
///
/// 同时支持两种存储：1.18+ 的 `block_states: {palette, data}`
/// 容器，与 1.13~1.17 的 section 内 `Palette` + `BlockStates` 并列字段。
/// 位打包按现代（非跨界）约定；旧版区块用 [`section_blocks_spanning`]。
pub fn section_blocks(
    section: &HashMap<String, Tag>,
) -> Result<SectionBlocks, AnvilError> {
    section_blocks_spanning(section, false)
}

/// [`section_blocks`] 的旧版打包版本（1.13–1.15，DataVersion < 2529 的跨 long 位流）。
pub fn section_blocks_spanning(
    section: &HashMap<String, Tag>,
    spanning: bool,
) -> Result<SectionBlocks, AnvilError> {
    let section_y = int_field(section, "Y").unwrap_or_default();

    if let Some(block_states) = container(section.get("block_states")) {
        let palette = block_palette(block_states);
        let indices = unpack_indices_spanning(
            long_array(block_states.get("data")).map(|v| v as &[i64]),
            palette.len(),
            BLOCK_ENTRY_COUNT,
            PaletteKind::Block,
            spanning,
        )?;
        return Ok(SectionBlocks { section_y, palette, indices });
    }

    // 旧版：Palette / BlockStates 直接在 section 里
    // （1.12 及更早的 Blocks/Data 扁平 ID 存储不在支持范围，会被跳过）
    if section.contains_key("Palette") || section.contains_key("BlockStates") {
        let palette = legacy_block_palette(section);
        let indices = unpack_indices_spanning(
            long_array(section.get("BlockStates")).map(|v| v as &[i64]),
            palette.len(),
            BLOCK_ENTRY_COUNT,
            PaletteKind::Block,
            spanning,
        )?;
        return Ok(SectionBlocks { section_y, palette, indices });
    }

    Ok(SectionBlocks {
        section_y,
        palette: vec![BlockStateRef::new("minecraft:air")],
        indices: vec![0; BLOCK_ENTRY_COUNT],
    })
}

/// 读取某个 section 的生物群系。
pub fn section_biomes(section: &HashMap<String, Tag>) -> Result<SectionBiomes, AnvilError> {
    let section_y = int_field(section, "Y").unwrap_or_default();
    let Some(biomes) = container(section.get("biomes")) else {
        return Ok(SectionBiomes {
            section_y,
            palette: vec!["minecraft:plains".to_string()],
            indices: vec![0; BIOME_ENTRY_COUNT],
        });
    };

    let palette = biome_palette(biomes);
    let indices = unpack_indices(
        long_array(biomes.get("data")).map(|v| v as &[i64]),
        palette.len(),
        BIOME_ENTRY_COUNT,
        PaletteKind::Biome,
    )?;

    Ok(SectionBiomes {
        section_y,
        palette,
        indices,
    })
}

/// 读取区块全部 section 的方块状态。
///
/// 1.12 及更早的扁平 ID section（Blocks/Data）不在支持范围，直接跳过。
pub fn all_section_blocks(root: &Tag) -> Result<Vec<SectionBlocks>, AnvilError> {
    let root = root_compound(root)?;
    let spanning = is_spanning_chunk(root);
    let sections = sections_by_y(root);
    let mut result = Vec::with_capacity(sections.len());
    for (_, section) in sections {
        if section.contains_key("Blocks") || section.contains_key("Data") {
            continue; // pre-flattening storage, unsupported by design
        }
        result.push(section_blocks_spanning(section, spanning)?);
    }
    Ok(result)
}

/// DataVersion < 2529（1.15 及更早）使用跨 long 位流打包。
fn is_spanning_chunk(root: &HashMap<String, Tag>) -> bool {
    data_version(root).map(|v| v < 2529).unwrap_or(false)
}

/// 取某个世界坐标的方块引用。
pub fn block_state_at(root: &Tag, x: i32, y: i32, z: i32) -> Result<Option<BlockStateRef>, AnvilError> {
    let root = root_compound(root)?;
    let section_y = y.div_euclid(SECTION_HEIGHT);
    let spanning = is_spanning_chunk(root);
    let sections = sections_by_y(root);
    let Some(section) = sections.get(&section_y) else {
        return Ok(None);
    };
    let blocks = section_blocks_spanning(section, spanning)?;
    let index = block_index(x, y, z);
    let palette_index = *blocks
        .indices
        .get(index)
        .ok_or(AnvilError::IndexOutOfBounds { index })?;
    Ok(blocks.palette.get(palette_index).cloned())
}

/// 在区块根 NBT 里写入一个方块（局部于本区块；section 不存在时用 fallback 填充创建）。
///
/// 返回修改后的新根标签（原标签不变）。
pub fn set_block_state(
    root: &Tag,
    x: i32,
    y: i32,
    z: i32,
    state: &BlockStateRef,
    fallback_block: &BlockStateRef,
    fallback_biome: &str,
) -> Result<Tag, AnvilError> {
    let mut root = root_compound(root)?.clone();
    let legacy = root.contains_key("Level");

    let section_y = y.div_euclid(SECTION_HEIGHT);
    let mut sections = section_list_mut(&mut root, legacy)
        .into_iter()
        .collect::<Vec<_>>();
    let section_index = sections.iter().position(|section| match section {
        Tag::Compound(fields) => int_field(fields, "Y") == Some(section_y),
        _ => false,
    });

    let mut section = match section_index {
        Some(index) => match &sections[index] {
            Tag::Compound(fields) => fields.as_ref().clone(),
            _ => return Err(AnvilError::SectionNotCompound(section_y)),
        },
        None => {
            let mut section = empty_section(section_y, fallback_block, fallback_biome);
            if legacy {
                // 转成旧版平铺字段
                let modern = section.remove("block_states");
                section.remove("biomes");
                let _ = modern; // 空字段的方块存储由 write_section_storage 重写
            }
            section
        }
    };

    let mut blocks = section_blocks(&section)?;
    // 目标状态进 palette（或复用已有条目）
    let target_index = blocks
        .palette
        .iter()
        .position(|existing| existing == state)
        .unwrap_or_else(|| {
            blocks.palette.push(state.clone());
            blocks.palette.len() - 1
        });
    let index = block_index(x, y, z);
    blocks.indices[index] = target_index;

    write_section_storage(&mut section, &blocks, fallback_biome, legacy)?;

    let section_tag = Tag::Compound(Arc::new(section));
    if let Some(index) = section_index {
        sections[index] = section_tag;
    } else {
        sections.push(section_tag);
        sections.sort_by_key(|section| match section {
            Tag::Compound(fields) => int_field(fields, "Y").unwrap_or(i32::MAX),
            _ => i32::MAX,
        });
    }
    if legacy {
        let Some(Tag::Compound(level)) = root.get_mut("Level") else {
            return Err(AnvilError::NotACompound);
        };
        Arc::make_mut(level).insert(
            "Sections".to_string(),
            list_tag(tag_id::COMPOUND, sections),
        );
    } else {
        root.insert("sections".to_string(), list_tag(tag_id::COMPOUND, sections));
    }

    Ok(Tag::Compound(Arc::new(root)))
}

/// 取区块的 section 列表（新旧两种布局），返回可变的副本。
fn section_list_mut(root: &mut HashMap<String, Tag>, legacy: bool) -> Vec<Tag> {
    let list_key = if legacy {
        root.get("Level").and_then(|tag| match tag {
            Tag::Compound(level) => level.get("Sections").cloned(),
            _ => None,
        })
    } else {
        root.get("sections").cloned()
    };
    match list_key {
        Some(Tag::List(_, items)) => items.to_vec(),
        _ => Vec::new(),
    }
}

/// 按布局写回 section 的方块存储：新版 `block_states` 容器或旧版平铺字段。
fn write_section_storage(
    section: &mut HashMap<String, Tag>,
    blocks: &SectionBlocks,
    fallback_biome: &str,
    legacy: bool,
) -> Result<(), AnvilError> {
    if !legacy {
        section.insert(
            "block_states".to_string(),
            block_states_tag(&blocks.palette, &blocks.indices)?,
        );
        section
            .entry("biomes".to_string())
            .or_insert_with(|| biome_states_tag(&[fallback_biome.to_string()], None));
        return Ok(());
    }

    // 旧版：Palette 列表 + BlockStates 长整型数组平铺在 section 里
    let palette_tags = blocks
        .palette
        .iter()
        .map(BlockStateRef::to_nbt)
        .collect::<Vec<_>>();
    section.insert(
        "Palette".to_string(),
        list_tag(tag_id::COMPOUND, palette_tags),
    );
    let data = (blocks.palette.len() > 1)
        .then(|| {
            let bits = storage_bits(PaletteKind::Block, blocks.palette.len());
            pack_values(&blocks.indices, bits)
        })
        .transpose()?
        .map(|values| values.into_iter().map(|value| value as i64).collect::<Vec<_>>());
    if let Some(data) = data {
        section.insert("BlockStates".to_string(), Tag::LongArray(Arc::from(data)));
    } else {
        section.remove("BlockStates");
    }
    Ok(())
}

/// 构造空 section：Y + 全 fallback 方块 + 单生物群系。
pub fn empty_section(
    section_y: i32,
    fallback_block: &BlockStateRef,
    fallback_biome: &str,
) -> HashMap<String, Tag> {
    HashMap::from([
        ("Y".to_string(), Tag::Byte(section_y as i8)),
        (
            "block_states".to_string(),
            block_states_tag(
                &[fallback_block.clone()],
                &vec![0; BLOCK_ENTRY_COUNT],
            )
            .expect("single-entry palette is always valid"),
        ),
        (
            "biomes".to_string(),
            biome_states_tag(&[fallback_biome.to_string()], None),
        ),
    ])
}

/// 由 palette + 索引构造 block_states 标签（索引超调时重新打包位宽）。
pub fn block_states_tag(
    palette: &[BlockStateRef],
    indices: &[usize],
) -> Result<Tag, AnvilError> {
    if indices.len() != BLOCK_ENTRY_COUNT {
        return Err(AnvilError::BlockStateCount {
            got: indices.len(),
            expected: BLOCK_ENTRY_COUNT,
        });
    }

    let palette_tags = palette.iter().map(BlockStateRef::to_nbt).collect::<Vec<_>>();
    let data = (palette.len() > 1)
        .then(|| {
            let bits = storage_bits(PaletteKind::Block, palette.len());
            pack_values(indices, bits)
        })
        .transpose()?
        .map(|values| values.into_iter().map(|value| value as i64).collect::<Vec<_>>());

    Ok(paletted_container_tag(palette_tags, data))
}

/// 由生物群系 palette + 索引构造 biomes 标签。
pub fn biome_states_tag(palette: &[String], indices: Option<&[usize]>) -> Tag {
    let palette_tags = palette
        .iter()
        .map(|name| Tag::String(Arc::from(name.as_str())))
        .collect::<Vec<_>>();
    let data = indices
        .and_then(|indices| {
            (palette.len() > 1).then(|| {
                let bits = storage_bits(PaletteKind::Biome, palette.len());
                pack_values(indices, bits)
                    .expect("validated indices fit storage bits")
                    .into_iter()
                    .map(|value| value as i64)
                    .collect::<Vec<_>>()
            })
        });
    paletted_container_tag(palette_tags, data)
}

fn paletted_container_tag(palette: Vec<Tag>, data: Option<Vec<i64>>) -> Tag {
    let mut fields = HashMap::new();
    fields.insert(
        "palette".to_string(),
        Tag::List(
            ListHeader {
                tag_id: palette.first().map(Tag::tag_id).unwrap_or(tag_id::END),
                length: palette.len() as i32,
            },
            Arc::from(palette),
        ),
    );
    if let Some(data) = data {
        fields.insert("data".to_string(), Tag::LongArray(Arc::from(data)));
    }
    Tag::Compound(Arc::new(fields))
}

/// 旧版（1.13~1.17）section 的 `Palette` 字段。
fn legacy_block_palette(section: &HashMap<String, Tag>) -> Vec<BlockStateRef> {
    let Some(Tag::List(_, entries)) = section.get("Palette") else {
        return vec![BlockStateRef::new("minecraft:air")];
    };

    let palette = entries
        .iter()
        .filter_map(|entry| match entry {
            Tag::Compound(fields) => Some(BlockStateRef::from_nbt(fields)),
            _ => None,
        })
        .collect::<Vec<_>>();

    if palette.is_empty() {
        vec![BlockStateRef::new("minecraft:air")]
    } else {
        palette
    }
}

fn block_palette(container: &HashMap<String, Tag>) -> Vec<BlockStateRef> {
    let Some(Tag::List(_, entries)) = container.get("palette") else {
        return vec![BlockStateRef::new("minecraft:air")];
    };

    let palette = entries
        .iter()
        .filter_map(|entry| match entry {
            Tag::Compound(fields) => Some(BlockStateRef::from_nbt(fields)),
            _ => None,
        })
        .collect::<Vec<_>>();

    if palette.is_empty() {
        vec![BlockStateRef::new("minecraft:air")]
    } else {
        palette
    }
}

fn biome_palette(container: &HashMap<String, Tag>) -> Vec<String> {
    let Some(Tag::List(_, entries)) = container.get("palette") else {
        return vec!["minecraft:plains".to_string()];
    };

    let palette = entries
        .iter()
        .filter_map(|entry| match entry {
            Tag::String(name) => Some(name.to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();

    if palette.is_empty() {
        vec!["minecraft:plains".to_string()]
    } else {
        palette
    }
}

pub(crate) fn string_field<'a>(
    compound: &'a HashMap<String, Tag>,
    name: &str,
) -> Option<&'a str> {
    match compound.get(name) {
        Some(Tag::String(value)) => Some(value),
        _ => None,
    }
}

pub(crate) fn int_field(compound: &HashMap<String, Tag>, name: &str) -> Option<i32> {
    match compound.get(name) {
        Some(Tag::Byte(value)) => Some(i32::from(*value)),
        Some(Tag::Short(value)) => Some(i32::from(*value)),
        Some(Tag::Int(value)) => Some(*value),
        Some(Tag::Long(value)) => i32::try_from(*value).ok(),
        _ => None,
    }
}

pub(crate) fn list_tag(tag_id: u8, items: Vec<Tag>) -> Tag {
    Tag::List(
        ListHeader {
            tag_id,
            length: items.len() as i32,
        },
        Arc::from(items),
    )
}
