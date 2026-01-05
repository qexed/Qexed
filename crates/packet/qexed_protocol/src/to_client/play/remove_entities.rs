
use qexed_packet::{PacketCodec, net_types::VarInt};
#[qexed_packet_macros::packet(id = 0x46)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct RemoveEntities {
    pub uuids:Vec<VarInt>
}
