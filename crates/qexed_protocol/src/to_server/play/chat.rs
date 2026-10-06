use qexed_packet::{PacketCodec, net_types::*};
use crate::types::*;

/// `ServerboundChatPacket` (play, to_server, id 0x09)。
#[qexed_packet_macros::packet(id = 0x09)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Chat {
    pub message: String,
    pub time_stamp: i64,
    pub salt: i64,
    pub signature: Option<crate::types::MessageSignature>,
    pub last_seen_messages: LastSeenMessagesUpdate,
}

/// `net.minecraft.network.chat.LastSeenMessages$Update`：
/// offset(VarInt) + acknowledged(fixedBitSet(20) = 3 个原始字节) + checksum(byte)。
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct LastSeenMessagesUpdate {
    pub offset: VarInt,
    pub acknowledged: [u8; 3],
    pub checksum: u8,
}
