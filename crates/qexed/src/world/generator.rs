use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use qexed_config::app::qexed::server::{
    World as WorldConfig, WorldGenerator as WorldGeneratorConfig,
};
use qexed_nbt::{ListHeader, Tag, tag_id};
use qexed_protocol::to_client::play::map_chunk::MapChunk;
use serde::Deserialize;

use super::{
    CHUNK_DAMPENING_LEN, SECTION_HEIGHT, WORLD_MAX_Y, WORLD_MIN_SECTION_Y, WORLD_MIN_Y,
    WorldLightAlgorithm, chunk_nbt, empty_chunk_packet, section_count,
};

const DEFAULT_FLAT_PRESET: &str = "minecraft:classic_flat";
const FLAT_PRESET_ROOT: &str =
    "assets/decompiled_source/src/data/minecraft/worldgen/flat_level_generator_preset";
const HEIGHTMAP_BITS: usize = 9;
const HEIGHTMAP_ENTRY_COUNT: usize = 16 * 16;
const BLOCK_ENTRY_COUNT: usize = 16 * 16 * 16;

pub(crate) struct GeneratedChunk {
    pub packet: MapChunk,
    pub light_dampening: Vec<u8>,
}

pub(crate) trait WorldChunkGenerator: Send + Sync + std::fmt::Debug {
    fn generate(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        light_algorithm: WorldLightAlgorithm,
    ) -> Result<GeneratedChunk>;

    fn light_dampening(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        light_algorithm: WorldLightAlgorithm,
    ) -> Result<Vec<u8>> {
        Ok(self
            .generate(dimension, chunk_x, chunk_z, light_algorithm)?
            .light_dampening)
    }

    fn block_state_at(
        &self,
        dimension: &str,
        position: &qexed_packet::net_types::Position,
    ) -> Option<i32>;
}

pub(crate) fn from_config(config: &WorldConfig) -> Arc<dyn WorldChunkGenerator> {
    match config.generator {
        WorldGeneratorConfig::Empty => Arc::new(EmptyWorldGenerator),
        WorldGeneratorConfig::VanillaFlat => Arc::new(VanillaFlatGenerator::from_preset(
            config.generator_preset.trim(),
        )),
    }
}

#[derive(Debug)]
pub(crate) struct EmptyWorldGenerator;

impl WorldChunkGenerator for EmptyWorldGenerator {
    fn generate(
        &self,
        _dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        _light_algorithm: WorldLightAlgorithm,
    ) -> Result<GeneratedChunk> {
        Ok(GeneratedChunk {
            packet: empty_chunk_packet(chunk_x, chunk_z, super::WorldLightMode::Static),
            light_dampening: vec![0; CHUNK_DAMPENING_LEN],
        })
    }

    fn light_dampening(
        &self,
        _dimension: &str,
        _chunk_x: i32,
        _chunk_z: i32,
        _light_algorithm: WorldLightAlgorithm,
    ) -> Result<Vec<u8>> {
        Ok(vec![0; CHUNK_DAMPENING_LEN])
    }

    fn block_state_at(
        &self,
        _dimension: &str,
        _position: &qexed_packet::net_types::Position,
    ) -> Option<i32> {
        None
    }
}

#[derive(Debug)]
pub(crate) struct VanillaFlatGenerator {
    layers: Vec<FlatLayer>,
    root: Tag,
}

impl VanillaFlatGenerator {
    pub(crate) fn from_preset(preset: &str) -> Self {
        let preset = if preset.is_empty() {
            DEFAULT_FLAT_PRESET
        } else {
            preset
        };

        match load_flat_preset(preset) {
            Ok(settings) => Self::from_settings(settings),
            Err(err) => {
                log::warn!(
                    "failed to load vanilla flat preset {preset}, using classic_flat: {err:#}"
                );
                Self::from_settings(FlatSettings::classic())
            }
        }
    }

    fn from_settings(settings: FlatSettings) -> Self {
        let layers = expand_layers(settings.layers);
        let biome = normalize_identifier(&settings.biome);
        let root = flat_chunk_root(&layers, &biome);
        Self { layers, root }
    }
}

