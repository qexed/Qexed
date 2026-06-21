use bytes::{Buf as _, Bytes, BytesMut};
use qexed_packet::{Packet as _, PacketCodec as _, PacketWriter, net_types::VarInt};
use qexed_protocol::to_client::play::map_chunk::{Chunk, Heightmaps, MapChunk};
use qexed_save::{DimensionId, RegionKind, SaveService, WorldStoreKind, region_coordinate};

pub mod chunk_nbt;
mod chunk_sync;
pub mod generator_rpc;
mod light;
mod manager;
pub mod region;
pub use chunk_sync::{
    ChunkPosition, PlayerChunkView, chunk_view_radius, chunk_window_positions,
    player_chunk_coordinate,
};
pub use light::{WorldLightAlgorithm, WorldLightMode};
pub use manager::{
    ChunkLoadEvent, ChunkSyncCause, ChunkSyncEvent, ChunkUnloadEvent, NetworkChunkLoad,
    NetworkChunkLoadSource, NetworkChunkSourceCounts, PrecompiledChunkSettings, WorldManager,
};

const OVERWORLD_HEIGHT: i32 = 384;
const SECTION_HEIGHT: i32 = 16;
const LIGHT_SECTION_COUNT: usize = (OVERWORLD_HEIGHT / SECTION_HEIGHT + 2) as usize;
const AIR_BLOCK_STATE_ID: i32 = 0;
const PLAINS_BIOME_ID: i32 = 40;
const DEFAULT_BIOME: &str = "minecraft:plains";
const WORLD_MIN_Y: i32 = -64;
const WORLD_MAX_Y: i32 = WORLD_MIN_Y + OVERWORLD_HEIGHT - 1;
const WORLD_SECTION_COUNT: usize = (OVERWORLD_HEIGHT / SECTION_HEIGHT) as usize;
const CHUNK_DAMPENING_LEN: usize = 16 * WORLD_SECTION_COUNT * 16 * 16;
const WORLD_MIN_SECTION_Y: i32 = WORLD_MIN_Y / SECTION_HEIGHT;
const MIN_LIGHT_SECTION_Y: i32 = WORLD_MIN_SECTION_Y - 1;

static EMPTY_CHUNK_BODY: std::sync::OnceLock<anyhow::Result<Bytes, String>> =
    std::sync::OnceLock::new();

#[derive(Debug, Clone)]
pub struct WorldStorage {
    save: SaveService,
}

impl WorldStorage {
    pub fn new(save: SaveService) -> Self {
        Self { save }
    }

    pub fn save(&self) -> &SaveService {
        &self.save
    }

    pub fn load_region_chunk(
        &self,
        dimension: &DimensionId,
        chunk_x: i32,
        chunk_z: i32,
    ) -> anyhow::Result<Option<region::ChunkData>> {
        self.ensure_region_readable()?;

        let region_path = self.region_path(dimension, chunk_x, chunk_z);
        if !region_path.exists() {
            return Ok(None);
        }

        let region = region::AnvilRegion::from_file(region_path)?;
        region.read_chunk(chunk_x, chunk_z)
    }

    pub fn write_region_chunk(
        &self,
        dimension: &DimensionId,
        chunk_x: i32,
        chunk_z: i32,
        chunk: region::ChunkData,
    ) -> anyhow::Result<()> {
        self.ensure_region_writable()?;

        let region_path = self.region_path(dimension, chunk_x, chunk_z);
        let mut region = if region_path.exists() {
            region::AnvilRegion::from_file(&region_path)?
        } else {
            region::AnvilRegion::new(&region_path)
        };
        region.write_chunk(chunk_x, chunk_z, chunk)?;
        region.save()
    }

    pub fn load_network_chunk(
        &self,
        dimension: &DimensionId,
        chunk_x: i32,
        chunk_z: i32,
        light_algorithm: WorldLightAlgorithm,
    ) -> anyhow::Result<Option<MapChunk>> {
        let Some(chunk) = self.load_region_chunk(dimension, chunk_x, chunk_z)? else {
            return Ok(None);
        };
        Ok(Some(
            chunk_nbt::network_chunk_and_light_dampening_from_region(
                chunk_x,
                chunk_z,
                &chunk,
                light_algorithm,
            )?
            .0,
        ))
    }

