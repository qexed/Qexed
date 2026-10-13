//! qexed_scoreboard — 记分板数据模型、客户端同步与 /scoreboard 指令声明。
//!
//! 职责：
//! 1. [Scoreboard]：目标（objective）→ 持有者 → 分数 的内存权威状态；
//! 2. [diff]：状态变更前后对比，产出最小客户端同步包序列
//!    （SetObjective / SetScore / ResetScore / SetDisplayObjective）；
//! 3. [command]：/scoreboard 指令树节点（objectives / players 两大子命令），
//!    执行结果以 [ScoreboardEffect] 上报，由接入方代发数据包与回显。
//!
//! 本包纯同步、不含 IO：包发送由宿主（qexed_play）在 drain 副作用时完成。

pub mod command;

use std::collections::BTreeMap;

use qexed_nbt::Tag;
use qexed_packet::net_types::VarInt;
pub use qexed_protocol::to_client::play::reset_score::ResetScore;
pub use qexed_protocol::to_client::play::set_display_objective::SetDisplayObjective;
pub use qexed_protocol::to_client::play::set_objective::SetObjective;
pub use qexed_protocol::to_client::play::set_score::SetScore;

/// 记分板显示槽位（协议 slot 编号）。
pub mod slot {
    pub const LIST: i32 = 0;
    pub const SIDEBAR: i32 = 1;
    pub const BELOW_NAME: i32 = 2;
}

// ---------------------------------------------------------------------------
// 数据模型
// ---------------------------------------------------------------------------

/// 单个记分板目标。
#[derive(Debug, Clone, PartialEq)]
pub struct Objective {
    /// 协议内部名（长度约束由 /scoreboard 指令层校验）。
    pub name: String,
    /// 展示名（NBT 文本组件）。
    pub display_name: Tag,
    /// 判据。当前仅装饰用 dummy；保留字段便于后续扩展。
    pub criteria: String,
}

/// 记分板权威状态：目标表 + 各目标下的持有者分数 + 槽位显示目标。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scoreboard {
    /// 目标名 → 目标。
    objectives: BTreeMap<String, Objective>,
    /// (目标名, 持有者) → 分数。持有者即玩家名（假人/自定义名同样合法）。
    scores: BTreeMap<(String, String), i32>,
    /// 显示槽位 → 目标名。None = 槽位清空。
    displays: [Option<String>; 3],
}

impl Scoreboard {
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加目标。同名存在或名字为空返回 None（调用方转报错回显）。
    pub fn add_objective(&mut self, name: &str, criteria: &str, display_name: Tag) -> Option<()> {
        if name.is_empty() || self.objectives.contains_key(name) {
            return None;
        }
        self.objectives.insert(
            name.to_string(),
            Objective {
                name: name.to_string(),
                display_name,
                criteria: criteria.to_string(),
            },
        );
        Some(())
    }

    /// 修改目标展示名并返回是否命中（/scoreboard objectives modify 用）。
    pub fn modify_display_name(&mut self, name: &str, display_name: Tag) -> bool {
        match self.objectives.get_mut(name) {
            Some(obj) => {
                obj.display_name = display_name;
                true
            }
            None => false,
        }
    }

    /// 删除目标：连带清分数、清显示引用。
    pub fn remove_objective(&mut self, name: &str) {
        self.objectives.remove(name);
        self.scores.retain(|(obj, _), _| obj != name);
        for display in &mut self.displays {
            if display.as_deref() == Some(name) {
                *display = None;
            }
        }
    }

    /// 目标存在性查询。
    pub fn has_objective(&self, name: &str) -> bool {
        self.objectives.contains_key(name)
    }

    /// 目标只读视图（按名字序）。
    pub fn objectives(&self) -> impl Iterator<Item = &Objective> {
        self.objectives.values()
    }

