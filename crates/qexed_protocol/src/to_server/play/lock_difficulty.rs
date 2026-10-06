use qexed_packet::{PacketCodec};

/// `ServerboundLockDifficultyPacket` (play, to_server, id 0x1D)。
#[qexed_packet_macros::packet(id = 0x1D)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct LockDifficulty {
    pub locked: bool,
}
