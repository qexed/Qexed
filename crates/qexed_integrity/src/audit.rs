//! 周期对账器（影子校验 + 业务不变量 + 差异上报）。

use std::collections::VecDeque;

use crate::error::ViolationKind;

/// 单次审计事件。
#[derive(Debug, Clone)]
pub struct AuditEvent {
    pub kind: ViolationKind,
    pub label: String,
    pub detail: String,
}

/// 审计报告（一个周期内的事件集合）。
#[derive(Debug, Clone, Default)]
pub struct AuditReport {
    pub events: Vec<AuditEvent>,
}

impl AuditReport {
    pub fn has_violations(&self) -> bool {
        !self.events.is_empty()
    }

    pub fn summary(&self) -> String {
        if self.events.is_empty() {
            return "integrity ok".to_string();
        }
        self.events
            .iter()
            .map(|event| format!("[{}] {}: {}", event.kind.as_str(), event.label, event.detail))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

/// 对账器：注册不变量检查，周期执行。
#[derive(Default)]
pub struct Auditor {
    checks: Vec<Check>,
    history: VecDeque<AuditReport>,
}

type Check = Box<dyn Fn() -> Vec<AuditEvent> + Send + Sync>;

impl Auditor {
    pub fn new() -> Self {
        Self { checks: Vec::new(), history: VecDeque::new() }
    }

    /// 注册一项检查（闭包返回该检查发现的事件；空 = 通过）。
    pub fn register<F>(&mut self, name: &str, check: F)
    where
        F: Fn() -> Vec<AuditEvent> + Send + Sync + 'static,
    {
        let _ = name;
        self.checks.push(Box::new(check));
    }

    /// 执行一轮全检。
    pub fn run_round(&mut self) -> AuditReport {
        let mut report = AuditReport::default();
        for check in &self.checks {
            report.events.extend(check());
        }
        self.history.push_back(report.clone());
        if self.history.len() > 64 {
            self.history.pop_front();
        }
        report
    }

    /// 最近一轮。
    pub fn last(&self) -> Option<&AuditReport> {
        self.history.back()
    }
}
