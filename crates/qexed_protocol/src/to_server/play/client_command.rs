use qexed_packet::{PacketCodec, net_types::VarInt};

pub const PERFORM_RESPAWN: i32 = 0;
pub const REQUEST_STATS: i32 = 1;
pub const REQUEST_GAMERULE_VALUES: i32 = 2;

#[qexed_packet_macros::packet(id = 0x0C)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ClientCommand {
    pub action: VarInt,
}

impl ClientCommand {
    pub fn perform_respawn() -> Self {
        Self {
            action: VarInt(PERFORM_RESPAWN),
        }
    }
}
