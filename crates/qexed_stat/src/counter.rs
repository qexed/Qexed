//! 每玩家统计计数器：累计 → dirty 集 → award_stats 包。
//!
//! 语义对齐原版 `ServerStatsCounter`：值存绝对值，发的是「dirty 集的当前值」，
//! 客户端按绝对值覆盖（`handleAwardStats` → `setValue`）。
use std::collections::HashMap;

use qexed_protocol::to_client::play::award_stats::AwardStats;

use crate::registry::{IdMaps, StatKey};

#[derive(Debug, Default)]
pub struct StatCounter {
    values: HashMap<StatKey, i64>,
    dirty: Vec<StatKey>,
}

impl StatCounter {
    pub fn new() -> Self {
        Self::default()
    }

    /// 累计一项统计。
    pub fn increment(&mut self, key: StatKey, amount: i64) {
        let value = self.values.entry(key.clone()).or_insert(0);
        *value = value.saturating_add(amount.max(0));
        if !self.dirty.contains(&key) {
            self.dirty.push(key);
        }
    }

    /// 便捷入口：custom 统计 +1（最常见用法）。
    pub fn increment_custom(&mut self, entry_name: &str) {
        self.increment(StatKey::custom(entry_name), 1);
    }

    /// 读取当前值（未累计过为 0）。
    pub fn get(&self, key: &StatKey) -> i64 {
        self.values.get(key).copied().unwrap_or(0)
    }

    /// dirty 集 → award_stats 包（借 IdMaps 解析 id）；空集返回 None 不发包。
    /// 成功取走后清空 dirty（对应原版 `getDirty` 的 take 语义）。
    pub fn take_award_stats(&mut self, maps: &IdMaps) -> Option<AwardStats> {
        if self.dirty.is_empty() {
            return None;
        }
        let mut stats = Vec::with_capacity(self.dirty.len());
        for key in self.dirty.drain(..) {
            let value = self.values.get(&key).copied().unwrap_or(0);
            if let Some(award) = maps.award_stat(&key, value) {
                stats.push(award);
            }
        }
        (!stats.is_empty()).then_some(AwardStats { stats })
    }

    /// 当前 dirty 数量（测试/诊断用）。
    pub fn dirty_len(&self) -> usize {
        self.dirty.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{StatCategory, StatKey};

    fn custom_key(name: &str) -> StatKey {
        StatKey {
            category: StatCategory::Custom,
            entry: format!("minecraft:{name}"),
        }
    }

    #[test]
    fn increment_accumulates_and_dedups_dirty() {
        let mut counter = StatCounter::new();
        let key = custom_key("leave_game");
        counter.increment(key.clone(), 1);
        counter.increment(key.clone(), 1);
        counter.increment_custom("play_time");
        assert_eq!(counter.get(&key), 2);
        assert_eq!(counter.dirty_len(), 2);
    }

    #[test]
    fn take_award_stats_clears_dirty() {
        let mut counter = StatCounter::new();
        counter.increment_custom("leave_game");
        assert_eq!(counter.dirty_len(), 1);
        counter.take_award_stats(&IdMaps::default());
        assert_eq!(counter.dirty_len(), 0);
        // 值保留，但已不 dirty：再次取包为空
        let key = custom_key("leave_game");
        assert_eq!(counter.get(&key), 1);
        assert!(counter.take_award_stats(&IdMaps::default()).is_none());
    }

    #[test]
    fn empty_dirty_yields_no_packet() {
        let mut counter = StatCounter::new();
        assert!(counter.take_award_stats(&IdMaps::default()).is_none());
    }
}
