//! 每玩家（或每 NPC）对话会话：滑动历史 + 从服务配置构造请求。

use crate::backend::JevBackend;
use crate::config::JevConfig;
use crate::engine::JevRequest;
use crate::error::JevError;
use crate::moderation::{Action, Moderator};
use std::collections::VecDeque;
use std::time::Instant;

/// 一条对话记录。
#[derive(Debug, Clone)]
pub struct JevTurn {
    pub role: String,
    pub content: String,
}

/// 单个会话的滑动窗口历史。
///
/// 超过 [`JevConfig::max_history_turns`] 时丢最旧的一轮；
/// 这是推理成本的下限保护，避免长会话把上下文撑爆。
#[derive(Debug)]
pub struct JevSession {
    system: Option<String>,
    history: VecDeque<JevTurn>,
    max_turns: usize,
}

impl JevSession {
    pub fn new(config: &JevConfig, system: Option<String>) -> Self {
        Self {
            system,
            history: VecDeque::new(),
            max_turns: config.max_history_turns.max(1),
        }
    }

    /// 追加一轮对话并裁剪窗口。
    pub fn push(&mut self, role: impl Into<String>, content: impl Into<String>) {
        self.history.push_back(JevTurn {
            role: role.into(),
            content: content.into(),
        });
        while self.history.len() > self.max_turns * 2 {
            self.history.pop_front();
        }
    }

    /// 用当前历史构造推理请求。
    pub fn build_request(&self, temperature: Option<f32>) -> JevRequest {
        let mut messages = Vec::with_capacity(self.history.len() + 1);
        if let Some(system) = &self.system {
            messages.push(("system".to_string(), system.clone()));
        }
        for turn in &self.history {
            messages.push((turn.role.clone(), turn.content.clone()));
        }
        JevRequest {
            model: None,
            messages,
            temperature,
        }
    }

    pub fn len(&self) -> usize {
        self.history.len()
    }

    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }
}

/// 一次完整对话往返（供上层计费/日志）。
#[derive(Debug, Clone)]
pub struct JevReply {
    pub text: String,
    /// 排队 + 推理总耗时。
    pub elapsed_ms: u64,
}

/// 会话 + 后端的便捷组合：检测 → push → complete → 检测 → push。
pub struct JevConversation<'a> {
    pub session: JevSession,
    backend: &'a JevBackend,
    config: &'a JevConfig,
    timeout_ms: u64,
    moderator: Moderator,
}

impl<'a> JevConversation<'a> {
    pub fn new(
        config: &'a JevConfig,
        backend: &'a JevBackend,
        system: Option<String>,
    ) -> Self {
        let session = JevSession::new(config, system);
        Self {
            session,
            backend,
            config,
            timeout_ms: config.embedded.request_timeout_ms,
            moderator: Moderator::new(&config.moderation),
        }
    }

    /// 发送一条用户消息并等待 jev 回复；往返都记入历史窗口。
    ///
    /// 内容检测在两端生效：
    /// - 输入：命中 Block 直接拒绝（不入历史、不推理）；Sanitize 打码后送推理；
    /// - 输出：命中 Block 用固定拒答文案替换，Sanitize 打码后返回。
    pub async fn ask(&mut self, message: impl Into<String>) -> Result<JevReply, JevError> {
        let message = message.into();

        // ── 输入侧检测 ──
        let mut verdict = self.moderator.inspect(&message);
        if verdict.is_clean() && self.config.moderation.semantic_enable {
            // 规则快筛未命中 → 模型语义判定（换写法/隐喻/谐音）。
            // 失败/超时按未命中放行；村长消息会在桥的门外先判一次，
            // 这里是对话内自用的兜底（独立使用 ask 的场景）。
            match crate::semantic::classify(self.backend, self.config, &self.moderator, &message)
                .await
            {
                Ok(Some(hit)) => verdict = hit,
                Ok(None) => {}
                Err(err) => log::warn!("semantic input moderation failed, allowing: {err}"),
            }
        }
        if !verdict.is_clean() {
            log::info!(
                "jev moderation input hit: {:?}",
                verdict
                    .violations
                    .iter()
                    .map(|v| (v.category.label(), v.rule.as_str(), v.matched.as_str()))
                    .collect::<Vec<_>>()
            );
        }
        let message = match verdict.action {
            Action::Block => {
                let categories = verdict
                    .categories()
                    .iter()
                    .map(|c| c.label())
                    .collect::<Vec<_>>()
                    .join(",");
                return Err(JevError::Moderated { categories });
            }
            Action::Sanitize => self.moderator.sanitize_verdict(&message, &verdict),
            Action::Allow => message,
        };

        self.session.push("user", message);
        let started = Instant::now();
        let text = self
            .backend
            .complete(self.session.build_request(None), self.timeout_ms)
            .await?;

        // ── 输出侧检测 ──
        let mut out_verdict = self.moderator.inspect(&text);
        if out_verdict.is_clean() && self.config.moderation.semantic_enable {
            match crate::semantic::classify(self.backend, self.config, &self.moderator, &text).await
            {
                Ok(Some(hit)) => out_verdict = hit,
                Ok(None) => {}
                Err(err) => log::warn!("semantic output moderation failed, allowing: {err}"),
            }
        }
        let text = if out_verdict.is_clean() {
            text
        } else {
            log::info!(
                "jev moderation output hit: {:?}",
                out_verdict
                    .violations
                    .iter()
                    .map(|v| (v.category.label(), v.rule.as_str(), v.matched.as_str()))
                    .collect::<Vec<_>>()
            );
            match out_verdict.action {
                Action::Block => self.moderator.refusal_text().to_string(),
                Action::Sanitize => self.moderator.sanitize_verdict(&text, &out_verdict),
                Action::Allow => text,
            }
        };

        self.session.push("assistant", &text);
        Ok(JevReply {
            text,
            elapsed_ms: started.elapsed().as_millis() as u64,
        })
    }
}
