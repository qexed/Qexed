use qexed_packet::{
    PacketCodec,
    net_types::{CustomQueryPayload, VarInt},
};

#[qexed_packet_macros::packet(id = 0x04)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct CustomQuery {
    pub transaction_id: VarInt,
    // Java: net.minecraft.network.protocol.login.custom.CustomQueryPayload -> 共享类型 CustomQueryPayload
    pub payload: CustomQueryPayload,
}
