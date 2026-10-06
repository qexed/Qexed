//! 每玩家统计计数器。

use std::collections::HashMap;
use std::sync::Mutex;

use crate::model::{StatKey, StatValue};

/// 单玩家统计：全量值 + 待推送增量。
#[derive(Debug, Default)]
pub struct StatsCounter {
    values: Mutex<Values>,
}

#[derive(Debug, Default)]
struct Values {
    all: HashMap<String, StatValue>,
    /// 自上次 flush 以来的增量（AwardStats 推送内容）。
    pending: HashMap<String, StatValue>,
}

impl StatsCounter {
    pub fn new() -> Self {
        Self::default()
    }

    /// 累加（value 允许负——原版 distance 类不会为负，但 API 不设限）。
    pub fn add(&self, key: StatKey, delta: StatValue) -> StatValue {
        let mut values = self.values.lock().expect("stats poisoned");
        let name = key.full_name();
        let updated = {
            let current = values.all.entry(name.clone()).or_insert(0);
            *current += delta;
            *current
        };
        {
            let pending = values.pending.entry(name).or_insert(0);
            *pending += delta;
        }
        updated
    }

    /// 递增 1（最常见路径：挖一格/杀一只）。
    pub fn increment(&self, key: StatKey) -> StatValue {
        self.add(key, 1)
    }

    /// 直接设置（存档恢复用；不改 pending）。
    pub fn set_raw(&self, name: String, value: StatValue) {
        let mut values = self.values.lock().expect("stats poisoned");
        values.all.insert(name, value);
    }

    pub fn get(&self, key: &StatKey) -> StatValue {
        let values = self.values.lock().expect("stats poisoned");
        values.all.get(&key.full_name()).copied().unwrap_or(0)
    }

    /// 全量快照（登录推送 / 存档保存）。
    pub fn snapshot(&self) -> Vec<(String, StatValue)> {
        let values = self.values.lock().expect("stats poisoned");
        let mut items: Vec<_> = values.all.iter().map(|(k, v)| (k.clone(), *v)).collect();
        items.sort();
        items
    }

    /// 取走待推送增量（AwardStats 变更推送）。
    pub fn take_pending(&self) -> Vec<(String, StatValue)> {
        let mut values = self.values.lock().expect("stats poisoned");
        let pending = std::mem::take(&mut values.pending);
        let mut items: Vec<_> = pending.into_iter().collect();
        items.sort();
        items
    }

    /// 是否有未推送变更。
    pub fn has_pending(&self) -> bool {
        let values = self.values.lock().expect("stats poisoned");
        !values.pending.is_empty()
    }
}

/// 多玩家计数器集。
#[derive(Debug, Default)]
pub struct StatsRegistry {
    counters: Mutex<HashMap<uuid::Uuid, std::sync::Arc<StatsCounter>>>,
}

impl StatsRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 取玩家计数器（无则建）。
    pub fn counter(&self, player: uuid::Uuid) -> std::sync::Arc<StatsCounter> {
        let mut counters = self.counters.lock().expect("stats registry poisoned");
        counters.entry(player).or_insert_with(|| std::sync::Arc::new(StatsCounter::new())).clone()
    }

    /// 移除玩家（退出时）。
    pub fn remove(&self, player: uuid::Uuid) {
        self.counters.lock().expect("stats registry poisoned").remove(&player);
    }
}

impl StatsCounter {
    /// 共享句柄（Registry / 会话注入用）。
    pub fn shared() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self::new())
    }
}
