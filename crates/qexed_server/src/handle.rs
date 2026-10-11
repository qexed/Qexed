//! 服务器统一关闭入口与跨连接共享上下文。

use tokio::sync::watch;

use crate::config;

/// 外部/内部统一的关闭入口。
#[derive(Clone)]
pub struct ServerHandle {
    shutdown_tx: watch::Sender<bool>,
}

impl ServerHandle {
    /// 由 [`crate::server::init`] 内部构造。
    pub(crate) fn from_sender(shutdown_tx: watch::Sender<bool>) -> Self {
        Self { shutdown_tx }
    }

    /// 触发关闭。可从任意任务调用，幂等。
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }

    /// 阻塞直到 shutdown 被触发。
    /// 外部 Ctrl+C、内部 `shutdown()`、accept loop 异常退出，都会唤醒。
    pub async fn wait(&self) {
        let mut rx = self.shutdown_tx.subscribe();
        if *rx.borrow() {
            return;
        }
        let _ = rx.changed().await;
    }

    /// 给后台任务订阅关闭信号用。
    pub fn subscribe(&self) -> watch::Receiver<bool> {
        self.shutdown_tx.subscribe()
    }
}

/// 单连接的共享上下文。
#[derive(Clone)]
pub struct ServerContext {
    pub config: std::sync::Arc<config::ServerConfig>,
    pub authenticator: std::sync::Arc<qexed_auth::Authenticator>,
    pub shutdown: ServerHandle,
}