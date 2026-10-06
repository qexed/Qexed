use qexed_packet::{
    Packet, PacketCodec, PacketReader, PacketWriter,
    net_types::{ByteArray, OptionalNbt, VarInt},
};

#[qexed_packet_macros::packet(id = 0x2e)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct LevelChunkWithLight {
    // 注意：x/z 用 ByteBufCodecs.INT（普通 i32），不是 VarInt
    pub x: i32,
    pub z: i32,
    pub chunk_data: LevelChunkPacketData,
    pub light_data: LightUpdatePacketData,
}

/// net.minecraft.network.protocol.game.ClientboundLevelChunkPacketData。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct LevelChunkPacketData {
    // TODO: Map<Heightmap$Types, long[]> — 键为 Heightmap Types 的 id（VarInt，见 HEIGHTMAP_IDS），值为 long 数组
    pub heightmaps: Vec<Heightmap>,
    // TODO: 区块数据缓冲区（原始方块数据 + 生物群系数据），VarInt 长度前缀字节串
    pub buffer: ByteArray,
    pub block_entities: Vec<BlockEntityInfo>,
}

/// Heightmap$Types 条目：类型 id + long 数组。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Heightmap {
    pub kind: VarInt,
    pub data: Vec<i64>,
}

/// Heightmap$Types 的 id 映射（BY_ID 顺序）。
pub const HEIGHTMAP_IDS: &[&str] = &[
    "WORLD_SURFACE_WG",
    "WORLD_SURFACE",
    "OCEAN_FLOOR_WG",
    "OCEAN_FLOOR",
    "MOTION_BLOCKING",
    "MOTION_BLOCKING_NO_LEAVES",
];

impl PacketCodec for LevelChunkPacketData {
    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        VarInt(self.heightmaps.len() as i32).serialize(w)?;
        for heightmap in &self.heightmaps {
            heightmap.kind.serialize(w)?;
            write_long_array(w, &heightmap.data)?;
        }

        self.buffer.serialize(w)?;
        self.block_entities.serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        let mut count = VarInt::default();
        count.deserialize(r)?;
        if count.0 < 0 {
            return Err(qexed_packet::PacketError::msg(format!(
                "negative heightmap count: {}",
                count.0
            )));
        }
        self.heightmaps.clear();
        for _ in 0..count.0 {
            let mut kind = VarInt::default();
            kind.deserialize(r)?;
            let data = read_long_array(r)?;
            self.heightmaps.push(Heightmap { kind, data });
        }

        self.buffer.deserialize(r)?;
        self.block_entities.deserialize(r)
    }
}

/// ClientboundLevelChunkPacketData$BlockEntityInfo。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct BlockEntityInfo {
    pub packed_xz: i8,
    pub y: i16,
    // TODO: BlockEntityType<?> — 注册表 id，用 VarInt 占位
    pub block_entity_type: VarInt,
    pub tag: OptionalNbt,
}

impl PacketCodec for BlockEntityInfo {
    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        self.packed_xz.serialize(w)?;
        self.y.serialize(w)?;
        self.block_entity_type.serialize(w)?;
        self.tag.serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        self.packed_xz.deserialize(r)?;
        self.y.deserialize(r)?;
        self.block_entity_type.deserialize(r)?;
        self.tag.deserialize(r)
    }
}

/// net.minecraft.network.protocol.game.ClientboundLightUpdatePacketData。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct LightUpdatePacketData {
    // 注意：26.3 的 BIT_SET 编码为 VarInt 长度 + 字节串（BitSet.toByteArray），
    // 与旧版 long 数组不同，因此这里用 ByteArray 承载
    pub sky_y_mask: ByteArray,
    pub block_y_mask: ByteArray,
    pub empty_sky_y_mask: ByteArray,
    pub empty_block_y_mask: ByteArray,
    // TODO: 光照数据层（每项最多 2048 字节的字节数组）
    pub sky_updates: Vec<ByteArray>,
    pub block_updates: Vec<ByteArray>,
}

impl PacketCodec for LightUpdatePacketData {
    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        self.sky_y_mask.serialize(w)?;
        self.block_y_mask.serialize(w)?;
        self.empty_sky_y_mask.serialize(w)?;
        self.empty_block_y_mask.serialize(w)?;
        self.sky_updates.serialize(w)?;
        self.block_updates.serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        self.sky_y_mask.deserialize(r)?;
        self.block_y_mask.deserialize(r)?;
        self.empty_sky_y_mask.deserialize(r)?;
        self.empty_block_y_mask.deserialize(r)?;
        self.sky_updates.deserialize(r)?;
        self.block_updates.deserialize(r)
    }
}

fn write_long_array(w: &mut PacketWriter, values: &[i64]) -> qexed_packet::Result<()> {
    VarInt(values.len() as i32).serialize(w)?;
    for value in values {
        value.serialize(w)?;
    }
    Ok(())
}

fn read_long_array(r: &mut PacketReader) -> qexed_packet::Result<Vec<i64>> {
    let mut len = VarInt::default();
    len.deserialize(r)?;
    if len.0 < 0 {
        return Err(qexed_packet::PacketError::msg(format!(
            "negative long array length: {}",
            len.0
        )));
    }
    let len = len.0 as usize;
    if len > r.buf.remaining() / 8 {
        return Err(qexed_packet::PacketError::msg(format!(
            "long array length {} exceeds remaining {}",
            len,
            r.buf.remaining() / 8
        )));
    }
    let mut values = Vec::with_capacity(len);
    for _ in 0..len {
        let mut value: i64 = 0;
        value.deserialize(r)?;
        values.push(value);
    }
    Ok(values)
}
