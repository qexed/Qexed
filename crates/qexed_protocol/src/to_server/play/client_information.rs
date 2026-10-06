use qexed_packet::{PacketCodec, net_types::*};

/// `ServerboundClientInformationPacket` (play, to_server, id 0x0E)。
///
/// 展开自 `net.minecraft.server.level.ClientInformation` 记录，字段顺序即线上顺序，
/// 与 to_server/configuration/client_information.rs（id 0x00）同一负载。
#[qexed_packet_macros::packet(id = 0x0E)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ClientInformation {
    /// 语言标签，例如 "zh_cn"。
    pub language: String,
    /// 视距（区块），单字节。
    pub view_distance: i8,
    // TODO: ChatVisibility 枚举（FULL/SYSTEM/HIDDEN），线上为 VarInt id。
    pub chat_visibility: VarInt,
    pub chat_colors: bool,
    /// 皮肤部件位掩码，单字节。
    pub model_customisation: u8,
    // TODO: HumanoidArm 枚举（LEFT/RIGHT），线上为 VarInt id。
    pub main_hand: VarInt,
    pub text_filtering_enabled: bool,
    pub allows_listing: bool,
    // TODO: ParticleStatus 枚举（ALL/DECREASED/MINIMAL），线上为 VarInt id。
    pub particle_status: VarInt,
}
