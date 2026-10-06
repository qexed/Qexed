use qexed_packet::{PacketCodec, net_types::*};

/// `ServerboundDebugSubscriptionRequestPacket` (play, to_server, id 0x17)。
#[qexed_packet_macros::packet(id = 0x17)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct DebugSubscriptionRequest {
    // TODO: Set<DebugSubscription<?>> —— DEBUG_SUBSCRIPTION 注册表 holder id（VarInt）集合，
    //  线上为 长度 VarInt + 各 id VarInt（集合上限 32），此处用 Vec<VarInt> 占位。
    pub subscriptions: Vec<VarInt>,
}
