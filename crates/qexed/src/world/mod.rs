pub mod chunk_nbt;
pub mod generator;
#[cfg(feature = "gpu")]
mod gpu_light;
#[cfg(not(feature = "gpu"))]
mod gpu_light {
    use anyhow::{Result, bail};
    use qexed_config::app::qexed::server::GpuDeviceSelector;

    use super::LightDampeningNeighborhood;

    #[derive(Debug)]
    pub struct GpuLightEngine;

    impl GpuLightEngine {
        pub fn new(_selector: &GpuDeviceSelector) -> Result<Self> {
            bail!("gpu feature is disabled at compile time")
        }

        pub fn fast_sky_light(
            &self,
            _neighbourhood: &LightDampeningNeighborhood,
        ) -> Result<Vec<u8>> {
            bail!("gpu feature is disabled at compile time")
        }
    }
}
#[cfg(feature = "gpu")]
mod gpu_worldgen;
#[cfg(not(feature = "gpu"))]
mod gpu_worldgen {
    use anyhow::{Result, bail};
    use qexed_config::app::qexed::server::GpuDeviceSelector;

    #[derive(Debug)]
    pub(crate) struct GpuWorldgenEngine;

    impl GpuWorldgenEngine {
        pub(crate) fn new(_selector: &GpuDeviceSelector, _height: i32) -> Result<Self> {
            bail!("gpu feature is disabled at compile time")
        }

        pub(crate) fn first_available_heights(&self, _solid_mask: &[u32]) -> Result<Vec<i32>> {
            bail!("gpu feature is disabled at compile time")
        }
    }
}
mod light;
mod manager;
mod ore_pits;
pub mod region;
mod rules;
mod vanilla_noise;

#[cfg(test)]
pub(crate) use light::empty_chunk_section_bytes;
pub(crate) use light::{
    LightDampeningNeighborhood, block_light_dampening_index, empty_chunk_packet, empty_heightmaps,
    light_for_mode, light_from_layers, light_from_sky_values, light_section_index,
    light_update_data, replace_sky_light, section_count, sky_light_from_dampening,
    sky_light_from_neighbourhood, write_empty_section, write_fixed_long_array,
};
pub use light::{WorldLightAlgorithm, WorldLightMode, light_gpu_from_config};
pub use manager::{PrecompiledChunkSettings, RuntimeEditRegion, WorldManager, WorldSession};
pub use ore_pits::{OrePitBlockUpdate, OrePitManager};
pub use rules::WorldRulesManager;

const OVERWORLD_HEIGHT: i32 = 384;
const SECTION_HEIGHT: i32 = 16;
const LIGHT_SECTION_COUNT: usize = (OVERWORLD_HEIGHT / SECTION_HEIGHT + 2) as usize;
pub(crate) const WORLD_MIN_Y: i32 = -64;
pub(crate) const WORLD_MAX_Y: i32 = WORLD_MIN_Y + OVERWORLD_HEIGHT - 1;
pub(crate) const WORLD_SECTION_COUNT: usize = (OVERWORLD_HEIGHT / SECTION_HEIGHT) as usize;
const CHUNK_DAMPENING_LEN: usize = 16 * WORLD_SECTION_COUNT * 16 * 16;
pub(crate) const WORLD_MIN_SECTION_Y: i32 = WORLD_MIN_Y / SECTION_HEIGHT;
const MIN_LIGHT_SECTION_Y: i32 = WORLD_MIN_SECTION_Y - 1;
const AIR_BLOCK_STATE_ID: i32 = 0;
const PLAINS_BIOME_ID: i32 = 40;

#[cfg(test)]
mod tests;
