use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

use anyhow::{Context as _, Result};
use serde::Deserialize;

use crate::{
    cache::WorldgenCache,
    util::{default_air_name, normalize_identifier, read_json},
    vanilla_noise,
};
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct BlockStateJson {
    #[serde(default = "default_air_name", rename = "Name")]
    pub(crate) name: String,
    #[serde(default, rename = "Properties")]
    pub(crate) properties: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub(crate) struct BlockIds {
    pub(crate) air: i32,
    pub(crate) cave_air: i32,
    pub(crate) void_air: i32,
    pub(crate) water: i32,
    pub(crate) lava: i32,
}

#[derive(Debug, Clone)]
pub(crate) struct SurfaceBlockIds {
    pub(crate) bedrock: i32,
    pub(crate) stone: i32,
    pub(crate) deepslate: i32,
    pub(crate) dirt: i32,
    pub(crate) grass_block: i32,
    pub(crate) podzol: i32,
    pub(crate) coarse_dirt: i32,
    pub(crate) mycelium: i32,
    pub(crate) calcite: i32,
    pub(crate) gravel: i32,
    pub(crate) sand: i32,
    pub(crate) sandstone: i32,
    pub(crate) packed_ice: i32,
    pub(crate) ice: i32,
    pub(crate) snow_block: i32,
    pub(crate) powder_snow: i32,
    pub(crate) mud: i32,
    pub(crate) water: i32,
    pub(crate) terracotta: i32,
    pub(crate) orange_terracotta: i32,
    pub(crate) white_terracotta: i32,
    pub(crate) yellow_terracotta: i32,
    pub(crate) brown_terracotta: i32,
    pub(crate) red_terracotta: i32,
    pub(crate) light_gray_terracotta: i32,
    pub(crate) red_sand: i32,
    pub(crate) lava: i32,
    pub(crate) copper_ore: i32,
    pub(crate) raw_copper_block: i32,
    pub(crate) granite: i32,
    pub(crate) deepslate_iron_ore: i32,
    pub(crate) raw_iron_block: i32,
    pub(crate) tuff: i32,
}

impl BlockIds {
    pub(crate) fn from_cache() -> Result<Self> {
        Ok(Self {
            air: qexed_registry_block_id("minecraft:air")?,
            cave_air: qexed_registry_block_id("minecraft:cave_air")?,
            void_air: qexed_registry_block_id("minecraft:void_air")?,
            water: qexed_registry_block_id("minecraft:water")?,
            lava: qexed_registry_block_id("minecraft:lava")?,
        })
    }

    pub(crate) fn is_air(&self, block: i32) -> bool {
        block == self.air || block == self.cave_air || block == self.void_air
    }

    pub(crate) fn is_ocean_floor_block(&self, block: i32) -> bool {
        !self.is_air(block) && block != self.water && block != self.lava
    }
}

impl SurfaceBlockIds {
    pub(crate) fn from_cache() -> Result<Self> {
        Ok(Self {
            bedrock: qexed_registry_block_id("minecraft:bedrock")?,
            stone: qexed_registry_block_id("minecraft:stone")?,
            deepslate: qexed_registry_block_id("minecraft:deepslate")?,
            dirt: qexed_registry_block_id("minecraft:dirt")?,
            grass_block: qexed_registry_block_id("minecraft:grass_block")?,
            podzol: qexed_registry_block_id("minecraft:podzol")?,
            coarse_dirt: qexed_registry_block_id("minecraft:coarse_dirt")?,
            mycelium: qexed_registry_block_id("minecraft:mycelium")?,
            calcite: qexed_registry_block_id("minecraft:calcite")?,
            gravel: qexed_registry_block_id("minecraft:gravel")?,
            sand: qexed_registry_block_id("minecraft:sand")?,
            sandstone: qexed_registry_block_id("minecraft:sandstone")?,
            packed_ice: qexed_registry_block_id("minecraft:packed_ice")?,
            ice: qexed_registry_block_id("minecraft:ice")?,
            snow_block: qexed_registry_block_id("minecraft:snow_block")?,
            powder_snow: qexed_registry_block_id("minecraft:powder_snow")?,
            mud: qexed_registry_block_id("minecraft:mud")?,
            water: qexed_registry_block_id("minecraft:water")?,
            terracotta: qexed_registry_block_id("minecraft:terracotta")?,
            orange_terracotta: qexed_registry_block_id("minecraft:orange_terracotta")?,
            white_terracotta: qexed_registry_block_id("minecraft:white_terracotta")?,
            yellow_terracotta: qexed_registry_block_id("minecraft:yellow_terracotta")?,
            brown_terracotta: qexed_registry_block_id("minecraft:brown_terracotta")?,
            red_terracotta: qexed_registry_block_id("minecraft:red_terracotta")?,
            light_gray_terracotta: qexed_registry_block_id("minecraft:light_gray_terracotta")?,
            red_sand: qexed_registry_block_id("minecraft:red_sand")?,
            lava: qexed_registry_block_id("minecraft:lava")?,
            copper_ore: qexed_registry_block_id("minecraft:copper_ore")?,
            raw_copper_block: qexed_registry_block_id("minecraft:raw_copper_block")?,
            granite: qexed_registry_block_id("minecraft:granite")?,
            deepslate_iron_ore: qexed_registry_block_id("minecraft:deepslate_iron_ore")?,
            raw_iron_block: qexed_registry_block_id("minecraft:raw_iron_block")?,
            tuff: qexed_registry_block_id("minecraft:tuff")?,
        })
    }

    pub(crate) fn id(&self, block: vanilla_noise::SurfaceBlock) -> i32 {
        match block {
            vanilla_noise::SurfaceBlock::Bedrock => self.bedrock,
            vanilla_noise::SurfaceBlock::Stone => self.stone,
            vanilla_noise::SurfaceBlock::Deepslate => self.deepslate,
            vanilla_noise::SurfaceBlock::Dirt => self.dirt,
            vanilla_noise::SurfaceBlock::GrassBlock => self.grass_block,
            vanilla_noise::SurfaceBlock::Podzol => self.podzol,
            vanilla_noise::SurfaceBlock::CoarseDirt => self.coarse_dirt,
            vanilla_noise::SurfaceBlock::Mycelium => self.mycelium,
            vanilla_noise::SurfaceBlock::Calcite => self.calcite,
            vanilla_noise::SurfaceBlock::Gravel => self.gravel,
            vanilla_noise::SurfaceBlock::Sand => self.sand,
            vanilla_noise::SurfaceBlock::Sandstone => self.sandstone,
            vanilla_noise::SurfaceBlock::PackedIce => self.packed_ice,
            vanilla_noise::SurfaceBlock::Ice => self.ice,
            vanilla_noise::SurfaceBlock::SnowBlock => self.snow_block,
            vanilla_noise::SurfaceBlock::PowderSnow => self.powder_snow,
            vanilla_noise::SurfaceBlock::Mud => self.mud,
            vanilla_noise::SurfaceBlock::Water => self.water,
            vanilla_noise::SurfaceBlock::Terracotta => self.terracotta,
            vanilla_noise::SurfaceBlock::OrangeTerracotta => self.orange_terracotta,
            vanilla_noise::SurfaceBlock::WhiteTerracotta => self.white_terracotta,
            vanilla_noise::SurfaceBlock::YellowTerracotta => self.yellow_terracotta,
            vanilla_noise::SurfaceBlock::BrownTerracotta => self.brown_terracotta,
            vanilla_noise::SurfaceBlock::RedTerracotta => self.red_terracotta,
            vanilla_noise::SurfaceBlock::LightGrayTerracotta => self.light_gray_terracotta,
            vanilla_noise::SurfaceBlock::RedSand => self.red_sand,
        }
    }
}

pub(crate) fn block_state_id(value: &BlockStateJson) -> Result<i32> {
    if value.properties.is_empty() {
        return qexed_registry_block_id(&value.name);
    }

    let report = read_blocks_report()?;
    let block = report
        .get(&value.name)
        .with_context(|| format!("block not found in Mojang report: {}", value.name))?;
    let states = block
        .get("states")
        .and_then(serde_json::Value::as_array)
        .with_context(|| format!("block states missing in Mojang report: {}", value.name))?;
    let mut properties = value.properties.iter().collect::<Vec<_>>();
    properties.sort_by(|left, right| left.0.cmp(right.0));
    for state in states {
        let state_props = state
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .into_iter()
            .flat_map(|props| props.iter())
            .filter_map(|(key, value)| value.as_str().map(|value| (key, value)))
            .collect::<Vec<_>>();
        if state_props.len() == properties.len()
            && properties
                .iter()
                .all(|(key, value)| state_props.iter().any(|(k, v)| k == key && v == value))
            && let Some(id) = state.get("id").and_then(serde_json::Value::as_i64)
        {
            return i32::try_from(id).context("block state id does not fit i32");
        }
    }
    qexed_registry_block_id(&value.name)
}

pub(crate) fn qexed_registry_block_id(name: &str) -> Result<i32> {
    let name = normalize_identifier(name);
    if let Some(id) = cached_default_block_state_ids()
        .lock()
        .expect("block state id cache poisoned")
        .get(&name)
        .copied()
    {
        return Ok(id);
    }

    let report = read_blocks_report()?;
    let block = report
        .get(&name)
        .with_context(|| format!("block not found in Mojang report: {name}"))?;
    let states = block
        .get("states")
        .and_then(serde_json::Value::as_array)
        .with_context(|| format!("block states missing in Mojang report: {name}"))?;
    let default = states
        .iter()
        .find(|state| {
            state
                .get("default")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        })
        .or_else(|| states.first())
        .and_then(|state| state.get("id"))
        .and_then(serde_json::Value::as_i64)
        .with_context(|| format!("block state id missing in Mojang report: {name}"))?;
    let id = i32::try_from(default).context("block state id does not fit i32")?;
    cached_default_block_state_ids()
        .lock()
        .expect("block state id cache poisoned")
        .insert(name, id);
    Ok(id)
}

fn cached_default_block_state_ids() -> &'static Mutex<HashMap<String, i32>> {
    static IDS: OnceLock<Mutex<HashMap<String, i32>>> = OnceLock::new();
    IDS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn read_blocks_report() -> Result<&'static serde_json::Value> {
    static REPORT: OnceLock<Result<serde_json::Value, String>> = OnceLock::new();
    let result = REPORT.get_or_init(|| {
        let cache = WorldgenCache::default();
        read_json(&cache.reports_root().join("blocks.json"))
            .or_else(|_| qexed_registry::load_blocks_report())
            .map_err(|err| format!("{err:#}"))
    });
    result.as_ref().map_err(|err| anyhow::anyhow!("{err}"))
}
