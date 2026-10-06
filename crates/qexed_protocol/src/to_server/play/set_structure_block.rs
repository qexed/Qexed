use bytes::{Buf as _, BufMut as _};
use qexed_packet::{PacketCodec, PacketReader, PacketWriter, net_types::*};

/// `ServerboundSetStructureBlockPacket` (play, to_server, id 0x3C)。
///
/// 字段顺序即 `write(FriendlyByteBuf)` 的线上顺序：
/// pos, updateType, mode, name, offset(3 字节), size(3 字节), mirror, rotation,
/// data, integrity(f32), seed(VarLong), flags(单字节位打包)。
#[qexed_packet_macros::packet(id = 0x3C)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetStructureBlock {
    pub pos: Position,
    // TODO: StructureBlockEntity.UpdateType 枚举（UPDATE_DATA=0, SAVE_AREA=1, LOAD_AREA=2, SCAN_AREA=3）→ VarInt。
    pub update_type: VarInt,
    // TODO: StructureMode 枚举（SAVE=0, LOAD=1, CORNER=2, DATA=3）→ VarInt。
    pub mode: VarInt,
    pub name: String,
    pub offset: Vec3Bytes,
    pub size: Vec3Bytes,
    // TODO: Mirror 枚举（NONE=0, LEFT_RIGHT=1, FRONT_BACK=2）→ VarInt。
    pub mirror: VarInt,
    // TODO: Rotation 枚举（NONE=0, CLOCKWISE_90=1, CLOCKWISE_180=2, COUNTERCLOCKWISE_90=3）→ VarInt。
    pub rotation: VarInt,
    pub data: String,
    pub integrity: f32,
    pub seed: VarLong,
    pub flags: StructureBlockFlags,
}

/// `net.minecraft.core.Vec3i`（offset/size）在线上是三个有符号字节（x/y/z）。
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Vec3Bytes {
    pub x: i8,
    pub y: i8,
    pub z: i8,
}

/// ignoreEntities/showAir/showBoundingBox/strict 四个布尔在线上打包为单个字节（位 0/1/2/3）。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct StructureBlockFlags {
    pub ignore_entities: bool,
    pub show_air: bool,
    pub show_bounding_box: bool,
    pub strict: bool,
}

impl PacketCodec for StructureBlockFlags {
    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        let mut bits: u8 = 0;
        if self.ignore_entities {
            bits |= 0b0000_0001;
        }
        if self.show_air {
            bits |= 0b0000_0010;
        }
        if self.show_bounding_box {
            bits |= 0b0000_0100;
        }
        if self.strict {
            bits |= 0b0000_1000;
        }
        w.buf.put_u8(bits);
        Ok(())
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        let bits = r.buf.get_u8();
        self.ignore_entities = bits & 0b0000_0001 != 0;
        self.show_air = bits & 0b0000_0010 != 0;
        self.show_bounding_box = bits & 0b0000_0100 != 0;
        self.strict = bits & 0b0000_1000 != 0;
        Ok(())
    }
}
