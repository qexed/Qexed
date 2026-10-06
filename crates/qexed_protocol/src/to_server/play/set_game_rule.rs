use qexed_packet::{PacketCodec};

/// `ServerboundSetGameRulePacket` (play, to_server, id 0x3A)。
#[qexed_packet_macros::packet(id = 0x3A)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetGameRule {
    pub entries: Vec<GameRuleEntry>,
}

/// `ServerboundSetGameRulePacket$Entry`。
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct GameRuleEntry {
    /// GameRule 注册表键标识符，例如 "minecraft:doDaylightCycle"。
    pub game_rule_key: String,
    pub value: String,
}
