/// `ServerboundPlayerLoadedPacket` (play, to_server, id 0x2C)，无字段。
#[qexed_packet_macros::packet(id = 0x2C)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PlayerLoaded {}
