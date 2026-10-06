use qexed_packet::{PacketCodec, net_types::*};

#[qexed_packet_macros::packet(id = 0x65)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetEntityLink {
    // Java 使用 writeInt/readInt（定长 4 字节），非 VarInt
    pub source_id: i32,
    pub dest_id: i32,
}
