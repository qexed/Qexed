use bytes::{Buf as _, BufMut as _};
use qexed_packet::{PacketCodec, PacketReader, PacketWriter, net_types::*};

/// `ServerboundSetCommandBlockPacket` (play, to_server, id 0x37)。
#[qexed_packet_macros::packet(id = 0x37)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetCommandBlock {
    pub pos: Position,
    pub command: String,
    // TODO: CommandBlockEntity.Mode 枚举（SEQUENCE=0, AUTO=1, REDSTONE=2）→ VarInt。
    pub mode: VarInt,
    pub flags: CommandBlockFlags,
}

/// trackOutput/conditional/automatic 三个布尔在线上打包为单个字节（位 0/1/2）。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct CommandBlockFlags {
    pub track_output: bool,
    pub conditional: bool,
    pub automatic: bool,
}

impl PacketCodec for CommandBlockFlags {
    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        let mut bits: u8 = 0;
        if self.track_output {
            bits |= 0b0000_0001;
        }
        if self.conditional {
            bits |= 0b0000_0010;
        }
        if self.automatic {
            bits |= 0b0000_0100;
        }
        w.buf.put_u8(bits);
        Ok(())
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        let bits = r.buf.get_u8();
        self.track_output = bits & 0b0000_0001 != 0;
        self.conditional = bits & 0b0000_0010 != 0;
        self.automatic = bits & 0b0000_0100 != 0;
        Ok(())
    }
}
