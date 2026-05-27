use anyhow::{Context, Result};
use qexed_packet::net_types::Position;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StructureBlock {
    pub dx: i32,
    pub dy: i32,
    pub dz: i32,
    pub block: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StructureTemplate {
    pub id: &'static str,
    pub blocks: &'static [StructureBlock],
}

impl StructureTemplate {
    pub(crate) fn instantiate(&self, origin: Position) -> Result<Vec<(Position, i32)>> {
        self.blocks
            .iter()
            .map(|block| {
                Ok((
                    Position {
                        x: origin.x + block.dx,
                        y: origin.y + block.dy,
                        z: origin.z + block.dz,
                    },
                    crate::world::chunk_nbt::default_block_state_id(block.block),
                ))
            })
            .collect()
    }
}

pub(crate) fn list_templates() -> &'static [StructureTemplate] {
    STRUCTURES
}

pub(crate) fn get_template(id: &str) -> Option<&'static StructureTemplate> {
    let id = normalize_template_id(id);
    STRUCTURES.iter().find(|template| template.id == id)
}

pub(crate) fn instantiate(id: &str, origin: Position) -> Result<Vec<(Position, i32)>> {
    let template = get_template(id).with_context(|| format!("unknown structure: {id}"))?;
    template.instantiate(origin)
}

fn normalize_template_id(id: &str) -> String {
    let id = id.trim();
    if id.contains(':') {
        id.to_string()
    } else {
        format!("qexed:{id}")
    }
}

const STRUCTURES: &[StructureTemplate] = &[
    StructureTemplate {
        id: "qexed:stone_platform",
        blocks: STONE_PLATFORM,
    },
    StructureTemplate {
        id: "qexed:desert_well",
        blocks: DESERT_WELL,
    },
    StructureTemplate {
        id: "qexed:obsidian_pillar",
        blocks: OBSIDIAN_PILLAR,
    },
];

const STONE_PLATFORM: &[StructureBlock] = &[
    StructureBlock {
        dx: -2,
        dy: 0,
        dz: -2,
        block: "minecraft:stone_bricks",
    },
    StructureBlock {
        dx: -1,
        dy: 0,
        dz: -2,
        block: "minecraft:stone_bricks",
    },
    StructureBlock {
        dx: 0,
        dy: 0,
        dz: -2,
        block: "minecraft:stone_bricks",
    },
    StructureBlock {
        dx: 1,
        dy: 0,
        dz: -2,
        block: "minecraft:stone_bricks",
    },
    StructureBlock {
        dx: 2,
        dy: 0,
        dz: -2,
        block: "minecraft:stone_bricks",
    },
    StructureBlock {
        dx: -2,
        dy: 0,
        dz: -1,
        block: "minecraft:stone_bricks",
    },
    StructureBlock {
        dx: -1,
        dy: 0,
        dz: -1,
        block: "minecraft:stone_bricks",
    },
    StructureBlock {
        dx: 0,
        dy: 0,
        dz: -1,
        block: "minecraft:stone_bricks",
    },
    StructureBlock {
        dx: 1,
        dy: 0,
        dz: -1,
        block: "minecraft:stone_bricks",
    },
    StructureBlock {
        dx: 2,
        dy: 0,
        dz: -1,
        block: "minecraft:stone_bricks",
    },
    StructureBlock {
        dx: -2,
        dy: 0,
        dz: 0,
        block: "minecraft:stone_bricks",
    },
    StructureBlock {
        dx: -1,
        dy: 0,
        dz: 0,
        block: "minecraft:stone_bricks",
    },
    StructureBlock {
        dx: 0,
        dy: 0,
        dz: 0,
        block: "minecraft:chiseled_stone_bricks",
    },
    StructureBlock {
        dx: 1,
        dy: 0,
        dz: 0,
        block: "minecraft:stone_bricks",
    },
    StructureBlock {
        dx: 2,
        dy: 0,
        dz: 0,
        block: "minecraft:stone_bricks",
    },
    StructureBlock {
        dx: -2,
        dy: 0,
        dz: 1,
        block: "minecraft:stone_bricks",
    },
    StructureBlock {
        dx: -1,
        dy: 0,
        dz: 1,
        block: "minecraft:stone_bricks",
    },
    StructureBlock {
        dx: 0,
        dy: 0,
        dz: 1,
        block: "minecraft:stone_bricks",
    },
    StructureBlock {
        dx: 1,
        dy: 0,
        dz: 1,
        block: "minecraft:stone_bricks",
    },
    StructureBlock {
        dx: 2,
        dy: 0,
        dz: 1,
        block: "minecraft:stone_bricks",
    },
    StructureBlock {
        dx: -2,
        dy: 0,
        dz: 2,
        block: "minecraft:stone_bricks",
    },
    StructureBlock {
        dx: -1,
        dy: 0,
        dz: 2,
        block: "minecraft:stone_bricks",
    },
    StructureBlock {
        dx: 0,
        dy: 0,
        dz: 2,
        block: "minecraft:stone_bricks",
    },
    StructureBlock {
        dx: 1,
        dy: 0,
        dz: 2,
        block: "minecraft:stone_bricks",
    },
    StructureBlock {
        dx: 2,
        dy: 0,
        dz: 2,
        block: "minecraft:stone_bricks",
    },
];

