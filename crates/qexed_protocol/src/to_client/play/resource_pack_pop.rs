use qexed_packet::{PacketCodec};

#[qexed_packet_macros::packet(id = 0x51)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ResourcePackPop {
    pub id: Option<uuid::Uuid>,
}