impl WorldChunkGenerator for VanillaFlatGenerator {
    fn generate(
        &self,
        _dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        light_algorithm: WorldLightAlgorithm,
    ) -> Result<GeneratedChunk> {
        let (packet, light_dampening) = chunk_nbt::network_chunk_and_light_dampening_from_nbt(
            chunk_x,
            chunk_z,
            &self.root,
            light_algorithm,
        )?;
        Ok(GeneratedChunk {
            packet,
            light_dampening,
        })
    }

    fn block_state_at(
        &self,
        _dimension: &str,
        position: &qexed_packet::net_types::Position,
    ) -> Option<i32> {
        let layer_index = position.y - WORLD_MIN_Y;
        let layer = usize::try_from(layer_index)
            .ok()
            .and_then(|index| self.layers.get(index))?;
        (!layer.is_air).then_some(layer.block_state_id)
    }
}

#[derive(Debug, Clone)]
struct FlatSettings {
    biome: String,
    layers: Vec<FlatLayerSetting>,
}

impl FlatSettings {
    fn classic() -> Self {
        Self {
            biome: "minecraft:plains".to_string(),
            layers: vec![
                FlatLayerSetting {
                    block: "minecraft:bedrock".to_string(),
                    height: 1,
                },
                FlatLayerSetting {
                    block: "minecraft:dirt".to_string(),
                    height: 2,
                },
                FlatLayerSetting {
                    block: "minecraft:grass_block".to_string(),
                    height: 1,
                },
            ],
        }
    }
}

#[derive(Debug, Clone)]
struct FlatLayerSetting {
    block: String,
    height: usize,
}

#[derive(Debug, Clone)]
struct FlatLayer {
    block: String,
    block_state_id: i32,
    properties: Vec<(String, String)>,
    is_air: bool,
}

#[derive(Deserialize)]
struct FlatPresetFile {
    settings: FlatPresetSettings,
}

#[derive(Deserialize)]
struct FlatPresetSettings {
    biome: String,
    layers: Vec<FlatPresetLayer>,
}

#[derive(Deserialize)]
struct FlatPresetLayer {
    block: String,
    height: usize,
}

fn load_flat_preset(preset: &str) -> Result<FlatSettings> {
    let path = flat_preset_path(preset)?;
    let content =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let preset: FlatPresetFile =
        serde_json::from_str(&content).with_context(|| format!("parse {}", path.display()))?;
    Ok(FlatSettings {
        biome: preset.settings.biome,
        layers: preset
            .settings
            .layers
            .into_iter()
            .map(|layer| FlatLayerSetting {
                block: layer.block,
                height: layer.height,
            })
            .collect(),
    })
}

fn flat_preset_path(preset: &str) -> Result<PathBuf> {
    let preset = preset.trim();
    let name = preset
        .strip_prefix("minecraft:")
        .or_else(|| preset.strip_prefix(':'))
        .unwrap_or(preset)
        .trim_matches('/');
    if name.is_empty()
        || Path::new(name)
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        anyhow::bail!("invalid flat preset name: {preset}");
    }
    Ok(workspace_root()
        .join(FLAT_PRESET_ROOT)
        .join(name)
        .with_extension("json"))
}

fn expand_layers(settings: Vec<FlatLayerSetting>) -> Vec<FlatLayer> {
    let mut layers = Vec::new();
    for layer in settings {
        let block = normalize_identifier(&layer.block);
        let block_state = chunk_nbt::default_block_state(&block);
        let is_air = is_air_block(&block);
        for _ in 0..layer.height {
            layers.push(FlatLayer {
                block: block.clone(),
                block_state_id: block_state.id,
                properties: block_state.properties.clone(),
                is_air,
            });
        }
        if layers.len() >= super::WORLD_SECTION_COUNT * SECTION_HEIGHT as usize {
            layers.truncate(super::WORLD_SECTION_COUNT * SECTION_HEIGHT as usize);
            break;
        }
    }
    layers
}

fn flat_chunk_root(layers: &[FlatLayer], biome: &str) -> Tag {
    compound_tag([
        ("sections", sections_tag(layers, biome)),
        ("Heightmaps", heightmaps_tag(layers)),
    ])
}

