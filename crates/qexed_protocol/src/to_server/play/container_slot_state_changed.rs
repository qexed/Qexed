use qexed_packet::{PacketCodec, net_types::*};

/// `ServerboundContainerSlotStateChangedPacket` (play, to_server, id 0x14)。
#[qexed_packet_macros::packet(id = 0x14)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ContainerSlotStateChanged {
    pub slot_id: VarInt,
    /// ByteBufCodecs.CONTAINER_ID，线上同样是 VarInt。
    pub container_id: VarInt,
    pub new_state: bool,
}
