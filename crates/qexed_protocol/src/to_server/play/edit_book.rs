use qexed_packet::{PacketCodec, net_types::*};

/// `ServerboundEditBookPacket` (play, to_server, id 0x18)。
#[qexed_packet_macros::packet(id = 0x18)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct EditBook {
    pub slot: VarInt,
    /// 页面文本列表（上限 100 项，每项最长 1024）。
    pub pages: Vec<String>,
    pub title: Option<String>,
}
