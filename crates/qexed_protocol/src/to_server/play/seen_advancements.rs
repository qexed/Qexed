use qexed_packet::{Packet, PacketCodec, PacketReader, PacketWriter, net_types::VarInt};

/// `ServerboundSeenAdvancementsPacket` (play, to_server, id 0x33)。
///
/// 线上格式是条件字段：action 之后仅当 action == ACTION_OPENED_TAB 才写入 tab 标识符，
/// 因此这里手写 `Packet` 实现而不用宏生成的顺序序列化。
pub const ACTION_OPENED_TAB: i32 = 0;
pub const ACTION_CLOSED_SCREEN: i32 = 1;

#[derive(Debug, Default, PartialEq, Clone)]
pub struct SeenAdvancements {
    // TODO: ServerboundSeenAdvancementsPacket.Action 枚举（OPENED_TAB=0, CLOSED_SCREEN=1）→ VarInt。
    pub action: VarInt,
    /// `net.minecraft.resources.Identifier`；仅当 action == ACTION_OPENED_TAB 时出现在线上。
    pub tab: Option<String>,
}

impl Packet for SeenAdvancements {
    const ID: i32 = 0x33;

    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        self.action.serialize(w)?;
        if self.action.0 == ACTION_OPENED_TAB {
            self.tab.clone().unwrap_or_default().serialize(w)?;
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        self.action.deserialize(r)?;
        self.tab = if self.action.0 == ACTION_OPENED_TAB {
            let mut tab = String::default();
            tab.deserialize(r)?;
            Some(tab)
        } else {
            None
        };
        Ok(())
    }
}
