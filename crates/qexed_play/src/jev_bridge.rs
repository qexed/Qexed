//! PLAY 层的 qexed_jev 接入点：聊天内容检测 + 村长 NPC 触发。
//!
//! 依赖方向（对齐 jev 文档）：
//! ```text
//! qexed_server →（启动时）qexed_jev::JevService::init_global()
//! qexed_play   →（聊天入口）JevService::get() → 检测 / NPC 回复
//! ```
//! qexed_chat 保持协议纯净，不感知 jev；本模块是两者之间唯一的桥。
//!
//! 行为：
//! - jev 未初始化 / 未启用：完全放行，零开销（不含 AI 回复）；
//! - 内容检测（广告 / 号码 / 违法）：Block 拦截；Sanitize 打码照转（签名链
//!   要求）+ 向发送者提示「已被拦截」，显示层对所有人遮蔽原文；
//! - 消息提及「村长」时把（打码后的）文本交给 AI 会话，回复以系统聊天跟发。

use qexed_jev::{JevService, Moderator};
use uuid::Uuid;

/// NPC 触发词：消息包含它才请求 AI 回复（不触发推理时零成本）。
pub const NPC_MENTION: &str = "村长";
/// NPC 系统提示（人设 + 简短回答约束）。
const NPC_SYSTEM: &str = "你是一座 Minecraft 服务器里的村长 NPC。用中文回答，保持一两句话，简短友好。";

/// 聊天门控结果。
#[derive(Debug, Clone, PartialEq)]
pub enum JevChatOutcome {
    /// 广播这段文本（可能被 Sanitize 打码）。
    Allowed { text: String },
    /// 广播文本，并跟发一条 AI 回复。
    AllowedWithReply { text: String, reply: String },
    /// 广播打码文本（签名链要求消息照转），同时向发送者提示被拦截。
    Sanitized { text: String, notice: String },
    /// 输入被 Block：不广播，只发提示。
    Blocked { notice: String },
}

impl JevChatOutcome {
    /// 应当广播的文本（Blocked 无广播文本）。
    pub fn text(&self) -> Option<&str> {
        match self {
            JevChatOutcome::Allowed { text }
            | JevChatOutcome::AllowedWithReply { text, .. }
            | JevChatOutcome::Sanitized { text, .. } => Some(text),
            JevChatOutcome::Blocked { .. } => None,
        }
    }

    /// 需要跟发的 AI 回复。
    pub fn reply(&self) -> Option<&str> {
        match self {
            JevChatOutcome::AllowedWithReply { reply, .. } => Some(reply),
            _ => None,
        }
    }
}

/// 聊天入口门控：取全局 jev 服务（未初始化即原样放行）。
pub async fn gate_chat_message(uuid: Uuid, raw: &str) -> JevChatOutcome {
    match JevService::get() {
        Ok(jev) => gate_with(jev, uuid, raw).await,
        Err(_) => JevChatOutcome::Allowed { text: raw.to_string() },
    }
}

