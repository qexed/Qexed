use qexed_packet::{PacketCodec, net_types::*};

/// `ServerboundChangeGameModePacket` (play, to_server, id 0x05).
#[qexed_packet_macros::packet(id = 0x05)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ChangeGameMode {
    // TODO: net.minecraft.world.level.GameType 枚举（SURVIVAL=0, CREATIVE=1, ADVENTURE=2, SPECTATOR=3），
    //  线上为 GameType.STREAM_CODEC 的 id-mapper VarInt。
    pub mode: VarInt,
}
