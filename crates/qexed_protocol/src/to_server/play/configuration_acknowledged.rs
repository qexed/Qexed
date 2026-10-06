/// `ServerboundConfigurationAcknowledgedPacket` (play, to_server, id 0x10)，无字段。
#[qexed_packet_macros::packet(id = 0x10)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ConfigurationAcknowledged {}
