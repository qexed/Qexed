/// `ServerboundClientTickEndPacket` (play, to_server, id 0x0D)，无字段。
#[qexed_packet_macros::packet(id = 0x0D)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ClientTickEnd {}
