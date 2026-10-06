use qexed_packet::{PacketCodec, net_types::VarInt};

/// `ServerboundClientInformationPacket` (id 0x00, configuration state).
///
/// Wraps the `net.minecraft.server.level.ClientInformation` record; field order
/// follows the record's `write(FriendlyByteBuf)`, i.e. the wire order.
#[qexed_packet_macros::packet(id = 0x00)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ClientInformation {
    /// Language tag, e.g. "en_GB" (max 16 chars).
    pub language: String,
    /// Client-side render distance, in chunks (written as a single byte).
    pub view_distance: i8,
    // TODO: net.minecraft.world.entity.player.ChatVisiblity enum (FULL/SYSTEM/HIDDEN), VarInt id-mapper on wire.
    pub chat_visibility: VarInt,
    pub chat_colors: bool,
    /// Displayed skin parts bit mask (written as a single unsigned byte).
    pub model_customisation: u8,
    // TODO: net.minecraft.world.entity.HumanoidArm enum (LEFT/RIGHT), VarInt id-mapper on wire.
    pub main_hand: VarInt,
    pub text_filtering_enabled: bool,
    pub allows_listing: bool,
    // TODO: net.minecraft.server.level.ParticleStatus enum (ALL/DECREASED/MINIMAL), VarInt id-mapper on wire.
    pub particle_status: VarInt,
}
