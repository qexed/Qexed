use qexed_packet::{PacketCodec, net_types::VarInt};

pub const DISPLAY_SLOT_LIST: i32 = 0;
pub const DISPLAY_SLOT_SIDEBAR: i32 = 1;
pub const DISPLAY_SLOT_BELOW_NAME: i32 = 2;

#[qexed_packet_macros::packet(id = 0x64)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetDisplayObjective {
    pub slot: VarInt,
    pub objective_name: String,
}

impl SetDisplayObjective {
    pub fn sidebar(objective_name: impl Into<String>) -> Self {
        Self {
            slot: VarInt(DISPLAY_SLOT_SIDEBAR),
            objective_name: objective_name.into(),
        }
    }

    pub fn clear_sidebar() -> Self {
        Self {
            slot: VarInt(DISPLAY_SLOT_SIDEBAR),
            objective_name: String::new(),
        }
    }
}
