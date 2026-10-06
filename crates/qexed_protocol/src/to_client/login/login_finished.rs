use qexed_packet::{PacketCodec, net_types::GameProfile};

#[qexed_packet_macros::packet(id = 0x02)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct LoginFinished {
    // Java: com.mojang.authlib.GameProfile -> 共享类型 GameProfile
    pub game_profile: GameProfile,
    pub session_id: uuid::Uuid,
}
