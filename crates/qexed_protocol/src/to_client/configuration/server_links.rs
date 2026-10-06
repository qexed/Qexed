use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x11)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ServerLinks {
    pub links: Vec<qexed_packet::net_types::ServerLink>,
}
