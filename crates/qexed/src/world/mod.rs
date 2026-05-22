pub mod chunk_nbt;
pub mod region;

use anyhow::{Context, Result};
use bytes::BytesMut;
use qexed_packet::{PacketCodec, PacketWriter, net_types::VarInt};
use qexed_protocol::to_client::play::map_chunk::{Chunk, Heightmaps, Light, MapChunk};

const OVERWORLD_HEIGHT: i32 = 384;
const SECTION_HEIGHT: i32 = 16;
const AIR_BLOCK_STATE_ID: i32 = 0;
const PLAINS_BIOME_ID: i32 = 40;

#[derive(Clone, Debug)]
pub struct WorldManager {
    save_path: std::path::PathBuf,
}

impl WorldManager {
    pub fn new(save_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            save_path: save_path.into(),
        }
    }

    pub fn save_path(&self) -> &std::path::Path {
        &self.save_path
    }

    pub fn network_chunk(&self, dimension: &str, chunk_x: i32, chunk_z: i32) -> Result<MapChunk> {
        if let Some(chunk) = self.load_region_chunk(dimension, chunk_x, chunk_z)? {
            match chunk_nbt::network_chunk_from_region(chunk_x, chunk_z, &chunk) {
                Ok(packet) => {
                    log::debug!(
                        "loaded saved chunk as network chunk: dimension={dimension}, chunk=({chunk_x}, {chunk_z})"
                    );
                    return Ok(packet);
                }
                Err(err) => {
                    log::warn!(
                        "failed to convert saved chunk, falling back to empty chunk: dimension={dimension}, chunk=({chunk_x}, {chunk_z}), error={err:#}"
                    );
                }
            }
        }

        log::trace!("生成空世界区块: dimension={dimension}, chunk=({chunk_x}, {chunk_z})");
        Ok(empty_chunk_packet(chunk_x, chunk_z))
    }

    pub fn load_region_chunk(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Result<Option<region::ChunkData>> {
        log::trace!(
            "从存档读取区块: dimension={dimension}, chunk=({chunk_x}, {chunk_z}), root={}",
            self.save_path.display()
        );
        let region_path = self.dimension_region_path(dimension).join(format!(
            "r.{}.{}.mca",
            floor_div(chunk_x, 32),
            floor_div(chunk_z, 32)
        ));
        if !region_path.exists() {
            return Ok(None);
        }

        let region = region::AnvilRegion::from_file(&region_path)
            .with_context(|| format!("读取区域文件失败: {}", region_path.display()))?;
        region
            .read_chunk(chunk_x, chunk_z)
            .with_context(|| format!("读取区块失败: {chunk_x}, {chunk_z}"))
    }

    pub fn write_region_chunk(
        &self,
        dimension: &str,
        chunk_x: i32,
        chunk_z: i32,
        chunk: region::ChunkData,
    ) -> Result<()> {
        log::trace!(
            "写入区块到存档: dimension={dimension}, chunk=({chunk_x}, {chunk_z}), root={}",
            self.save_path.display()
        );
        let region_dir = self.dimension_region_path(dimension);
        std::fs::create_dir_all(&region_dir)
            .with_context(|| format!("创建区域目录失败: {}", region_dir.display()))?;

        let region_path = region_dir.join(format!(
            "r.{}.{}.mca",
            floor_div(chunk_x, 32),
            floor_div(chunk_z, 32)
        ));
        let mut region = if region_path.exists() {
            region::AnvilRegion::from_file(&region_path)?
        } else {
            region::AnvilRegion::new(&region_path)
        };
        region.write_chunk(chunk_x, chunk_z, chunk)?;
        region.save()?;
        Ok(())
    }

    pub fn ensure_storage(&self, dimension: &str) -> Result<()> {
        std::fs::create_dir_all(self.dimension_region_path(dimension))
            .with_context(|| format!("创建世界存档目录失败: {}", self.save_path.display()))
    }

    fn dimension_region_path(&self, dimension: &str) -> std::path::PathBuf {
        match dimension {
            "minecraft:the_nether" => self.save_path.join("DIM-1").join("region"),
            "minecraft:the_end" => self.save_path.join("DIM1").join("region"),
            _ => self.save_path.join("region"),
        }
    }
}

