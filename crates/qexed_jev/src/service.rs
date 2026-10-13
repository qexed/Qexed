//! 对外门面：启动初始化 + 全局服务句柄（模式分发）。
//!
//! qexed_server 的接入方式（对齐 qexed_auth 的全局 init 惯例）：
//!
//! ```rust,ignore
//! // 启动时（qexed_server::server::init）：
//! qexed_jev::JevService::init_global()?;          // 读取 config/jev.toml（自动创建）
//! let jev = qexed_jev::JevService::get()?;        // &'static Arc<JevService>
//!
//! // 聊天处理处（PLAY 层）：
//! let mut conv = jev.conversation_with_system(key, Some(system_prompt));
//! let reply = conv.ask(message).await?;
//! ```

use crate::backend::JevBackend;
use crate::config::JevConfig;
use crate::error::JevError;
use crate::session::JevConversation;
use qexed_config::Config;
use std::sync::Arc;

/// 全局服务句柄：进程内共享一个后端实例（worker / HTTP 连接池都只建一次）。
static SERVICE: tokio::sync::OnceCell<Arc<JevService>> = tokio::sync::OnceCell::const_new();

pub struct JevService {
    config: Arc<JevConfig>,
    backend: JevBackend,
}

impl JevService {
    /// 加载 `config/jev.toml`（不存在则按默认值创建）并启动后端。
    ///
    /// - Embedded（默认）：启动常驻 worker 线程，无网络依赖；
    /// - Remote：构造带连接池的 HTTP 客户端（需 `remote` feature）。
    pub fn init() -> Result<Arc<Self>, JevError> {
        let config = Arc::new(JevConfig::load_and_create_default(true)?);
        Self::start(config)
    }

    /// 用给定配置启动（服务器可先加载再传入，便于统一校验）。
    pub fn start(config: Arc<JevConfig>) -> Result<Arc<Self>, JevError> {
        if !config.enable {
            log::info!("jev disabled by config");
        }
        let backend = crate::backend::build_backend(&config)?;
        log::info!("jev service started (mode={:?})", config.mode);
        Ok(Arc::new(Self { config, backend }))
    }

    /// 进程级单例（qexed_server 启动时调用一次；重复调用复用已有实例）。
    pub async fn init_global() -> Result<&'static Arc<Self>, JevError> {
        let service = Self::init()?;
        SERVICE.get_or_init(|| async move { service }).await;
        Self::get()
    }

    /// 取全局实例；未初始化返回错误而不是 panic（服务器可据此降级关闭 AI 功能）。
    pub fn get() -> Result<&'static Arc<Self>, JevError> {
        SERVICE.get().ok_or(JevError::WorkerDown)
    }

    pub fn enabled(&self) -> bool {
        self.config.enable
    }

    pub fn config(&self) -> &Arc<JevConfig> {
        &self.config
    }

    /// 后端只读访问（play 层想绕开会话直接问一发的场景）。
    pub fn backend(&self) -> &JevBackend {
        &self.backend
    }

    /// 为一个玩家/NPC 开对话（不带系统提示）。
    pub fn conversation(&self, key: impl Into<String>) -> JevConversation<'_> {
        self.conversation_with_system(key, None)
    }

    /// 为一个玩家/NPC 开对话（带系统提示，如 NPC 人设）。
    ///
    /// `key` 目前只用于日志定位；会话是调用方持有（PLAY 循环里生命周期
    /// 与玩家连接一致），之后接 Redis/存档时才需要全局会话表。
    pub fn conversation_with_system(
        &self,
        key: impl Into<String>,
        system: Option<String>,
    ) -> JevConversation<'_> {
        let _ = &key;
        JevConversation::new(&self.config, &self.backend, system)
    }
}

/// 每玩家会话表（可选）：上层如果想集中管理会话，用这个类型兜住锁语义。
pub type SessionMap<'a> = std::sync::Mutex<std::collections::HashMap<String, JevConversation<'a>>>;