fn sections_tag(layers: &[FlatLayer], biome: &str) -> Tag {
    let mut sections = Vec::new();
    for section_y in WORLD_MIN_SECTION_Y..WORLD_MIN_SECTION_Y + section_count() {
        let start_layer = ((section_y * SECTION_HEIGHT) - WORLD_MIN_Y) as usize;
        let end_layer = start_layer + SECTION_HEIGHT as usize;
        let section_layers = &layers[start_layer.min(layers.len())..end_layer.min(layers.len())];
        if section_layers.iter().all(|layer| layer.is_air) {
            continue;
        }
        sections.push(section_tag(section_y, section_layers, biome));
    }

    Tag::List(
        ListHeader {
            tag_id: tag_id::COMPOUND,
            length: sections.len() as i32,
        },
        Arc::from(sections),
    )
}

fn section_tag(section_y: i32, layers: &[FlatLayer], biome: &str) -> Tag {
    compound_tag([
        ("Y", Tag::Byte(section_y as i8)),
        ("block_states", block_states_tag(layers)),
        ("biomes", biomes_tag(biome)),
    ])
}

fn block_states_tag(layers: &[FlatLayer]) -> Tag {
    let mut palette = Vec::new();
    let mut index_by_block = HashMap::<String, usize>::new();
    let mut values = vec![0_i32; BLOCK_ENTRY_COUNT];
    let air = chunk_nbt::default_block_state("minecraft:air");

    for local_y in 0..SECTION_HEIGHT as usize {
        let (block, properties) = layers
            .get(local_y)
            .map(|layer| (layer.block.as_str(), layer.properties.as_slice()))
            .unwrap_or(("minecraft:air", air.properties.as_slice()));
        let key = block_state_palette_key(block, properties);
        let next_index = palette.len();
        let palette_index = *index_by_block.entry(key).or_insert_with(|| {
            palette.push(block_state_tag(block, properties));
            next_index
        });

        for z in 0..16 {
            for x in 0..16 {
                values[(local_y * 16 + z) * 16 + x] = palette_index as i32;
            }
        }
    }

    let data = (palette.len() > 1).then(|| {
        let bits = palette_storage_bits(palette.len());
        pack_fixed_long_values(&values, bits)
            .into_iter()
            .map(|value| value as i64)
            .collect::<Vec<_>>()
    });
    paletted_container(palette, data)
}

fn biomes_tag(biome: &str) -> Tag {
    paletted_container(vec![Tag::String(Arc::from(biome.to_string()))], None)
}

fn heightmaps_tag(layers: &[FlatLayer]) -> Tag {
    let height = flat_first_available_height(layers);
    let packed = pack_fixed_long_values(&vec![height; HEIGHTMAP_ENTRY_COUNT], HEIGHTMAP_BITS)
        .into_iter()
        .map(|value| value as i64)
        .collect::<Vec<_>>();
    compound_tag([
        ("WORLD_SURFACE", Tag::LongArray(Arc::from(packed.clone()))),
        ("MOTION_BLOCKING", Tag::LongArray(Arc::from(packed.clone()))),
        (
            "MOTION_BLOCKING_NO_LEAVES",
            Tag::LongArray(Arc::from(packed)),
        ),
    ])
}

fn flat_first_available_height(layers: &[FlatLayer]) -> i32 {
    let Some(highest) = layers.iter().rposition(|layer| !layer.is_air) else {
        return 0;
    };
    let height = WORLD_MIN_Y + highest as i32 + 1;
    (height - WORLD_MIN_Y).clamp(0, WORLD_MAX_Y - WORLD_MIN_Y + 1)
}

fn block_state_tag(name: &str, properties: &[(String, String)]) -> Tag {
    let mut fields = HashMap::new();
    fields.insert("Name".to_string(), Tag::String(Arc::from(name.to_string())));
    if !properties.is_empty() {
        fields.insert(
            "Properties".to_string(),
            Tag::Compound(Arc::new(
                properties
                    .iter()
                    .map(|(key, value)| (key.clone(), Tag::String(Arc::from(value.clone()))))
                    .collect(),
            )),
        );
    }
    Tag::Compound(Arc::new(fields))
}

fn block_state_palette_key(name: &str, properties: &[(String, String)]) -> String {
    let mut key = name.to_string();
    key.push('|');
    for (name, value) in properties {
        key.push_str(name);
        key.push('=');
        key.push_str(value);
        key.push(';');
    }
    key
}