    /// 设置显示槽位指向的目标。目标不存在时不动状态（返回 false）。
    pub fn set_display(&mut self, slot: i32, objective: Option<&str>) -> bool {
        if !(0..=2).contains(&slot) {
            return false;
        }
        if let Some(obj) = objective {
            if !self.objectives.contains_key(obj) {
                return false;
            }
        }
        self.displays[slot as usize] = objective.map(str::to_string);
        true
    }

    /// 槽位当前显示的目标名。
    pub fn display(&self, slot: i32) -> Option<&str> {
        self.displays.get(slot as usize)?.as_deref()
    }

    /// 设置分数（upsert）。目标不存在时返回 None。
    pub fn set_score(&mut self, objective: &str, holder: &str, score: i32) -> Option<()> {
        if !self.objectives.contains_key(objective) {
            return None;
        }
        self.scores
            .insert((objective.to_string(), holder.to_string()), score);
        Some(())
    }

    /// 累加分数（负数即扣减），返回新值。目标不存在时返回 None。
    pub fn add_score(&mut self, objective: &str, holder: &str, delta: i32) -> Option<i32> {
        if !self.objectives.contains_key(objective) {
            return None;
        }
        let entry = self
            .scores
            .entry((objective.to_string(), holder.to_string()))
            .or_insert(0);
        *entry += delta;
        Some(*entry)
    }

    /// 读取分数。
    pub fn get_score(&self, objective: &str, holder: &str) -> Option<i32> {
        self.scores
            .get(&(objective.to_string(), holder.to_string()))
            .copied()
    }

    /// 删除单条分数。
    pub fn reset_score(&mut self, objective: &str, holder: &str) {
        self.scores
            .remove(&(objective.to_string(), holder.to_string()));
    }

    /// 删除持有者在某目标下的全部分数；objective 为 None 时跨全部目标。
    pub fn reset_holder(&mut self, holder: &str, objective: Option<&str>) {
        self.scores
            .retain(|(obj, h), _| !(h == holder && objective.is_none_or(|o| obj == o)));
    }

    /// 某目标下的 (持有者, 分数) 列表（按分数降序、同分按名字序）。
    pub fn entries(&self, objective: &str) -> Vec<(String, i32)> {
        let mut rows: Vec<(String, i32)> = self
            .scores
            .iter()
            .filter(|((obj, _), _)| obj == objective)
            .map(|((_, holder), score)| (holder.clone(), *score))
            .collect();
        rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        rows
    }

    /// 全部出现过的持有者（按名字序）。
    pub fn holders(&self) -> Vec<String> {
        let mut names: Vec<String> = self.scores.keys().map(|(_, h)| h.clone()).collect();
        names.sort();
        names.dedup();
        names
    }

    /// 某持有者在全部目标下的 (目标名, 分数)（按目标名字序）。
    pub fn holder_entries(&self, holder: &str) -> Vec<(String, i32)> {
        self.scores
            .iter()
            .filter(|((_, h), _)| h == holder)
            .map(|((obj, _), score)| (obj.clone(), *score))
            .collect()
    }

    /// 全量客户端同步包序列（进服/重发声明时用）。
    pub fn full_sync(&self) -> Vec<SyncPacket> {
        let mut out = Vec::new();
        for obj in self.objectives.values() {
            out.push(SyncPacket::Objective(SetObjective::create(
                obj.name.clone(),
                obj.display_name.clone(),
            )));
        }
        for ((obj, holder), score) in &self.scores {
            out.push(SyncPacket::Score(SetScore::new(
                holder.clone(),
                obj.clone(),
                *score,
                None,
            )));
        }
        for (slot, obj) in self.displays.iter().enumerate() {
            if let Some(name) = obj {
                out.push(SyncPacket::Display(SetDisplayObjective {
                    slot: VarInt(slot as i32),
                    objective_name: name.clone(),
                }));
            }
        }
        out
    }
}

/// 便捷构造：{text: "..."} 文本组件。
pub fn text_component(text: &str) -> Tag {
    let mut map = std::collections::HashMap::new();
    map.insert(
        "text".to_string(),
        Tag::String(std::sync::Arc::from(text.to_string())),
    );
    Tag::Compound(std::sync::Arc::new(map))
}

