//! 内嵌服务端到端：会话往返、配置校验、默认值序列化、并发与背压形状。

use qexed_jev::{JevConfig, JevMode, JevService};
use std::sync::Arc;

fn embedded_config() -> JevConfig {
    JevConfig {
        enable: true,
        mode: JevMode::Embedded,
        ..JevConfig::default()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn echo_end_to_end() {
    let jev = JevService::start(Arc::new(embedded_config())).unwrap();
    assert!(jev.enabled());

    let mut conv = jev.conversation_with_system("player-1", Some("你是村长。".into()));
    let reply = conv.ask("hello jev").await.unwrap();
    assert_eq!(reply.text, "[jev:echo] hello jev");

    let reply2 = conv.ask("again").await.unwrap();
    assert_eq!(reply2.text, "[jev:echo] again");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_players() {
    let jev = JevService::start(Arc::new(embedded_config())).unwrap();
    let mut handles = Vec::new();
    for i in 0..8 {
        let jev = jev.clone();
        handles.push(tokio::spawn(async move {
            let mut conv = jev.conversation(format!("player-{i}"));
            let reply = conv.ask(format!("msg-{i}")).await.unwrap();
            assert_eq!(reply.text, format!("[jev:echo] msg-{i}"));
        }));
    }
    for handle in handles {
        handle.await.unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queue_full_backpressure() {
    // 引擎排队速度远快于提交速度时打不出背压；这里验证 API 形状：
    // 超小队列 + 大量并发提交，所有请求要么成功要么立刻 QueueFull，绝不挂死。
    let mut config = embedded_config();
    config.embedded.queue_capacity = 1;
    config.embedded.request_timeout_ms = 2_000;
    let jev = JevService::start(Arc::new(config)).unwrap();

    let mut handles = Vec::new();
    for i in 0..32 {
        let jev = jev.clone();
        handles.push(tokio::spawn(async move {
            let mut conv = jev.conversation(format!("burst-{i}"));
            match conv.ask(format!("burst-{i}")).await {
                Ok(reply) => assert_eq!(reply.text, format!("[jev:echo] burst-{i}")),
                Err(qexed_jev::JevError::QueueFull { .. }) => {} // 预期内的背压
                Err(err) => panic!("unexpected error: {err}"),
            }
        }));
    }
    for handle in handles {
        handle.await.unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn history_window_trims() {
    let mut config = embedded_config();
    config.max_history_turns = 2;
    let jev = JevService::start(Arc::new(config)).unwrap();
    let mut conv = jev.conversation("player-4");
    for i in 0..10 {
        conv.ask(format!("m{i}")).await.unwrap();
    }
    assert!(
        conv.session.len() <= 4,
        "history must stay <= max_turns*2, got {}",
        conv.session.len()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disabled_service_still_constructs() {
    let mut config = embedded_config();
    config.enable = false;
    let jev = JevService::start(Arc::new(config)).unwrap();
    assert!(!jev.enabled());
    // 禁用只影响上层是否调用；底层管线保持可运行（echo 兜底）
    let mut conv = jev.conversation("player-3");
    let reply = conv.ask("ping").await.unwrap();
    assert_eq!(reply.text, "[jev:echo] ping");
}

#[test]
fn embedded_config_validate_rejects_zero() {
    let mut config = embedded_config();
    config.embedded.queue_capacity = 0;
    assert!(config.embedded.validate().is_err());
    config.embedded.queue_capacity = 4;
    config.embedded.micro_batch = 0;
    assert!(config.embedded.validate().is_err());
    config.embedded.micro_batch = 2;
    config.embedded.request_timeout_ms = 0;
    assert!(config.embedded.validate().is_err());
}

#[test]
fn config_toml_roundtrip_defaults() {
    // 默认模式必须是 Embedded：直接断言序列化结果
    let mode = qexed_doc::eval_default(&JevMode::default());
    assert_eq!(mode, serde_json::json!("Embedded"), "default mode must be Embedded");
}

// ── 内容检测（广告 / 号码 / 违法）端到端 ──

use qexed_jev::{Action, ModerationPolicy};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ask_blocks_illegal_input() {
    let mut config = embedded_config();
    config.moderation.illegal = Action::Block;
    let jev = JevService::start(Arc::new(config)).unwrap();
    let mut conv = jev.conversation("player-block");
    let err = conv.ask("卖外挂加我").await.unwrap_err();
    assert!(
        matches!(err, qexed_jev::JevError::Moderated { .. }),
        "expect Moderated, got {err}"
    );
    // 被拒的输入不进历史
    assert_eq!(conv.session.len(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ask_sanitizes_numbers_by_default() {
    let jev = JevService::start(Arc::new(embedded_config())).unwrap();
    let mut conv = jev.conversation("player-mask");
    // 默认 Sanitize：号码打码后照常推理（echo 引擎回显打码结果）
    let reply = conv.ask("我qq 12345678").await.unwrap();
    assert!(!reply.text.contains("12345678"), "leaked: {}", reply.text);
    assert!(reply.text.contains('＊'));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ask_allow_passes_through() {
    let mut config = embedded_config();
    config.moderation.contact_numbers = Action::Allow;
    let jev = JevService::start(Arc::new(config)).unwrap();
    let mut conv = jev.conversation("player-allow");
    let reply = conv.ask("房间号 12345").await.unwrap();
    assert!(reply.text.contains("12345"), "allow lost content: {}", reply.text);
}

#[test]
fn moderation_policy_toml_defaults() {
    // 默认策略可序列化（toml 兼容），且三类都是 Sanitize
    let policy = ModerationPolicy::default();
    let json = serde_json::to_string(&policy).unwrap();
    assert!(json.contains("\"Sanitize\""));
    let back: ModerationPolicy = serde_json::from_str(&json).unwrap();
    assert_eq!(back.ads, Action::Sanitize);
    assert_eq!(back.contact_numbers, Action::Sanitize);
    assert_eq!(back.illegal, Action::Sanitize);
}
