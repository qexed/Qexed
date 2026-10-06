use qexed_packet::{PacketCodec, net_types::*};
use crate::types::*;

#[qexed_packet_macros::packet(id = 0x6f)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetPlayerTeam {
    pub name: String,
    pub method: i8,
    // TODO: Java 按 method 条件写入：players 仅在 join(3)/leave(4) 时写入且无数组长度前缀外的额外标记；
    // parameters 仅在 add(0)/change(2) 时写入（无 presence 字节）。扁平宏模板无法表达，需手工 codec
    pub players: Vec<String>,
    pub parameters: Option<TeamParameters>,
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct TeamParameters {
    pub friendly_flags: i8,
    // TODO: net.minecraft.ChatFormatting -> VarInt 占位
    pub color: VarInt,
    pub display_name: TextComponent,
    pub player_prefix: TextComponent,
    pub player_suffix: TextComponent,
    // TODO: Visible/ChatFormatting 枚举 Java 写为 Utf 字符串，暂用 String
    pub name_tag_visibility: String,
    pub collision_rule: String,
}
