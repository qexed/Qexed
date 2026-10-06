/// `ServerboundPunchPacket` (play, to_server, id 0x2E)，无字段。
#[qexed_packet_macros::packet(id = 0x2E)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Punch {}