fn paletted_container(palette: Vec<Tag>, data: Option<Vec<i64>>) -> Tag {
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

fn compound_tag<I>(fields: I) -> Tag
where
    I: IntoIterator<Item = (&'static str, Tag)>,
{
    Tag::Compound(Arc::new(
        fields
            .into_iter()
            .map(|(name, value)| (name.to_string(), value))
            .collect(),
    ))
}

fn palette_storage_bits(palette_len: usize) -> usize {
    ceil_log2(palette_len).max(4)
}

fn pack_fixed_long_values(values: &[i32], bits: usize) -> Vec<u64> {
    let values_per_long = 64 / bits;
    let mask = (1_u64 << bits) - 1;
    let mut packed = vec![0_u64; values.len().div_ceil(values_per_long)];

    for (index, value) in values.iter().enumerate() {
        let cell = index / values_per_long;
        let bit_offset = (index % values_per_long) * bits;
        packed[cell] |= ((*value as u64) & mask) << bit_offset;
    }

    packed
}

fn normalize_identifier(value: &str) -> String {
    let value = value.trim();
    if value.contains(':') {
        value.to_string()
    } else {
        format!("minecraft:{value}")
    }
}

fn is_air_block(name: &str) -> bool {
    matches!(
        name,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    )
}

fn ceil_log2(count: usize) -> usize {
    if count <= 1 {
        0
    } else {
        usize::BITS as usize - (count - 1).leading_zeros() as usize
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use qexed_packet::Packet;

    use super::*;

    #[test]
    fn vanilla_flat_uses_classic_flat_layers_from_min_y() {
        let generator = VanillaFlatGenerator::from_preset(DEFAULT_FLAT_PRESET);
        let bedrock = chunk_nbt::default_block_state_id("minecraft:bedrock");
        let dirt = chunk_nbt::default_block_state_id("minecraft:dirt");
        let grass = chunk_nbt::default_block_state_id("minecraft:grass_block");

        assert_eq!(
            generator.block_state_at(
                "minecraft:overworld",
                &qexed_packet::net_types::Position {
                    x: 0,
                    y: WORLD_MIN_Y,
                    z: 0
                }
            ),
            Some(bedrock)
        );
        assert_eq!(
            generator.block_state_at(
                "minecraft:overworld",
                &qexed_packet::net_types::Position {
                    x: 0,
                    y: WORLD_MIN_Y + 2,
                    z: 0
                }
            ),
            Some(dirt)
        );
        assert_eq!(
            generator.block_state_at(
                "minecraft:overworld",
                &qexed_packet::net_types::Position {
                    x: 0,
                    y: WORLD_MIN_Y + 3,
                    z: 0
                }
            ),
            Some(grass)
        );
        assert_eq!(
            generator.block_state_at(
                "minecraft:overworld",
                &qexed_packet::net_types::Position {
                    x: 0,
                    y: WORLD_MIN_Y + 4,
                    z: 0
                }
            ),
            None
        );
    }

    #[test]
    fn vanilla_flat_generated_chunk_serializes() {
        let generator = VanillaFlatGenerator::from_preset(DEFAULT_FLAT_PRESET);
        let generated = generator
            .generate("minecraft:overworld", 0, 0, WorldLightAlgorithm::Fast)
            .unwrap();
        let mut payload = bytes::BytesMut::new();
        let mut writer = qexed_packet::PacketWriter::new(&mut payload);

        generated.packet.serialize(&mut writer).unwrap();

        assert!(!payload.is_empty());
        assert_eq!(generated.light_dampening.len(), CHUNK_DAMPENING_LEN);
        assert_eq!(generated.packet.data.heightmaps.len(), 3);
    }

    #[test]
    fn flat_heightmap_uses_vanilla_first_available_offset() {
        let layers = expand_layers(FlatSettings::classic().layers);
        let packed = match heightmaps_tag(&layers) {
            Tag::Compound(fields) => fields
                .get("WORLD_SURFACE")
                .and_then(|tag| match tag {
                    Tag::LongArray(values) => Some(values.clone()),
                    _ => None,
                })
                .unwrap(),
            _ => panic!("heightmaps must be compound"),
        };

        assert_eq!(packed.len(), 37);
        assert_eq!(packed[0] & 0x1ff, 4);
    }
}
