use qexed_packet::{
    PacketCodec,
    net_types::{VarInt, VarLong},
};

#[qexed_packet_macros::packet(id = 0x71)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetTime {
    pub game_time: i64,
    /// 26.1+：每个 clock 注册表条目的当前时间状态。
    /// 条目 id 对应 configuration 阶段同步的 `minecraft:world_clock` 注册表序号
    /// （按 entry id 字母序：0 = overworld，1 = the_end）。
    pub clock_updates: Vec<ClockUpdate>,
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ClockUpdate {
    /// world_clock 注册表条目 id。
    pub clock: VarInt,
    /// 该 clock 累计走过的总 tick 数（VarLong）。
    pub total_ticks: VarLong,
    /// 小数 tick（用于插值渲染）。
    pub partial_tick: f32,
    /// 流速倍率：1.0 正常，0.0 暂停。
    pub rate: f32,
}
