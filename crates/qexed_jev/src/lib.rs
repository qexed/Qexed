//! qexed_jev：jev 模型推理网关 —— 默认内嵌服务，可切远端 HTTP。
//!
//! # 定位
//!
//! ```text
//! qexed_server → qexed_play → qexed_chat      （聊天：协议 / 签名 / 限速）
//!            └────────────→ qexed_jev         （AI 回复：内嵌 jev 推理 / 远端 HTTP）
//! ```
//!
//! 与 qexed_light 一样是叶子 crate：不依赖任何 qexed_* 运行时 crate，
//! 依赖方向永远是 server/play 层向下引用本 crate。qexed_chat 保持协议职责纯净，
//! 不接 AI 的部署也完全不为 jev 付出编译或运行成本。
//!
//! # 默认内嵌（Embedded）
//!
//! - 专用 worker OS 线程独占引擎：推理不阻塞 tokio runtime，玩家网络包不受影响；
//! - 有界队列（[`config::JevEmbeddedConfig::queue_capacity`]）硬背压：
//!   满了立刻返回 [`JevError::QueueFull`]，内存占用有上界；
//! - 微批（[`config::JevEmbeddedConfig::micro_batch`]）：worker 把同时就绪的请求
//!   凑批喂引擎，为真实 jev 权重的加速器吞吐做准备；
//! - 现阶段内置零权重 [`EchoEngine`]（确定性回声），真实 jev 权重接入点在
//!   [`engine::build_embedded_engine`]。
//!
//! # 用法（qexed_chat 集成视角）
//!
//! ```
//! use qexed_jev::{JevConfig, JevMode, JevService};
//! use std::sync::Arc;
//!
//! # async fn demo() -> Result<(), qexed_jev::JevError> {
//! let config = Arc::new(JevConfig { mode: JevMode::Embedded, ..JevConfig::default() });
//! let jev = JevService::start(config)?;
//! let mut conv = jev.conversation_with_system(
//!     "player-uuid",
//!     Some("你是一座 Minecraft 服务器里的村长 NPC。".into()),
//! );
//! let reply = conv.ask("附近哪里有钻石？").await?;
//! assert!(!reply.text.is_empty());
//! # Ok(())
//! # }
//! ```
//!
//! # 切远端（可选）
//!
//! 在 \`config/jev.toml\` 里改 `mode = "Remote"` 并编译 `features = ["remote"]`；
//! `remote.api_key` 是机密字段，自动落到 \`config/.secrets/jev.toml\`。

pub mod backend;
pub mod config;
pub mod engine;
pub mod error;
pub mod moderation;
pub mod service;
pub mod session;

pub use backend::JevBackend;
pub use config::embedded::JevEmbeddedConfig;
pub use config::mode::JevMode;
pub use config::remote::JevRemoteConfig;
pub use config::JevConfig;
pub use engine::{EchoEngine, JevCompletion, JevEngine, JevRequest};
pub use error::JevError;
pub use moderation::{Action, Category, ModerationPolicy, Moderator, Verdict, Violation};
pub use service::{JevService, SessionMap};
pub use session::{JevConversation, JevReply, JevSession, JevTurn};
pub mod semantic;
pub use semantic::SemanticJudgement;
