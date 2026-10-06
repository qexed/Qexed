use qexed_packet::{PacketCodec, net_types::*};
use crate::types::*;

/// `ServerboundChatCommandSignedPacket` (play, to_server, id 0x08).
#[qexed_packet_macros::packet(id = 0x08)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ChatCommandSigned {
    pub command: String,
    pub time_stamp: i64,
    pub salt: i64,
    pub argument_signatures: Vec<ArgumentSignatureEntry>,
    pub last_seen_messages: LastSeenMessagesUpdate,
}

/// `net.minecraft.commands.arguments.ArgumentSignatures$Entry`（列表上限 8 项）。
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ArgumentSignatureEntry {
    pub name: String,
    pub signature: crate::types::MessageSignature,
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
