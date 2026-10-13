//! PLAY 会话状态：单个玩家的游戏内数据与统计。

use qexed_packet::net_types::{GameProfile, VarInt};

/// 玩家在 PLAY 阶段的会话状态。
#[derive(Debug, Clone)]
pub struct PlayPlayer {
    /// 登录后解析出的档案（在线/离线模式均由 qexed_auth 产出）。
    pub profile: GameProfile,
    /// Configuration 阶段收集到的客户端信息。
    pub config: PlayContextConfig,
    /// 实体 id：当前单实体实现恒为 0，多实体后由 qexed_entities 分配。
    pub entity_id: i32,
    /// 游戏内位置（世界坐标）。
    pub position: (f64, f64, f64),
    /// 视角（度）。
    pub rotation: (f32, f32),
    /// 游戏模式：0 生存 / 1 创造 / 2 冒险 / 3 旁观。
    pub game_mode: u8,
    /// 玩家是否已在客户端完成世界加载（收到 PlayerLoaded）。
    pub loaded: bool,
}

/// 从 Configuration 阶段带过来的上下文（服务端配置 + 客户端声明）。
#[derive(Debug, Clone)]
pub struct PlayContextConfig {
    pub view_distance: i32,
    pub simulation_distance: i32,
    pub dimension_name: String,
    pub dimension_type: i32,
    pub hashed_seed: i64,
    pub online_mode: bool,
}

impl PlayPlayer {
    pub fn new(profile: GameProfile, config: PlayContextConfig, game_mode: u8) -> Self {
        Self {
            profile,
            config,
            entity_id: 0,
            position: (0.0, 64.0, 0.0),
            rotation: (0.0, 0.0),
            game_mode,
            loaded: false,
        }
    }

    /// 更新位置/视角；`None` 表示该包不携带对应数据。
    pub fn update_position(
        &mut self,
        position: Option<(f64, f64, f64)>,
        rotation: Option<(f32, f32)>,
    ) {
        if let Some(pos) = position {
            self.position = pos;
        }
        if let Some(rot) = rotation {
            self.rotation = rot;
        }
    }

    /// 当前所在区块坐标（`div_euclid(16)`，跨负数边界正确）。
    pub fn chunk_pos(&self) -> (i32, i32) {
        (
            (self.position.0 as i32).div_euclid(16),
            (self.position.2 as i32).div_euclid(16),
        )
    }

    /// 心跳超时等场景下给客户端的展示名。
    pub fn display_name(&self) -> &str {
        &self.profile.username
    }
}

/// 一次 PLAY 会话结束时的统计。
#[derive(Debug, Default, Clone)]
pub struct PlaySessionStats {
    pub packets_received: u64,
    pub chat_messages: u64,
    /// 本次会话成功执行的指令数量。
    pub commands_executed: u64,
    /// 本次会话流送给客户端的区块数量。
    pub chunks_sent: u64,
    /// 会话持续秒数（由驱动循环在结束时填写）。
    pub duration_secs: u64,
}

impl PlaySessionStats {
    /// 供测试与日志使用的快捷构造。
    pub fn new(packets_received: u64, chat_messages: u64) -> Self {
        Self {
            packets_received,
            chat_messages,
            commands_executed: 0,
            chunks_sent: 0,
            duration_secs: 0,
        }
    }
}

/// 通用的“ VarInt 长度”工具：服务器后续广播玩家列表时需要。
pub fn varint_len(value: VarInt) -> usize {
    let mut n = value.0 as u32; // 编码按 u32 二补码（-1 → 5 字节）
    let mut len = 1;
    while n >= 0x80 {
        n >>= 7;
        len += 1;
    }
    len
}

/// 增量区块流送的会话状态：与客户端已发/已卸集合严格同步。
#[derive(Debug, Clone)]
pub struct ChunkStreamingState {
    /// 已发给客户端、尚未卸载的区块坐标。
    pub sent: std::collections::HashSet<(i32, i32)>,
    /// 当前视野圈（中心 + 半径）。
    pub circle: qexed_world::stream::ViewCircle,
}

impl ChunkStreamingState {
    /// 用出生流送覆盖的初始视野圈初始化（与 SpawnChunkStreamer 计划一致）。
    pub fn new(center: (i32, i32), radius: i32) -> Self {
        let circle = qexed_world::stream::ViewCircle::new(center, radius);
        let streamer = qexed_world::stream::SpawnChunkStreamer::new(center, radius);
        let sent = (0..streamer.total_chunks())
            .map(|n| streamer.plan_public(n))
            .collect();
        Self { sent, circle }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use qexed_packet::net_types::ProfileProperty;

    fn profile(name: &str) -> GameProfile {
        GameProfile {
            uuid: uuid::Uuid::nil(),
            username: name.to_string(),
            properties: vec![ProfileProperty {
                name: "textures".to_string(),
                value: "abc".to_string(),
                signature: None,
            }],
        }
    }

    fn ctx() -> PlayContextConfig {
        PlayContextConfig {
            view_distance: 8,
            simulation_distance: 8,
            dimension_name: "minecraft:overworld".to_string(),
            dimension_type: 0,
            hashed_seed: 0,
            online_mode: true,
        }
    }

    #[test]
    fn new_player_defaults() {
        let p = PlayPlayer::new(profile("steve"), ctx(), 1);
        assert_eq!(p.game_mode, 1);
        assert_eq!(p.entity_id, 0);
        assert!(!p.loaded);
        assert_eq!(p.position, (0.0, 64.0, 0.0));
        assert_eq!(p.display_name(), "steve");
    }

    #[test]
    fn chunk_pos_across_negative_boundary() {
        let mut p = PlayPlayer::new(profile("a"), ctx(), 1);
        p.update_position(Some((-1.0, 64.0, -1.0)), None);
        assert_eq!(p.chunk_pos(), (-1, -1));
        p.update_position(Some((-16.0, 64.0, -15.5)), None);
        assert_eq!(p.chunk_pos(), (-1, -1));
        p.update_position(Some((-17.0, 64.0, 15.9)), None);
        assert_eq!(p.chunk_pos(), (-2, 0));
        p.update_position(Some((15.9, 64.0, 16.0)), None);
        assert_eq!(p.chunk_pos(), (0, 1));
    }

    #[test]
    fn update_position_partial_fields() {
        let mut p = PlayPlayer::new(profile("a"), ctx(), 1);
        p.update_position(Some((1.0, 2.0, 3.0)), Some((90.0, -45.0)));
        // 仅旋转的包不改位置
        p.update_position(None, Some((0.0, 0.0)));
        assert_eq!(p.position, (1.0, 2.0, 3.0));
        assert_eq!(p.rotation, (0.0, 0.0));
        // 仅位置的包不改旋转
        p.update_position(Some((4.0, 5.0, 6.0)), None);
        assert_eq!(p.rotation, (0.0, 0.0));
        assert_eq!(p.position, (4.0, 5.0, 6.0));
    }

    #[test]
    fn stats_default_and_constructor() {
        let s = PlaySessionStats::default();
        assert_eq!(
            (s.packets_received, s.chat_messages, s.duration_secs),
            (0, 0, 0)
        );
        let s = PlaySessionStats::new(10, 3);
        assert_eq!((s.packets_received, s.chat_messages), (10, 3));
    }

    #[test]
    fn varint_len_matches_encoding() {
        assert_eq!(varint_len(VarInt(0)), 1);
        assert_eq!(varint_len(VarInt(127)), 1);
        assert_eq!(varint_len(VarInt(128)), 2);
        assert_eq!(varint_len(VarInt(-1)), 5);
    }
}