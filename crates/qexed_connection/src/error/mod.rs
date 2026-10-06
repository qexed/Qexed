//! qexed_connection 错误类型。
//!
//! 迁移自 v4 的 anyhow 用法（连接 / 登录 / 认证 / 安全聊天 / 代理转发），
//! 全部落到本文件的 thiserror 枚举，禁止 anyhow。

/// 连接处理统一错误。
#[derive(Debug, thiserror::Error)]
pub enum ConnectionError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Config Error: {0}")]
    ConfigError(#[from] qexed_config::error::ConfigError),
    /// 传输层（PacketStream/PacketSink）读写失败。
    #[error("transport error: {0}")]
    Transport(#[from] crate::transport::TransportError),
    /// 包编解码失败（VarInt / NBT / 长度越界等）。
    #[error("packet error: {0}")]
    Packet(#[from] qexed_packet::PacketError),
    /// Mojang 会话服务器 / publickeys 服务 HTTP 请求失败。
    #[error("http error: {0}")]
    Http(String),
    /// 认证流程错误（token 不匹配、会话未确认、公钥无效等）。
    #[error("auth error: {0}")]
    Auth(String),
    /// 安全聊天校验错误（签名无效、last-seen 异常等）。
    #[error("secure chat error: {0}")]
    SecureChat(String),
    /// 代理转发（BungeeCord / Velocity）数据解析或校验失败。
    #[error("proxy forwarding error: {0}")]
    ProxyForwarding(String),
    /// 行为守则（code of conduct）文本加载失败。
    #[error("code of conduct error: {0}")]
    CodeOfConduct(String),
    /// 状态 ping 响应构造失败。
    #[error("status error: {0}")]
    Status(String),
    /// 监听端口绑定失败。
    #[error("bind error: {0}")]
    Bind(#[from] crate::transport::BindError),
    /// 注册表同步（依赖 qexed_mojang_data）数据缺失或损坏。
    #[error("registry error: {0}")]
    Registry(String),
    /// 其余未分类错误。
    #[error("{0}")]
    Message(String),
}

impl ConnectionError {
    /// 快捷构造 Message 变体。
    pub fn msg(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }
}

pub type Result<T> = std::result::Result<T, ConnectionError>;