pub(crate) fn empty_chunk_packet(chunk_x: i32, chunk_z: i32) -> MapChunk {
    MapChunk {
        chunk_x,
        chunk_z,
        data: Chunk {
            heightmaps: empty_heightmaps(),
            data: empty_chunk_section_bytes(),
            block_entities: Vec::new(),
        },
        light: empty_light(),
    }
}

pub(crate) fn empty_heightmaps() -> Vec<Heightmaps> {
    vec![
        Heightmaps {
            type_id: VarInt(1),
            data: vec![0; 37],
        },
        Heightmaps {
            type_id: VarInt(4),
            data: vec![0; 37],
        },
        Heightmaps {
            type_id: VarInt(5),
            data: vec![0; 37],
        },
    ]
}

pub(crate) fn empty_light() -> Light {
    Light {
        sky_light_mask: qexed_packet::net_types::Bitset(Vec::new()),
        block_light_mask: qexed_packet::net_types::Bitset(Vec::new()),
        empty_sky_light_mask: qexed_packet::net_types::Bitset(Vec::new()),
        empty_block_light_mask: qexed_packet::net_types::Bitset(Vec::new()),
        sky_light_arrays: Vec::new(),
        block_light_arrays: Vec::new(),
    }
}

fn empty_chunk_section_bytes() -> Vec<u8> {
    let mut bytes = BytesMut::new();
    let mut writer = PacketWriter::new(&mut bytes);

    for _ in 0..section_count() {
        write_empty_section(&mut writer).expect("empty section packet data is infallible");
    }

    bytes.to_vec()
}

pub(crate) fn write_empty_section(writer: &mut PacketWriter) -> Result<()> {
    0_i16.serialize(writer)?;
    0_i16.serialize(writer)?;
    write_single_value_palette(writer, AIR_BLOCK_STATE_ID)?;
    write_single_value_palette(writer, PLAINS_BIOME_ID)?;
    Ok(())
}

pub(crate) fn write_single_value_palette(
    writer: &mut PacketWriter,
    registry_id: i32,
) -> Result<()> {
    0_u8.serialize(writer)?;
    VarInt(registry_id).serialize(writer)?;
    write_fixed_long_array(writer, &[])
}

pub(crate) fn write_fixed_long_array(writer: &mut PacketWriter, values: &[u64]) -> Result<()> {
    for value in values {
        value.serialize(writer)?;
    }
    Ok(())
}

pub(crate) fn section_count() -> i32 {
    OVERWORLD_HEIGHT / SECTION_HEIGHT
}

fn floor_div(value: i32, divisor: i32) -> i32 {
    value.div_euclid(divisor)
}

#[cfg(test)]
mod tests {
    use qexed_packet::Packet;

    use super::{WorldManager, empty_chunk_section_bytes, section_count};

    #[test]
    fn empty_chunk_has_all_overworld_sections() {
        let bytes = empty_chunk_section_bytes();
        assert_eq!(bytes.len(), section_count() as usize * 8);
    }

    #[test]
    fn empty_network_chunk_serializes() {
        let manager = WorldManager::new("world");
        let chunk = manager.network_chunk("minecraft:overworld", 0, 0).unwrap();
        let mut payload = bytes::BytesMut::new();
        let mut writer = qexed_packet::PacketWriter::new(&mut payload);

        chunk.serialize(&mut writer).unwrap();

        assert!(!payload.is_empty());
    }

    #[test]
    fn world_manager_writes_region_chunks() {
        let dir = tempfile::tempdir().unwrap();
        let manager = WorldManager::new(dir.path());
        let chunk = super::region::ChunkData::zlib(b"chunk").unwrap();

        manager
            .write_region_chunk("minecraft:overworld", 0, 0, chunk)
            .unwrap();

        let loaded = manager
            .load_region_chunk("minecraft:overworld", 0, 0)
            .unwrap()
            .unwrap();
        assert_eq!(loaded.decompress().unwrap(), b"chunk");
    }
}
