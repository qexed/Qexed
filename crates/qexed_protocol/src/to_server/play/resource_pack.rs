use qexed_packet::{PacketCodec, net_types::*};

/// `ServerboundResourcePackPacket` (play, to_server, id 0x32)，
/// 与 to_server/configuration/resource_pack.rs（id 0x06）同一负载。
#[qexed_packet_macros::packet(id = 0x32)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ResourcePack {
    pub id: uuid::Uuid,
    // TODO: ServerboundResourcePackPacket.Action 枚举，VarInt id-mapper：
    //  0 SUCCESSFULLY_LOADED, 1 DECLINED, 2 FAILED_DOWNLOAD, 3 ACCEPTED,
    //  4 DOWNLOADED, 5 INVALID_URL, 6 FAILED_RELOAD, 7 DISCARDED。
    pub action: VarInt,
}