// ---------------------------------------------------------------------------
// 同步包与差量
// ---------------------------------------------------------------------------

/// 记分板相关的客户端同步包（newtype 包一层，方便接入方统一发送）。
#[derive(Debug, Clone, PartialEq)]
pub enum SyncPacket {
    Objective(SetObjective),
    Score(SetScore),
    Reset(ResetScore),
    Display(SetDisplayObjective),
}

/// 状态差异：old 到 new 需要下发的最小包序列。
///
/// 语义：
/// * 目标集对比 → create / remove；
/// * 分数对比 → 新增与变更发 SetScore，消失发 ResetScore；
/// * 槽位对比 → 变更发 Display，清空发空 objective_name 的 Display。
pub fn diff(old: &Scoreboard, new: &Scoreboard) -> Vec<SyncPacket> {
    let mut out = Vec::new();

    // 目标：新增
    for obj in new.objectives.values() {
        if !old.has_objective(&obj.name) {
            out.push(SyncPacket::Objective(SetObjective::create(
                obj.name.clone(),
                obj.display_name.clone(),
            )));
        }
    }
    // 目标：删除
    for obj in old.objectives.values() {
        if !new.has_objective(&obj.name) {
            out.push(SyncPacket::Objective(SetObjective::remove(obj.name.clone())));
        }
    }

    // 分数：新增 / 变更
    for ((obj, holder), score) in &new.scores {
        if old.get_score(obj, holder) != Some(*score) {
            out.push(SyncPacket::Score(SetScore::new(
                holder.clone(),
                obj.clone(),
                *score,
                None,
            )));
        }
    }
    // 分数：消失（目标已整体删除的场景由 remove 包覆盖，不重复发）
    for ((obj, holder), _) in &old.scores {
        if !new.has_objective(obj) {
            continue;
        }
        if new.get_score(obj, holder).is_none() {
            out.push(SyncPacket::Reset(ResetScore {
                owner: holder.clone(),
                objective_name: Some(obj.clone()),
            }));
        }
    }

    // 槽位
    for slot in 0..3i32 {
        let before = old.display(slot);
        let after = new.display(slot);
        if before != after {
            out.push(SyncPacket::Display(SetDisplayObjective {
                slot: VarInt(slot),
                objective_name: after.unwrap_or_default().to_string(),
            }));
        }
    }

    out
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_query_objective_lifecycle() {
        let mut sb = Scoreboard::new();
        assert!(sb.add_objective("kills", "dummy", text_component("Kills")).is_some());
        assert!(sb.add_objective("kills", "dummy", text_component("dup")).is_none());
        assert!(sb.has_objective("kills"));
        assert!(!sb.has_objective("nope"));
        assert_eq!(sb.objectives().next().map(|o| o.criteria.as_str()), Some("dummy"));
        sb.remove_objective("kills");
        assert!(!sb.has_objective("kills"));
    }

    #[test]
    fn score_upsert_add_and_entries_ordering() {
        let mut sb = Scoreboard::new();
        sb.add_objective("money", "dummy", text_component("Money")).unwrap();
        assert!(sb.set_score("money", "alice", 10).is_some());
        assert!(sb.set_score("ghost", "alice", 5).is_none()); // 目标不存在
        assert_eq!(sb.get_score("money", "alice"), Some(10));
        assert_eq!(sb.add_score("money", "alice", -3), Some(7));
        sb.set_score("money", "bob", 7);
        // 同分按名字序
        assert_eq!(
            sb.entries("money"),
            vec![("alice".to_string(), 7), ("bob".to_string(), 7)]
        );
        assert!(sb.holder_entries("alice").contains(&("money".to_string(), 7)));
        assert_eq!(sb.holders(), vec!["alice".to_string(), "bob".to_string()]);
    }

    #[test]
    fn remove_objective_cascades_scores_and_displays() {
        let mut sb = Scoreboard::new();
        sb.add_objective("a", "dummy", text_component("A")).unwrap();
        sb.set_score("a", "p", 1).unwrap();
        assert!(sb.set_display(slot::SIDEBAR, Some("a")));
        assert!(!sb.set_display(slot::SIDEBAR, Some("ghost")));
        sb.remove_objective("a");
        assert_eq!(sb.get_score("a", "p"), None);
        assert_eq!(sb.display(slot::SIDEBAR), None);
    }

    #[test]
    fn modify_display_name_updates_objective() {
        let mut sb = Scoreboard::new();
        sb.add_objective("a", "dummy", text_component("A")).unwrap();
        assert!(sb.modify_display_name("a", text_component("Fancy")));
        assert_eq!(sb.objectives().next().map(|o| &o.display_name), Some(&text_component("Fancy")));
        assert!(!sb.modify_display_name("ghost", text_component("X")));
    }

    #[test]
    fn full_sync_emits_objectives_scores_displays() {
        let mut sb = Scoreboard::new();
        sb.add_objective("a", "dummy", text_component("A")).unwrap();
        sb.set_score("a", "p", 3).unwrap();
        sb.set_display(slot::LIST, Some("a"));
        let packets = sb.full_sync();
        assert_eq!(packets.len(), 3);
        assert!(matches!(packets[0], SyncPacket::Objective(_)));
        assert!(matches!(packets[1], SyncPacket::Score(_)));
        assert!(matches!(packets[2], SyncPacket::Display(_)));
    }

    #[test]
    fn diff_minimal_packets_for_each_change_kind() {
        let mut a = Scoreboard::new();
        a.add_objective("a", "dummy", text_component("A")).unwrap();
        a.set_score("a", "p", 1).unwrap();

        // 新目标 + 分数新增/变更
        let mut b = a.clone();
        b.add_objective("b", "dummy", text_component("B")).unwrap();
        b.set_score("a", "p", 2).unwrap();
        b.set_score("a", "q", 9).unwrap();
        let packets = diff(&a, &b);
        assert_eq!(packets.len(), 3);
        assert!(matches!(packets[0], SyncPacket::Objective(_)));

        // 分数删除 → ResetScore
        let mut c = b.clone();
        c.reset_score("a", "q");
        let packets = diff(&b, &c);
        assert_eq!(packets.len(), 1);
        assert!(matches!(&packets[0], SyncPacket::Reset(r) if r.owner == "q"));

        // 目标删除 → remove + 两条 ResetScore（分数不再由 SetScore 覆盖）
        let packets = diff(&b, &a);
        assert_eq!(packets.len(), 3);

        // 槽位显示 → Display 包
        let mut e = a.clone();
        e.set_display(slot::SIDEBAR, Some("a"));
        let packets = diff(&a, &e);
        assert_eq!(packets.len(), 1);
        assert!(matches!(&packets[0], SyncPacket::Display(x) if x.slot.0 == slot::SIDEBAR));

        // 槽位清空 → 空名 Display
        let packets = diff(&e, &a);
        assert_eq!(packets.len(), 1);
        assert!(matches!(&packets[0], SyncPacket::Display(x) if x.objective_name.is_empty()));
    }

    #[test]
    fn reset_holder_scopes() {
        let mut sb = Scoreboard::new();
        sb.add_objective("a", "dummy", text_component("A")).unwrap();
        sb.add_objective("b", "dummy", text_component("B")).unwrap();
        sb.set_score("a", "p", 1).unwrap();
        sb.set_score("b", "p", 2).unwrap();
        sb.set_score("a", "q", 3).unwrap();

        sb.reset_holder("p", Some("a"));
        assert_eq!(sb.get_score("a", "p"), None);
        assert_eq!(sb.get_score("b", "p"), Some(2));

        sb.reset_holder("p", None);
        assert_eq!(sb.get_score("b", "p"), None);
        assert_eq!(sb.get_score("a", "q"), Some(3));
    }
}