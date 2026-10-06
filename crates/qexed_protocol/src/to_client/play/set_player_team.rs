use qexed_packet::{net_types::VarInt, PacketCodec, PacketReader, PacketWriter};
use crate::types::TextComponent;

/// method 字段取值（Java ClientboundSetPlayerTeamPacket 的 METHOD_* 常量）。
pub const METHOD_ADD: i8 = 0;
pub const METHOD_REMOVE: i8 = 1;
pub const METHOD_CHANGE: i8 = 2;
pub const METHOD_JOIN: i8 = 3;
pub const METHOD_LEAVE: i8 = 4;

/// Team.Visibility / Team.CollisionRule 的 idMapper 序号（VarInt）。
pub const VISIBILITY_ALWAYS: i32 = 0;
pub const VISIBILITY_NEVER: i32 = 1;
pub const VISIBILITY_HIDE_FOR_OTHER_TEAMS: i32 = 2;
pub const VISIBILITY_HIDE_FOR_OWN_TEAM: i32 = 3;

pub const COLLISION_ALWAYS: i32 = 0;
pub const COLLISION_NEVER: i32 = 1;
pub const COLLISION_PUSH_OTHER_TEAMS: i32 = 2;
pub const COLLISION_PUSH_OWN_TEAM: i32 = 3;

/// Parameters：依据 server-26.3.jar
/// ClientboundSetPlayerTeamPacket$Parameters.STREAM_CODEC（StreamCodec.composite 七段）：
/// display_name、player_prefix、player_suffix（各 TRUSTED_STREAM_CODEC）→
/// name_tag_visibility(Team$Visibility.STREAM_CODEC，idMapper VarInt 0-3) →
/// collision_rule(Team$CollisionRule.STREAM_CODEC，idMapper VarInt 0-3) →
/// color(TeamColor.STREAM_CODEC 的 optional 包装：bool 存在标记 + idMapper VarInt) →
/// options(ByteBufCodecs.BYTE)。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct TeamParameters {
    pub friendly_flags: i8,
    /// TeamColor：Optional 包裹的 idMapper VarInt（TeamColor 枚举序号 0-15）。
    pub color: Option<VarInt>,
    pub display_name: TextComponent,
    pub player_prefix: TextComponent,
    pub player_suffix: TextComponent,
    /// Team$Visibility idMapper VarInt（0-3）。
    pub name_tag_visibility: VarInt,
    /// Team$CollisionRule idMapper VarInt（0-3）。
    pub collision_rule: VarInt,
}

impl PacketCodec for TeamParameters {
    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        self.display_name.serialize(w)?;
        self.player_prefix.serialize(w)?;
        self.player_suffix.serialize(w)?;
        self.name_tag_visibility.serialize(w)?;
        self.collision_rule.serialize(w)?;
        self.color.serialize(w)?;
        self.friendly_flags.serialize(w)?;
        Ok(())
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        self.display_name.deserialize(r)?;
        self.player_prefix.deserialize(r)?;
        self.player_suffix.deserialize(r)?;
        self.name_tag_visibility.deserialize(r)?;
        self.collision_rule.deserialize(r)?;
        self.color.deserialize(r)?;
        self.friendly_flags.deserialize(r)?;
        Ok(())
    }
}

/// 26.3 ClientboundSetPlayerTeam（minecraft:set_player_team，id 0x6f）。
/// 依据 server-26.3.jar ClientboundSetPlayerTeamPacket.write：
/// name(Utf) + method(Byte)；method ∈ {0=add, 2=change} 时写 parameters（无 presence 标记）；
/// method ∈ {0=add, 3=join, 4=leave} 时写 players（PLAYER_LIST_STREAM_CODEC =
/// STRING_UTF8 的 list()：VarInt 计数前缀 + Utf 列表）。
/// 两个条件字段顺序为 parameters 在前、players 在后——扁平宏模板无法表达，故手工实现 codec。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetPlayerTeam {
    pub name: String,
    pub method: i8,
    pub players: Vec<String>,
    pub parameters: Option<TeamParameters>,
}

impl qexed_packet::Packet for SetPlayerTeam {
    const ID: i32 = 0x6f;

    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        self.name.serialize(w)?;
        self.method.serialize(w)?;
        if self.method == METHOD_ADD || self.method == METHOD_CHANGE {
            match &self.parameters {
                Some(parameters) => parameters.serialize(w)?,
                None => {
                    return Err(qexed_packet::PacketError::msg(format!(
                        "SetPlayerTeam parameters is required when method = {}",
                        self.method
                    )));
                }
            }
        }
        if self.method == METHOD_ADD || self.method == METHOD_JOIN || self.method == METHOD_LEAVE {
            VarInt(self.players.len() as i32).serialize(w)?;
            for player in &self.players {
                player.serialize(w)?;
            }
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        self.name.deserialize(r)?;
        self.method.deserialize(r)?;
        if self.method == METHOD_ADD || self.method == METHOD_CHANGE {
            let mut parameters = TeamParameters::default();
            parameters.deserialize(r)?;
            self.parameters = Some(parameters);
        } else {
            self.parameters = None;
        }
        if self.method == METHOD_ADD || self.method == METHOD_JOIN || self.method == METHOD_LEAVE {
            let mut len = VarInt::default();
            len.deserialize(r)?;
            if len.0 < 0 {
                return Err(qexed_packet::PacketError::msg(format!(
                    "negative SetPlayerTeam players length: {}",
                    len.0
                )));
            }
            self.players.clear();
            self.players.reserve(len.0.min(65536) as usize);
            for _ in 0..len.0 {
                let mut player = String::new();
                player.deserialize(r)?;
                self.players.push(player);
            }
        } else {
            self.players.clear();
        }
        Ok(())
    }
}