    pub fn block_state_at(
        &self,
        dimension: &DimensionId,
        position: &qexed_packet::net_types::Position,
    ) -> anyhow::Result<Option<i32>> {
        let chunk_x = position.x.div_euclid(16);
        let chunk_z = position.z.div_euclid(16);
        let Some(chunk) = self.load_region_chunk(dimension, chunk_x, chunk_z)? else {
            return Ok(None);
        };
        chunk_nbt::block_state_at_from_region(&chunk, position)
    }

    pub fn set_block_state(
        &self,
        dimension: &DimensionId,
        position: &qexed_packet::net_types::Position,
        block_state: i32,
        fallback_block_state: Option<i32>,
    ) -> anyhow::Result<()> {
        let chunk_x = position.x.div_euclid(16);
        let chunk_z = position.z.div_euclid(16);
        let existing = self.load_region_chunk(dimension, chunk_x, chunk_z)?;
        let updated = chunk_nbt::set_block_state_in_region(
            chunk_x,
            chunk_z,
            existing.as_ref(),
            position,
            block_state,
            fallback_block_state,
        )?;
        self.write_region_chunk(dimension, chunk_x, chunk_z, updated)
    }

    pub fn set_block_states(
        &self,
        dimension: &DimensionId,
        blocks: &[(qexed_packet::net_types::Position, i32, Option<i32>)],
    ) -> anyhow::Result<()> {
        let mut by_chunk = std::collections::BTreeMap::<(i32, i32), Vec<_>>::new();
        for (position, block_state, fallback_block_state) in blocks {
            by_chunk
                .entry((position.x.div_euclid(16), position.z.div_euclid(16)))
                .or_default()
                .push((position.clone(), *block_state, *fallback_block_state));
        }

        for ((chunk_x, chunk_z), chunk_blocks) in by_chunk {
            let existing = self.load_region_chunk(dimension, chunk_x, chunk_z)?;
            let updated = chunk_nbt::set_block_states_in_region(
                chunk_x,
                chunk_z,
                existing.as_ref(),
                &chunk_blocks,
            )?;
            self.write_region_chunk(dimension, chunk_x, chunk_z, updated)?;
        }

        Ok(())
    }

    pub async fn request_vanilla_generated_chunk(
        &self,
        client: &generator_rpc::VanillaWorldgenClient,
        dimension: &DimensionId,
        chunk_x: i32,
        chunk_z: i32,
    ) -> anyhow::Result<Option<region::ChunkData>> {
        self.ensure_region_writable()?;

        let region_path = absolute_path(self.region_path(dimension, chunk_x, chunk_z))?;
        client
            .generate_chunk(generator_rpc::GenerateChunkRequest::new(
                dimension,
                chunk_x,
                chunk_z,
                absolute_path(self.save.root().path())?,
                absolute_path(self.save.dimension_root(dimension))?,
                region_path,
            ))
            .await?;
        self.load_region_chunk(dimension, chunk_x, chunk_z)
    }

    pub fn request_local_generated_chunk(
        &self,
        generator: &qexed_worldgen::WorldGenerator,
        dimension: &DimensionId,
        chunk_x: i32,
        chunk_z: i32,
    ) -> anyhow::Result<Option<region::ChunkData>> {
        self.ensure_region_writable()?;

        let root = generator.generate_chunk_nbt(qexed_worldgen::ChunkRequest {
            dimension: &format!("{}:{}", dimension.namespace(), dimension.value()),
            chunk_x,
            chunk_z,
        })?;
        let chunk = chunk_nbt::region_chunk_from_nbt(chunk_x, chunk_z, &root)?;
        self.write_region_chunk(dimension, chunk_x, chunk_z, chunk)?;
        self.load_region_chunk(dimension, chunk_x, chunk_z)
    }

    pub fn region_path(
        &self,
        dimension: &DimensionId,
        chunk_x: i32,
        chunk_z: i32,
    ) -> std::path::PathBuf {
        self.save.region_path(
            dimension,
            RegionKind::Chunk,
            region_coordinate(chunk_x),
            region_coordinate(chunk_z),
        )
    }

    fn ensure_region_readable(&self) -> anyhow::Result<()> {
        let store = self.save.world_store(WorldStoreKind::Region);
        if !store.enabled || !store.mode.can_read() {
            anyhow::bail!("世界区块存储未启用读取");
        }
        Ok(())
    }

