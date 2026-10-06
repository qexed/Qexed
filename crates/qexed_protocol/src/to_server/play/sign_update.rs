use qexed_packet::{PacketCodec, PacketReader, PacketWriter, net_types::*};

/// `ServerboundSignUpdatePacket` (play, to_server, id 0x3E)。
#[qexed_packet_macros::packet(id = 0x3E)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SignUpdate {
    pub pos: Position,
    pub lines: SignLines,
    // TODO: SignTextSlot 枚举（BACK=0, FRONT=1）→ VarInt。
    pub slot: VarInt,
}

/// 四行告示牌文本：fixedSizeList(4)，线上不写长度前缀，每项为 stringUtf8(384)。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SignLines(pub [String; 4]);

impl PacketCodec for SignLines {
    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        for line in &self.0 {
            line.serialize(w)?;
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        for line in &mut self.0 {
            line.deserialize(r)?;
        }
        Ok(())
    }
}
