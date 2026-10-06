use qexed_packet::{PacketCodec, net_types::*};

/// `ServerboundSetBeaconPacket` (play, to_server, id 0x35)。
#[qexed_packet_macros::packet(id = 0x35)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetBeacon {
    // TODO: Optional<Holder<MobEffect>> —— MOB_EFFECT 注册表 holder id，线上为
    //  optional 前缀 bool + VarInt，此处用 Option<VarInt> 占位。
    pub primary: Option<VarInt>,
    // TODO: Optional<Holder<MobEffect>>，同 primary。
    pub secondary: Option<VarInt>,
}
