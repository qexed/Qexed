use qexed_packet::{
    PacketCodec,
    net_types::{CustomQueryPayload, VarInt},
};

#[qexed_packet_macros::packet(id = 0x02)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct CustomQueryAnswer {
    pub transaction_id: VarInt,
    // TODO: net.minecraft.network.protocol.login.custom.CustomQueryAnswerPayload 是复杂类型；
    // 线上格式为 present 布尔 + 通道标识 + 剩余字节，暂用 Option<CustomQueryPayload> 占位
    pub payload: Option<CustomQueryPayload>,
}
