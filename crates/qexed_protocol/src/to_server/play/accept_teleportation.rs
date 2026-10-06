use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x00)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct AcceptTeleportation {
    pub teleport_id: VarInt,
}
