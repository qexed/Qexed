use bytes::{Buf as _, Bytes, BytesMut};
use qexed_packet::{
    Packet as _, PacketCodec as _, PacketWriter,
    net_types::{Bitset, VarInt},
};
use qexed_protocol::to_client::play::map_chunk::{
    Chunk, Heightmaps, LIGHT_ARRAY_BYTES, Light, LightArray, MapChunk,
};

const OVERWORLD_HEIGHT: i32 = 384;
const SECTION_HEIGHT: i32 = 16;
const LIGHT_SECTION_COUNT: usize = (OVERWORLD_HEIGHT / SECTION_HEIGHT + 2) as usize;
const AIR_BLOCK_STATE_ID: i32 = 0;
const DEFAULT_BIOME: &str = "minecraft:plains";

static EMPTY_CHUNK_BODY: std::sync::OnceLock<anyhow::Result<Bytes, String>> =
    std::sync::OnceLock::new();

pub fn empty_chunk_packet(chunk_x: i32, chunk_z: i32) -> anyhow::Result<MapChunk> {
    Ok(MapChunk {
        chunk_x,
        chunk_z,
        data: Chunk {
            heightmaps: empty_heightmaps(),
            data: empty_chunk_section_bytes()?,
            block_entities: Vec::new(),
        },
        light: static_sky_light(),
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

fn write_single_value_palette(writer: &mut PacketWriter, registry_id: i32) -> anyhow::Result<()> {
    0_u8.serialize(writer)?;
    VarInt(registry_id).serialize(writer)?;
    Ok(())
}

fn static_sky_light() -> Light {
    Light {
        sky_light_mask: full_light_section_mask(),
        block_light_mask: Bitset(Vec::new()),
        empty_sky_light_mask: Bitset(Vec::new()),
        empty_block_light_mask: full_light_section_mask(),
        sky_light_arrays: filled_light_arrays(15),
        block_light_arrays: Vec::new(),
    }
}

fn full_light_section_mask() -> Bitset {
    Bitset(vec![(1_u64 << LIGHT_SECTION_COUNT) - 1])
}

fn filled_light_arrays(level: u8) -> Vec<LightArray> {
    let level = level.min(15);
    if level == 0 {
        return Vec::new();
    }

    let packed = (level & 0x0f) | ((level & 0x0f) << 4);
    vec![LightArray([packed; LIGHT_ARRAY_BYTES]); LIGHT_SECTION_COUNT]
}

fn section_count() -> i32 {
    OVERWORLD_HEIGHT / SECTION_HEIGHT
}

#[cfg(test)]
mod tests {
    use super::{LIGHT_SECTION_COUNT, empty_chunk_body_bytes, empty_chunk_packet};
    use bytes::BytesMut;
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
}
