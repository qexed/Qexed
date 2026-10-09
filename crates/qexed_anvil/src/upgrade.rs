//! 自身实现的升级链：把更旧的存档升级到 1.13+（flattening）。
//!
//! 覆盖：
//! - 1.12 及更早的扁平 ID 区块（Blocks/Data 字节数组）-> 1.13+ 调色板布局
//! - 旧版实体（区块内 Entities）拆分说明见 [`upgrade_world_entities`]

use std::collections::HashMap;
use std::sync::Arc;

use qexed_nbt::{Tag, tag_id};

use crate::chunk::{
    self, BlockStateRef, block_states_tag, int_field, list_tag,
};
use crate::error::AnvilError;

/// 1.12 扁平 ID（id<<4 | meta）-> 现代方块状态。
///
/// 覆盖 vanilla 常用方块；未收录的组合回退为 air（计数见返回的 `unknown`）。
pub fn flatten_state(id: u8, meta: u8) -> Option<BlockStateRef> {
    use BlockStateRef as B;
    let b = |name: &str| B::new(name);
    let bp = |name: &str, props: &[(&str, &str)]| {
        B::new(name).with_properties(props.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
    };
    let key = (u16::from(id) << 4) | u16::from(meta);
    Some(match (u16::from(id), u16::from(meta)) {
        (0, _) => b("minecraft:air"),
        (1, 0) => b("minecraft:stone"),
        (1, 1) => b("minecraft:granite"),
        (1, 3) => b("minecraft:diorite"),
        (1, 5) => b("minecraft:andesite"),
        (2, _) => b("minecraft:grass_block"),
        (3, 0) => b("minecraft:dirt"),
        (3, 1) => b("minecraft:coarse_dirt"),
        (3, 2) => b("minecraft:podzol"),
        (4, _) => b("minecraft:cobblestone"),
        (5, m) => { // wood planks
            let kind = match m { 1 => "spruce", 2 => "birch", 3 => "jungle", 4 => "acacia", 5 => "dark_oak", _ => "oak" };
            B::new(format!("minecraft:{kind}_planks").leak())
        }
        (7, _) => b("minecraft:bedrock"),
        (8, _) | (9, _) => b("minecraft:water"),
        (10, _) | (11, _) => b("minecraft:lava"),
        (12, _) => b("minecraft:sand"),
        (13, _) => b("minecraft:gravel"),
        (14, 0) => b("minecraft:gold_ore"),
        (15, 0) => b("minecraft:iron_ore"),
        (16, 0) => b("minecraft:coal_ore"),
        (17, m) => {
            let kind = match m & 0b11 { 1 => "spruce", 2 => "birch", 3 => "jungle", _ => "oak" };
            let axis = match (m >> 2) & 0b11 { 1 => "x", 2 => "z", 3 => "y", _ => "y" };
            bp(&format!("minecraft:{kind}_log"), &[("axis", axis)])
        }
        (18, m) => {
            let kind = match m & 0b11 { 1 => "spruce", 2 => "birch", 3 => "jungle", _ => "oak" };
            B::new(format!("minecraft:{kind}_leaves").leak())
        }
        (19, _) => b("minecraft:sponge"),
        (20, _) => b("minecraft:glass"),
        (21, _) => b("minecraft:lapis_ore"),
        (22, _) => b("minecraft:lapis_block"),
        (24, m) => if m & 1 == 1 { b("minecraft:chiseled_sandstone") } else { b("minecraft:sandstone") },
        (31, 1) => b("minecraft:grass"),
        (31, 2) => b("minecraft:fern"),
        (35, m) => {
            let color = ["white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray",
                "light_gray", "cyan", "purple", "blue", "brown", "green", "red", "black"][m as usize % 16];
            B::new(format!("minecraft:{color}_wool").leak())
        }
        (37, _) => b("minecraft:dandelion"),
        (38, m) => B::new(["minecraft:poppy", "minecraft:blue_orchid", "minecraft:allium",
            "minecraft:azure_bluet", "minecraft:red_tulip", "minecraft:orange_tulip", "minecraft:white_tulip",
            "minecraft:pink_tulip", "minecraft:oxeye_daisy"][m as usize % 9]),
        (41, _) => b("minecraft:gold_block"),
        (42, _) => b("minecraft:iron_block"),
        (45, _) => b("minecraft:bricks"),
        (46, _) => b("minecraft:tnt"),
        (47, _) => b("minecraft:bookshelf"),
        (48, _) => b("minecraft:mossy_cobblestone"),
        (50, m) => {
            let facing = match m & 0b111 { 1 => "west", 2 => "east", 3 => "north", 4 => "south", 5 => "up", _ => "down" };
            match facing {
                "west" | "east" | "north" | "south" => bp("minecraft:wall_torch", &[("facing", facing)]),
                _ => b("minecraft:torch"),
            }
        }
        (52, _) => b("minecraft:spawner"),
        (54, m) => {
            let facing = match m & 0b111 { 2 => "north", 3 => "south", 4 => "west", 5 => "east", _ => "up" };
            bp("minecraft:chest", &[("facing", facing), ("type", if m & 0x8 != 0 { "right" } else { "single" })])
        }
        (56, _) => b("minecraft:diamond_ore"),
        (57, _) => b("minecraft:diamond_block"),
        (58, _) => b("minecraft:crafting_table"),
        (60, _) => b("minecraft:farmland"),
        (61, _) | (62, _) => b("minecraft:furnace"),
        (73, _) => b("minecraft:redstone_ore"),
        (74, _) => b("minecraft:redstone_ore"),
        (79, _) => b("minecraft:ice"),
        (80, _) => b("minecraft:snow_block"),
        (78, m) => bp("minecraft:snow", &[("layers", &(m % 8).to_string())]),
        (82, _) => b("minecraft:clay"),
        (86, 0) => b("minecraft:carved_pumpkin"),
        (91, 0) => b("minecraft:jack_o_lantern"),
        (98, m) => match m { 1 => b("minecraft:mossy_stone_bricks"), 2 => b("minecraft:cracked_stone_bricks"), 3 => b("minecraft:chiseled_stone_bricks"), _ => b("minecraft:stone_bricks") },
        (102, _) => b("minecraft:glass_pane"),
        (103, _) => b("minecraft:melon"),
        (110, _) => b("minecraft:mycelium"),
        (112, _) => b("minecraft:nether_bricks"),
        (121, _) => b("minecraft:end_stone"),
        (123, _) => b("minecraft:redstone_torch"),
        (124, _) => b("minecraft:redstone_torch"),
        (129, _) => b("minecraft:emerald_ore"),
        (133, _) => b("minecraft:emerald_block"),
        (152, _) => b("minecraft:redstone_block"),
        (153, _) => b("minecraft:quartz_block"),
        (159, m) => {
            let color = ["white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray",
                "light_gray", "cyan", "purple", "blue", "brown", "green", "red", "black"][m as usize % 16];
            B::new(format!("minecraft:{color}_terracotta").leak())
        }
        (165, _) => b("minecraft:slime_block"),
        (168, m) => match m { 1 => b("minecraft:polished_prismarine"), 2 => b("minecraft:prismarine_bricks"), 3 => b("minecraft:dark_prismarine"), _ => b("minecraft:prismarine") },
        (170, m) => if m == 0 { b("minecraft:hay_block") } else { bp("minecraft:hay_block", &[("axis", if m == 8 { "x" } else { "z" })]) },
        (179, m) => {
            let kind = match m { 1 => "red_sandstone".to_string(), _ => "red_sandstone".to_string() };
            let _ = kind;
            b("minecraft:red_sandstone")
        }
        (201, _) => b("minecraft:purpur_block"),
        (206, _) => b("minecraft:end_stone_bricks"),
        _ => {
            let _ = key;
            return None;
        }
    })
}

/// 升级 1.12（及更早）区块到 1.13+ 布局（flattening）。
///
/// 返回 (新区块, 统计)。未收录的方块 ID 映射为 air 并计入 `unknown`。
pub fn upgrade_chunk_1_12(
    root: &Tag,
    target_data_version: i32,
) -> Result<(Tag, PreFlatteningStats), AnvilError> {
    let fields = chunk::root_compound(root)?;
    let Some(Tag::Compound(level)) = fields.get("Level") else {
        return Err(AnvilError::NotACompound);
    };
    let level = level.as_ref();

    let mut stats = PreFlatteningStats::default();
    let mut out: HashMap<String, Tag> = HashMap::new();
    out.insert("DataVersion".to_string(), Tag::Int(target_data_version));

    // Level 字段提升（重命名为现代名）
    for (key, value) in level {
        match key.as_str() {
            "Sections" | "Biomes" | "Entities" => {}
            "TileEntities" => { out.insert("block_entities".to_string(), value.clone()); }
            "TileTicks" => { out.insert("block_ticks".to_string(), value.clone()); }
            "LiquidTicks" => { out.insert("fluid_ticks".to_string(), value.clone()); }
            "Status" => {
                out.insert("Status".to_string(), Tag::String(Arc::from("minecraft:full")));
            }
            _ => { out.insert(key.clone(), value.clone()); }
        }
    }
    out.entry("xPos".to_string()).or_insert(Tag::Int(0));
    out.entry("zPos".to_string()).or_insert(Tag::Int(0));
    out.entry("yPos".to_string()).or_insert(Tag::Int(0));

    // sections：扁平存储 -> 调色板
    let mut modern_sections = Vec::new();
    if let Some(Tag::List(_, items)) = level.get("Sections") {
        for item in items.iter() {
            let Tag::Compound(section) = item else { continue };
            let Some(section_y) = int_field(section, "Y") else { continue };

            let (Some(Tag::ByteArray(blocks)), Some(Tag::ByteArray(data))) =
                (section.get("Blocks"), section.get("Data")) else {
                continue; // 空光照 section
            };

            let mut palette: Vec<BlockStateRef> = Vec::new();
            let mut indices: Vec<usize> = Vec::with_capacity(4096);
            let mut index_of: HashMap<BlockStateRef, usize> = HashMap::new();

            for i in 0..4096usize {
                let id = blocks[i] as u8;
                let meta = if i % 2 == 0 {
                    (data[i / 2] as u8) & 0x0F
                } else {
                    ((data[i / 2] as u8) >> 4) & 0x0F
                };

                let state = match flatten_state(id, meta) {
                    Some(state) => state,
                    None => {
                        stats.unknown_block_ids.insert(u16::from(id) << 4 | u16::from(meta));
                        BlockStateRef::new("minecraft:air")
                    }
                };
                let index = match index_of.get(&state) {
                    Some(&existing) => existing,
                    None => {
                        palette.push(state.clone());
                        let index = palette.len() - 1;
                        index_of.insert(state, index);
                        index
                    }
                };
                indices.push(index);
            }

            if palette.is_empty() { continue }
            let mut modern = HashMap::new();
            modern.insert("Y".to_string(), Tag::Byte(section_y as i8));
            modern.insert("block_states".to_string(), block_states_tag(&palette, &indices)?);
            modern_sections.push(Tag::Compound(Arc::new(modern)));
            stats.sections_upgraded += 1;
        }
    }
    out.insert("sections".to_string(), list_tag(tag_id::COMPOUND, modern_sections));

    Ok((Tag::Compound(Arc::new(out)), stats))
}

/// 1.12 十级统计。
#[derive(Debug, Clone, Default)]
pub struct PreFlatteningStats {
    pub sections_upgraded: usize,
    /// 未能映射的方块状态（id<<4|meta 集合）。
    pub unknown_block_ids: std::collections::BTreeSet<u16>,
}