const DESERT_WELL: &[StructureBlock] = &[
    StructureBlock {
        dx: -2,
        dy: 0,
        dz: -2,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: -1,
        dy: 0,
        dz: -2,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: 0,
        dy: 0,
        dz: -2,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: 1,
        dy: 0,
        dz: -2,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: 2,
        dy: 0,
        dz: -2,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: -2,
        dy: 0,
        dz: -1,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: -1,
        dy: 0,
        dz: -1,
        block: "minecraft:water",
    },
    StructureBlock {
        dx: 0,
        dy: 0,
        dz: -1,
        block: "minecraft:water",
    },
    StructureBlock {
        dx: 1,
        dy: 0,
        dz: -1,
        block: "minecraft:water",
    },
    StructureBlock {
        dx: 2,
        dy: 0,
        dz: -1,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: -2,
        dy: 0,
        dz: 0,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: -1,
        dy: 0,
        dz: 0,
        block: "minecraft:water",
    },
    StructureBlock {
        dx: 0,
        dy: 0,
        dz: 0,
        block: "minecraft:water",
    },
    StructureBlock {
        dx: 1,
        dy: 0,
        dz: 0,
        block: "minecraft:water",
    },
    StructureBlock {
        dx: 2,
        dy: 0,
        dz: 0,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: -2,
        dy: 0,
        dz: 1,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: -1,
        dy: 0,
        dz: 1,
        block: "minecraft:water",
    },
    StructureBlock {
        dx: 0,
        dy: 0,
        dz: 1,
        block: "minecraft:water",
    },
    StructureBlock {
        dx: 1,
        dy: 0,
        dz: 1,
        block: "minecraft:water",
    },
    StructureBlock {
        dx: 2,
        dy: 0,
        dz: 1,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: -2,
        dy: 0,
        dz: 2,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: -1,
        dy: 0,
        dz: 2,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: 0,
        dy: 0,
        dz: 2,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: 1,
        dy: 0,
        dz: 2,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: 2,
        dy: 0,
        dz: 2,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: -1,
        dy: 1,
        dz: -1,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: 1,
        dy: 1,
        dz: -1,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: -1,
        dy: 1,
        dz: 1,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: 1,
        dy: 1,
        dz: 1,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: -1,
        dy: 2,
        dz: -1,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: 1,
        dy: 2,
        dz: -1,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: -1,
        dy: 2,
        dz: 1,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: 1,
        dy: 2,
        dz: 1,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: -2,
        dy: 3,
        dz: -2,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: -1,
        dy: 3,
        dz: -2,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: 0,
        dy: 3,
        dz: -2,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: 1,
        dy: 3,
        dz: -2,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: 2,
        dy: 3,
        dz: -2,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: -2,
        dy: 3,
        dz: -1,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: -1,
        dy: 3,
        dz: -1,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: 0,
        dy: 3,
        dz: -1,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: 1,
        dy: 3,
        dz: -1,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: 2,
        dy: 3,
        dz: -1,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: -2,
        dy: 3,
        dz: 0,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: -1,
        dy: 3,
        dz: 0,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: 0,
        dy: 3,
        dz: 0,
        block: "minecraft:sandstone_slab",
    },
    StructureBlock {
        dx: 1,
        dy: 3,
        dz: 0,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: 2,
        dy: 3,
        dz: 0,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: -2,
        dy: 3,
        dz: 1,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: -1,
        dy: 3,
        dz: 1,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: 0,
        dy: 3,
        dz: 1,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: 1,
        dy: 3,
        dz: 1,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: 2,
        dy: 3,
        dz: 1,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: -2,
        dy: 3,
        dz: 2,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: -1,
        dy: 3,
        dz: 2,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: 0,
        dy: 3,
        dz: 2,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: 1,
        dy: 3,
        dz: 2,
        block: "minecraft:sandstone",
    },
    StructureBlock {
        dx: 2,
        dy: 3,
        dz: 2,
        block: "minecraft:sandstone",
    },
];

