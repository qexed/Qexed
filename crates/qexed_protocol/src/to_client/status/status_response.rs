use qexed_packet::{PacketCodec, net_types::JsonValue};

#[qexed_packet_macros::packet(id = 0x0)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct StatusResponse {
    // TODO: net.minecraft.network.protocol.status.ServerStatus — 线上格式为一段 JSON 文本，
    // 此处用 JsonValue 占位；如需强类型可后续展开为 version/players/description/favicon 等字段。
    pub status: JsonValue,
}