    fn ensure_region_writable(&self) -> anyhow::Result<()> {
        let store = self.save.world_store(WorldStoreKind::Region);
        if !store.enabled || !store.mode.can_write() {
            anyhow::bail!("世界区块存储未启用写入");
        }
        Ok(())
    }
}

fn absolute_path(path: impl AsRef<std::path::Path>) -> anyhow::Result<std::path::PathBuf> {
    let path = path.as_ref();
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    Ok(std::env::current_dir()?.join(path))
}

pub fn empty_chunk_packet(chunk_x: i32, chunk_z: i32) -> anyhow::Result<MapChunk> {
    Ok(MapChunk {
        chunk_x,
        chunk_z,
        data: Chunk {
            heightmaps: empty_heightmaps(),
            data: empty_chunk_section_bytes()?,
            block_entities: Vec::new(),
        },
        light: light::light_for_mode(WorldLightMode::Static),
    })
}

pub fn empty_chunk_body_bytes() -> anyhow::Result<&'static Bytes> {
    match EMPTY_CHUNK_BODY
        .get_or_init(|| build_empty_chunk_body_bytes().map_err(|err| format!("{err:#}")))
    {
        Ok(bytes) => Ok(bytes),
        Err(err) => anyhow::bail!("{err}"),
    }
}

fn build_empty_chunk_body_bytes() -> anyhow::Result<Bytes> {
    let chunk = empty_chunk_packet(0, 0)?;
    let mut payload = BytesMut::new();
    {
        let mut writer = PacketWriter::new(&mut payload);
        chunk.serialize(&mut writer)?;
    }
    if payload.len() < 8 {
        anyhow::bail!("empty chunk payload is too short: {}", payload.len());
    }
    payload.advance(8);
    Ok(payload.freeze())
}

