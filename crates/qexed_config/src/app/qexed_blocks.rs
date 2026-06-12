use serde::{Deserialize, Serialize};

use crate::app::qexed::server::{Gameplay, GameplayBlockMechanics};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QexedBlocksConfig {
    #[serde(default = "default_block_updates")]
    pub block_updates: bool,

    #[serde(default, alias = "block_mechanics")]
    pub mechanics: GameplayBlockMechanics,

    #[serde(default = "default_crafting_table")]
    pub crafting_table: bool,

    #[serde(default = "default_furnace")]
    pub furnace: bool,

    #[serde(default = "default_furnace_blocks")]
    pub furnace_blocks: Vec<String>,

    #[serde(default = "default_cauldron")]
    pub cauldron: bool,

    #[serde(default = "default_cauldron_blocks")]
    pub cauldron_blocks: Vec<String>,

    #[serde(default = "default_redstone")]
    pub redstone: bool,

    #[serde(default = "default_furnace_tick_ms")]
    pub furnace_tick_ms: u64,

    #[serde(default = "default_redstone_tick_ms")]
    pub redstone_tick_ms: u64,

    #[serde(default = "default_redstone_max_distance")]
    pub redstone_max_distance: u32,

    #[serde(default = "default_farmland_tick_ms")]
    pub farmland_tick_ms: u64,
}

impl Default for QexedBlocksConfig {
    fn default() -> Self {
        let gameplay = Gameplay::default();
        Self {
            block_updates: gameplay.block_updates,
            mechanics: gameplay.block_mechanics,
            crafting_table: gameplay.crafting_table,
            furnace: gameplay.furnace,
            furnace_blocks: gameplay.furnace_blocks,
            cauldron: gameplay.cauldron,
            cauldron_blocks: gameplay.cauldron_blocks,
            redstone: gameplay.redstone,
            furnace_tick_ms: gameplay.furnace_tick_ms,
            redstone_tick_ms: gameplay.redstone_tick_ms,
            redstone_max_distance: gameplay.redstone_max_distance,
            farmland_tick_ms: gameplay.farmland_tick_ms,
        }
    }
}

impl QexedBlocksConfig {
    pub fn apply_to(self, gameplay: &mut Gameplay) {
        gameplay.block_updates = self.block_updates;
        gameplay.block_mechanics = self.mechanics;
        gameplay.crafting_table = self.crafting_table;
        gameplay.furnace = self.furnace;
        gameplay.furnace_blocks = self.furnace_blocks;
        gameplay.cauldron = self.cauldron;
        gameplay.cauldron_blocks = self.cauldron_blocks;
        gameplay.redstone = self.redstone;
        gameplay.furnace_tick_ms = self.furnace_tick_ms;
        gameplay.redstone_tick_ms = self.redstone_tick_ms;
        gameplay.redstone_max_distance = self.redstone_max_distance;
        gameplay.farmland_tick_ms = self.farmland_tick_ms;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QexedBlocks {
    #[serde(default)]
    pub blocks: QexedBlocksConfig,
}

impl Default for QexedBlocks {
    fn default() -> Self {
        Self {
            blocks: QexedBlocksConfig::default(),
        }
    }
}

impl qexed_config::tool::AppConfigTrait for QexedBlocks {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed_blocks";
}

fn default_block_updates() -> bool {
    Gameplay::default().block_updates
}

fn default_crafting_table() -> bool {
    Gameplay::default().crafting_table
}

fn default_furnace() -> bool {
    Gameplay::default().furnace
}

fn default_furnace_blocks() -> Vec<String> {
    Gameplay::default().furnace_blocks
}

fn default_cauldron() -> bool {
    Gameplay::default().cauldron
}

fn default_cauldron_blocks() -> Vec<String> {
    Gameplay::default().cauldron_blocks
}

fn default_redstone() -> bool {
    Gameplay::default().redstone
}

fn default_furnace_tick_ms() -> u64 {
    Gameplay::default().furnace_tick_ms
}

fn default_redstone_tick_ms() -> u64 {
    Gameplay::default().redstone_tick_ms
}

fn default_redstone_max_distance() -> u32 {
    Gameplay::default().redstone_max_distance
}

fn default_farmland_tick_ms() -> u64 {
    Gameplay::default().farmland_tick_ms
}

#[cfg(test)]
mod tests {
    use super::{QexedBlocks, QexedBlocksConfig};
    use crate::app::qexed::server::Gameplay;

    #[test]
    fn blocks_config_applies_to_gameplay_block_settings() {
        let config: QexedBlocks = toml::from_str(
            r#"
[blocks]
block_updates = false
furnace = false
furnace_blocks = ["minecraft:furnace", "minecraft:blast_furnace"]
cauldron = false
cauldron_blocks = ["minecraft:cauldron"]
redstone = false
furnace_tick_ms = 25
redstone_tick_ms = 10
redstone_max_distance = 8
farmland_tick_ms = 250

[blocks.mechanics]
default_enabled = true
disabled_blocks = ["minecraft:tnt"]

[[blocks.mechanics.rules]]
block = "minecraft:farmland"
enabled = false
"#,
        )
        .unwrap();
        let mut gameplay = Gameplay::default();

        config.blocks.apply_to(&mut gameplay);

        assert!(!gameplay.block_updates);
        assert!(!gameplay.furnace);
        assert_eq!(
            gameplay.furnace_blocks,
            vec![
                "minecraft:furnace".to_string(),
                "minecraft:blast_furnace".to_string()
            ]
        );
        assert!(!gameplay.cauldron);
        assert_eq!(gameplay.cauldron_blocks, vec!["minecraft:cauldron"]);
        assert!(!gameplay.redstone);
        assert_eq!(gameplay.furnace_tick_ms, 25);
        assert_eq!(gameplay.redstone_tick_ms, 10);
        assert_eq!(gameplay.redstone_max_distance, 8);
        assert_eq!(gameplay.farmland_tick_ms, 250);
        assert!(!gameplay.block_mechanics.block_enabled("minecraft:tnt"));
        assert!(!gameplay.block_mechanics.block_enabled("minecraft:farmland"));
    }

    #[test]
    fn default_blocks_config_tracks_gameplay_defaults() {
        let config = QexedBlocksConfig::default();
        let gameplay = Gameplay::default();

        assert_eq!(config.block_updates, gameplay.block_updates);
        assert_eq!(config.mechanics, gameplay.block_mechanics);
        assert_eq!(config.crafting_table, gameplay.crafting_table);
        assert_eq!(config.furnace, gameplay.furnace);
        assert_eq!(config.furnace_blocks, gameplay.furnace_blocks);
        assert_eq!(config.cauldron, gameplay.cauldron);
        assert_eq!(config.cauldron_blocks, gameplay.cauldron_blocks);
        assert_eq!(config.redstone, gameplay.redstone);
        assert_eq!(config.furnace_tick_ms, gameplay.furnace_tick_ms);
        assert_eq!(config.redstone_tick_ms, gameplay.redstone_tick_ms);
        assert_eq!(config.redstone_max_distance, gameplay.redstone_max_distance);
        assert_eq!(config.farmland_tick_ms, gameplay.farmland_tick_ms);
    }
}
