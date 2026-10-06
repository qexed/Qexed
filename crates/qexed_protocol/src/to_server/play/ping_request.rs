use qexed_packet::{PacketCodec};

/// `ServerboundPingRequestPacket` (play, to_server, id 0x26)。
#[qexed_packet_macros::packet(id = 0x26)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PingRequest {
    pub time: i64,
}
