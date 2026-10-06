use qexed_packet::{Packet, PacketCodec};

#[qexed_packet_macros::packet(id = 0x27)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct GameRuleValues {
    // TODO: Map<ResourceKey<GameRule<?>>, String> — 键为 Identifier 字符串，展开为条目列表
    pub values: Vec<GameRuleValue>,
}

impl GameRuleValues {
    pub const MAX_ENTRIES: usize = 32;

    /// 带条目数上限校验的序列化（Map 最多 32 项）。
    pub fn serialize_bounded(&self, w: &mut qexed_packet::PacketWriter) -> qexed_packet::Result<()> {
        if self.values.len() > Self::MAX_ENTRIES {
            return Err(qexed_packet::PacketError::msg(format!(
                "game rule entry count {} exceeds max {}",
                self.values.len(),
                Self::MAX_ENTRIES
            )));
        }
        self.serialize(w)
    }
}

/// 单条游戏规则值：规则 Identifier + 序列化后的值。
#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct GameRuleValue {
    // TODO: ResourceKey<GameRule<?>> — 线上格式为 Identifier 字符串
    pub rule: String,
    pub value: String,
}

impl PacketCodec for GameRuleValue {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> qexed_packet::Result<()> {
        self.rule.serialize(w)?;
        self.value.serialize(w)
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> qexed_packet::Result<()> {
        self.rule.deserialize(r)?;
        self.value.deserialize(r)
    }
}