/// 用给定服务执行门控（可测试；服务为 None 等价于未启用）。
///
/// 顺序：Block 先拦（节省推理）→ Sanitize 打码照转 + 拦截提示 →
/// 提及 NPC 才请求回复。AI 输入用打码后的文本：违规内容不进入推理引擎。
pub async fn gate_with(jev: &JevService, uuid: Uuid, raw: &str) -> JevChatOutcome {
    if !jev.enabled() {
        return JevChatOutcome::Allowed { text: raw.to_string() };
    }
    let moderator = Moderator::new(&jev.config().moderation);
    let mut verdict = moderator.inspect(raw);

    // 规则快筛未命中 → 语义判定（模型理解）：换写法、隐喻、谐音归它抓。
    // 关闭（semantic_enable=false）或推理不可用时放行，聊天永远可用。
    if verdict.is_clean() {
        match qexed_jev::semantic::classify(jev.backend(), jev.config(), &moderator, raw).await {
            Ok(Some(hit)) => {
                log::info!("semantic moderation hit for {uuid}: {:?}",
                    hit.violations.iter().map(|v| v.category.label()).collect::<Vec<_>>());
                verdict = hit;
            }
            Ok(None) => {}
            Err(err) => log::warn!("semantic moderation failed, allowing message: {err}"),
        }
    }

    match verdict.action {
        qexed_jev::Action::Block => {
            let categories = verdict
                .categories()
                .into_iter()
                .map(|c| c.label())
                .collect::<Vec<_>>()
                .join(",");
            log::info!("chat blocked for {uuid} (categories={categories})");
            return JevChatOutcome::Blocked {
                notice: "该消息包含违规内容，已被拦截。".to_string(),
            };
        }
        qexed_jev::Action::Sanitize => {
            log::info!(
                "chat moderated (blocked) for {uuid}: {:?}",
                verdict.categories().iter().map(|c| c.label()).collect::<Vec<_>>()
            );
            // 签名链要求消息照转（丢包/换文本=验证错误），打码文本广播，
            // 同时告知发送者消息已被拦截。显示层由 FULLY_FILTERED 遮蔽。
            return JevChatOutcome::Sanitized {
                text: moderator.sanitize_verdict(raw, &verdict),
                notice: "该消息包含违规内容，已被拦截。".to_string(),
            };
        }
        qexed_jev::Action::Allow => {}
    }

    // 走到这里的一定是干净文本
    let text = raw.to_string();
    if !text.contains(NPC_MENTION) {
        return JevChatOutcome::Allowed { text };
    }

    let key = format!("npc:{uuid}");
    let mut conv = jev.conversation_with_system(key, Some(NPC_SYSTEM.to_string()));
    match conv.ask(&text).await {
        Ok(reply) => JevChatOutcome::AllowedWithReply { text, reply: reply.text },
        Err(err) => {
            log::warn!("jev ask failed (chat continues without AI reply): {err}");
            JevChatOutcome::Allowed { text }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qexed_jev::JevConfig;
    use std::sync::Arc;

    fn service() -> Arc<JevService> {
        // EchoEngine 确定性回复，测试无需真实权重
        JevService::start(Arc::new(JevConfig::default())).expect("start embedded jev")
    }

    fn service_with(policy: qexed_jev::ModerationPolicy) -> Arc<JevService> {
        let mut config = JevConfig::default();
        config.moderation = policy;
        JevService::start(Arc::new(config)).expect("start embedded jev")
    }

    #[tokio::test]
    async fn passthrough_clean_message() {
        let jev = service();
        let out = gate_with(&jev, uuid::Uuid::new_v4(), "大家好，晚上一起挖矿吗").await;
        assert_eq!(out.text().unwrap(), "大家好，晚上一起挖矿吗");
        assert!(out.reply().is_none());
    }

    #[tokio::test]
    async fn sanitizes_contact_numbers_with_block_notice() {
        let jev = service();
        let out = gate_with(&jev, uuid::Uuid::new_v4(), "加我qq 12345678 聊").await;
        // 打码照转（签名链要求）+ 发送者收到拦截提示
        let JevChatOutcome::Sanitized { text, notice } = out else {
            panic!("应为 Sanitized，got {out:?}");
        };
        assert!(!text.contains("12345678"), "号码必须被打码: {text}");
        assert!(text.contains('＊'), "应含打码字符: {text}");
        assert!(notice.contains("拦截"), "应有拦截提示: {notice}");
    }

    #[tokio::test]
    async fn blocks_illegal_input() {
        // 默认全 Sanitize 不拦截，需显式把违法类配成 Block
        let mut policy = qexed_jev::ModerationPolicy::default();
        policy.illegal = qexed_jev::Action::Block;
        let jev = service_with(policy);
        let out = gate_with(&jev, uuid::Uuid::new_v4(), "卖外挂加我qq 12345678").await;
        assert!(matches!(out, JevChatOutcome::Blocked { .. }));
        assert!(out.text().is_none());
    }

    #[tokio::test]
    async fn illegal_sanitize_masks_with_notice_by_default() {
        let jev = service();
        let out = gate_with(&jev, uuid::Uuid::new_v4(), "卖外挂加我qq 12345678").await;
        let JevChatOutcome::Sanitized { text, notice } = out else {
            panic!("默认 Sanitize 应为 Sanitized，got {out:?}");
        };
        assert!(!text.contains("外挂"), "命中词应被打码: {text}");
        assert!(!text.contains("12345678"), "号码应被打码: {text}");
        assert!(notice.contains("拦截"));
    }

    #[tokio::test]
    async fn vice_keywords_sanitized_with_notice() {
        // 线上实测漏网词：卖淫必须在违法词表里；Sanitize 打码照转+提示
        let jev = service();
        let out = gate_with(&jev, uuid::Uuid::new_v4(), "卖淫加我qq 12345678").await;
        let JevChatOutcome::Sanitized { text, notice } = out else {
            panic!("应为 Sanitized，got {out:?}");
        };
        assert!(!text.contains("卖淫"), "涉黄词应被打码: {text}");
        assert!(notice.contains("拦截"));
    }

    #[tokio::test]
    async fn semantic_enable_with_echo_engine_degrades_to_allow() {
        // Echo 引擎不输出合法判定 JSON → 解析失败按未命中放行（可用性优先）。
        // 真实权重接入后同一路径会真正拦截语义违规。
        let mut config = JevConfig::default();
        config.moderation.semantic_enable = true;
        let jev = JevService::start(Arc::new(config)).expect("start embedded jev");
        let out = gate_with(&jev, uuid::Uuid::new_v4(), "今天天气不错适合盖房子").await;
        assert_eq!(out.text().unwrap(), "今天天气不错适合盖房子");
        assert!(out.reply().is_none());
    }

    #[tokio::test]
    async fn npc_mention_triggers_reply() {
        let jev = service();
        let out = gate_with(&jev, uuid::Uuid::new_v4(), "村长，附近哪里有钻石？").await;
        assert_eq!(out.text().unwrap(), "村长，附近哪里有钻石？");
        let reply = out.reply().expect("提及村长应有 AI 回复");
        assert!(!reply.is_empty());
    }

    #[tokio::test]
    async fn no_mention_no_inference() {
        let jev = service();
        let out = gate_with(&jev, uuid::Uuid::new_v4(), "谁在挖铁矿").await;
        assert!(out.reply().is_none(), "未提及村长不应触发推理");
    }
}
