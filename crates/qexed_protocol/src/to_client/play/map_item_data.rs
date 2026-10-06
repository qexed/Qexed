use qexed_packet::{PacketCodec, net_types::*};
use crate::types::*;

/// net.minecraft.world.level.saveddata.maps.MapDecoration。
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MapDecoration {
    // TODO: net.minecraft.core.Holder<net.minecraft.world.level.saveddata.maps.MapDecorationType> 注册表 id，用 VarInt 占位
    pub decoration_type: VarInt,
    pub x: i8,
    pub y: i8,
    pub rot: i8,
    pub name: Option<crate::types::TextComponent>,
}

/// net.minecraft.world.level.saveddata.maps.MapItemSavedData$MapPatch。
/// 线格式：width/height/startX/startY 各 1 字节，后跟 ByteArray 颜色数据。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MapPatch {
    pub width: u8,
    pub height: u8,
    pub start_x: u8,
    pub start_y: u8,
    pub map_colors: ByteArray,
}

impl PacketCodec for MapPatch {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> qexed_packet::Result<()> {
        self.width.serialize(w)?;
        self.height.serialize(w)?;
        self.start_x.serialize(w)?;
        self.start_y.serialize(w)?;
        self.map_colors.serialize(w)
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> qexed_packet::Result<()> {
        self.width.deserialize(r)?;
        self.height.deserialize(r)?;
        self.start_x.deserialize(r)?;
        self.start_y.deserialize(r)?;
        self.map_colors.deserialize(r)
    }
}

#[qexed_packet_macros::packet(id = 0x33)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MapItemData {
    pub map_id: VarInt,
    pub scale: i8,
    pub locked: bool,
    pub decorations: Option<Vec<MapDecoration>>,
    /// present => width>0 的补丁；absent => 线上单字节 0。
    pub color_patch: Option<MapPatch>,
}