fn empty_heightmaps() -> Vec<Heightmaps> {
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

fn empty_chunk_section_bytes() -> anyhow::Result<Vec<u8>> {
    let biome_id =
        qexed_registry::dynamic_registry_entry_id("worldgen/biome", DEFAULT_BIOME)?.unwrap_or(40);
    let mut bytes = BytesMut::new();
    let mut writer = PacketWriter::new(&mut bytes);

    for _ in 0..section_count() {
        write_empty_section(&mut writer, biome_id)?;
    }

    Ok(bytes.to_vec())
}

fn write_empty_section(writer: &mut PacketWriter, biome_id: i32) -> anyhow::Result<()> {
    0_i16.serialize(writer)?;
    0_i16.serialize(writer)?;
    write_single_value_palette(writer, AIR_BLOCK_STATE_ID)?;
    write_single_value_palette(writer, biome_id)?;
    Ok(())
}

pub(crate) fn write_single_value_palette(
    writer: &mut PacketWriter,
    registry_id: i32,
) -> anyhow::Result<()> {
    0_u8.serialize(writer)?;
    VarInt(registry_id).serialize(writer)?;
    light::write_fixed_long_array(writer, &[])?;
    Ok(())
}

fn section_count() -> i32 {
    OVERWORLD_HEIGHT / SECTION_HEIGHT
}

#[cfg(test)]
mod tests {
    use super::{
        LIGHT_SECTION_COUNT, WorldStorage, chunk_nbt::default_block_state_id,
        empty_chunk_body_bytes, empty_chunk_packet,
    };
    use bytes::BytesMut;
    use qexed_packet::net_types::Position;
    use qexed_packet::{Packet as _, PacketWriter};

    #[test]
    fn empty_chunk_has_vanilla_height_and_light_shape() {
        let chunk = empty_chunk_packet(0, 0).unwrap();

        assert_eq!(chunk.data.heightmaps.len(), 3);
        assert_eq!(chunk.data.data.len(), 24 * 8);
        assert_eq!(chunk.light.sky_light_arrays.len(), LIGHT_SECTION_COUNT);
        assert!(chunk.light.block_light_arrays.is_empty());
    }

    #[test]
    fn empty_chunk_serializes_as_map_chunk_payload() {
        let chunk = empty_chunk_packet(0, 0).unwrap();
        let mut payload = BytesMut::new();
        chunk
            .serialize(&mut PacketWriter::new(&mut payload))
            .unwrap();

        assert!(payload.len() > chunk.data.data.len());
        assert_eq!(&payload[..4], &[0, 0, 0, 0]);
    }

    #[test]
    fn empty_chunk_body_template_matches_serialized_chunk_without_coordinates() {
        let chunk = empty_chunk_packet(0, 0).unwrap();
        let mut payload = BytesMut::new();
        chunk
            .serialize(&mut PacketWriter::new(&mut payload))
            .unwrap();

        assert_eq!(empty_chunk_body_bytes().unwrap().as_ref(), &payload[8..]);
    }

    #[test]
    fn world_storage_reads_and_writes_java_26_2_region_path() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = qexed_config::app::qexed_save::Save::default();
        config.root.universe = dir.path().to_string_lossy().to_string();
        config.root.world = "world".to_string();
        let save = qexed_save::SaveService::new(config).unwrap();
        let storage = WorldStorage::new(save);

        storage
            .write_region_chunk(
                &qexed_save::DimensionId::overworld(),
                -1,
                32,
                super::region::ChunkData::zlib(b"chunk").unwrap(),
            )
            .unwrap();

        assert!(
            dir.path()
                .join("world/dimensions/minecraft/overworld/region/r.-1.1.mca")
                .exists()
        );
        let loaded = storage
            .load_region_chunk(&qexed_save::DimensionId::overworld(), -1, 32)
            .unwrap()
            .unwrap();
        assert_eq!(loaded.decompress().unwrap(), b"chunk");
    }

    #[test]
    fn world_storage_respects_region_read_only_mode() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = qexed_config::app::qexed_save::Save::default();
        config.root.universe = dir.path().to_string_lossy().to_string();
        config.world.region.mode = qexed_config::app::qexed_save::StorageMode::ReadOnly;
        let save = qexed_save::SaveService::new(config).unwrap();
        let storage = WorldStorage::new(save);

        assert!(
            storage
                .load_region_chunk(&qexed_save::DimensionId::overworld(), 0, 0)
                .unwrap()
                .is_none()
        );
        assert!(
            storage
                .write_region_chunk(
                    &qexed_save::DimensionId::overworld(),
                    0,
                    0,
                    super::region::ChunkData::zlib(b"chunk").unwrap(),
                )
                .is_err()
        );
    }

    #[test]
    fn world_storage_sets_and_reads_saved_block_state() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = qexed_config::app::qexed_save::Save::default();
        config.root.universe = dir.path().to_string_lossy().to_string();
        let save = qexed_save::SaveService::new(config).unwrap();
        let storage = WorldStorage::new(save);
        let position = Position { x: -1, y: 64, z: 2 };
        let stone = default_block_state_id("minecraft:stone");

        storage
            .set_block_state(
                &qexed_save::DimensionId::overworld(),
                &position,
                stone,
                None,
            )
            .unwrap();

        assert_eq!(
            storage
                .block_state_at(&qexed_save::DimensionId::overworld(), &position)
                .unwrap(),
            Some(stone)
        );
        assert!(
            storage
                .load_network_chunk(
                    &qexed_save::DimensionId::overworld(),
                    -1,
                    0,
                    super::WorldLightAlgorithm::Fast,
                )
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn external_worldgen_region_can_be_loaded_when_configured() {
        let Some(root) = std::env::var_os("QEXED_WORLDGEN_E2E_UNIVERSE") else {
            return;
        };
        let mut config = qexed_config::app::qexed_save::Save::default();
        config.root.universe = root.to_string_lossy().to_string();
        config.root.world = "world".to_string();
        let storage = WorldStorage::new(qexed_save::SaveService::new(config).unwrap());

        let chunk = storage
            .load_region_chunk(&qexed_save::DimensionId::overworld(), 0, 0)
            .unwrap()
            .expect("external worldgen chunk should exist");
        assert!(!chunk.decompress().unwrap().is_empty());
        assert!(
            storage
                .load_network_chunk(
                    &qexed_save::DimensionId::overworld(),
                    0,
                    0,
                    super::WorldLightAlgorithm::Fast,
                )
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn worldgen_request_uses_absolute_paths() {
        let mut config = qexed_config::app::qexed_save::Save::default();
        config.root.universe = ".".to_string();
        config.root.world = "world".to_string();
        let storage = WorldStorage::new(qexed_save::SaveService::new(config).unwrap());
        let dimension = qexed_save::DimensionId::overworld();
        let region_path = super::absolute_path(storage.region_path(&dimension, -1, -1)).unwrap();

        assert!(region_path.is_absolute());
        assert!(region_path.ends_with("world/dimensions/minecraft/overworld/region/r.-1.-1.mca"));
    }
}