const OBSIDIAN_PILLAR: &[StructureBlock] = &[
    StructureBlock {
        dx: -1,
        dy: 0,
        dz: -1,
        block: "minecraft:obsidian",
    },
    StructureBlock {
        dx: 0,
        dy: 0,
        dz: -1,
        block: "minecraft:obsidian",
    },
    StructureBlock {
        dx: 1,
        dy: 0,
        dz: -1,
        block: "minecraft:obsidian",
    },
    StructureBlock {
        dx: -1,
        dy: 0,
        dz: 0,
        block: "minecraft:obsidian",
    },
    StructureBlock {
        dx: 0,
        dy: 0,
        dz: 0,
        block: "minecraft:obsidian",
    },
    StructureBlock {
        dx: 1,
        dy: 0,
        dz: 0,
        block: "minecraft:obsidian",
    },
    StructureBlock {
        dx: -1,
        dy: 0,
        dz: 1,
        block: "minecraft:obsidian",
    },
    StructureBlock {
        dx: 0,
        dy: 0,
        dz: 1,
        block: "minecraft:obsidian",
    },
    StructureBlock {
        dx: 1,
        dy: 0,
        dz: 1,
        block: "minecraft:obsidian",
    },
    StructureBlock {
        dx: 0,
        dy: 1,
        dz: 0,
        block: "minecraft:obsidian",
    },
    StructureBlock {
        dx: 0,
        dy: 2,
        dz: 0,
        block: "minecraft:obsidian",
    },
    StructureBlock {
        dx: 0,
        dy: 3,
        dz: 0,
        block: "minecraft:obsidian",
    },
    StructureBlock {
        dx: 0,
        dy: 4,
        dz: 0,
        block: "minecraft:obsidian",
    },
    StructureBlock {
        dx: 0,
        dy: 5,
        dz: 0,
        block: "minecraft:obsidian",
    },
    StructureBlock {
        dx: 0,
        dy: 6,
        dz: 0,
        block: "minecraft:obsidian",
    },
    StructureBlock {
        dx: -1,
        dy: 7,
        dz: -1,
        block: "minecraft:iron_bars",
    },
    StructureBlock {
        dx: 0,
        dy: 7,
        dz: -1,
        block: "minecraft:iron_bars",
    },
    StructureBlock {
        dx: 1,
        dy: 7,
        dz: -1,
        block: "minecraft:iron_bars",
    },
    StructureBlock {
        dx: -1,
        dy: 7,
        dz: 0,
        block: "minecraft:iron_bars",
    },
    StructureBlock {
        dx: 0,
        dy: 7,
        dz: 0,
        block: "minecraft:bedrock",
    },
    StructureBlock {
        dx: 1,
        dy: 7,
        dz: 0,
        block: "minecraft:iron_bars",
    },
    StructureBlock {
        dx: -1,
        dy: 7,
        dz: 1,
        block: "minecraft:iron_bars",
    },
    StructureBlock {
        dx: 0,
        dy: 7,
        dz: 1,
        block: "minecraft:iron_bars",
    },
    StructureBlock {
        dx: 1,
        dy: 7,
        dz: 1,
        block: "minecraft:iron_bars",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structure_lookup_accepts_short_ids() {
        assert_eq!(
            get_template("stone_platform").unwrap().id,
            "qexed:stone_platform"
        );
    }

    #[test]
    fn structure_instantiation_offsets_blocks() {
        let blocks = instantiate(
            "qexed:obsidian_pillar",
            Position {
                x: 10,
                y: 64,
                z: -3,
            },
        )
        .unwrap();
        assert!(
            blocks
                .iter()
                .any(|(position, _)| position.x == 10 && position.y == 71 && position.z == -3)
        );
    }
}
