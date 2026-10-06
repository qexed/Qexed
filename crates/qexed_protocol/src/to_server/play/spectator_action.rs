use qexed_packet::{PacketCodec, net_types::*};

/// `ServerboundSpectatorActionPacket` (play, to_server, id 0x3F)。
#[qexed_packet_macros::packet(id = 0x3F)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SpectatorAction {
    /// `java.util.OptionalInt`：OPTIONAL_VAR_INT（0 = 无值，否则实体 id + 1）。
    pub spectate_entity_id: OptionalVarInt,
}
