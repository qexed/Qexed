use qexed_packet::{PacketCodec};

/// `ServerboundRenameItemPacket` (play, to_server, id 0x31)。
#[qexed_packet_macros::packet(id = 0x31)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct RenameItem {
    pub name: String,
}
