use qexed_packet::{PacketCodec, net_types::VarInt};

/// `ServerboundResourcePackPacket` (id 0x06, configuration state).
#[qexed_packet_macros::packet(id = 0x06)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ResourcePack {
    /// UUID of the resource pack push this response refers to.
    pub id: uuid::Uuid,
    // TODO: ServerboundResourcePackPacket.Action enum, VarInt id-mapper on wire:
    //  0 SUCCESSFULLY_LOADED, 1 DECLINED, 2 FAILED_DOWNLOAD, 3 ACCEPTED,
    //  4 DOWNLOADED, 5 INVALID_URL, 6 FAILED_RELOAD, 7 DISCARDED.
    pub action: VarInt,
}
