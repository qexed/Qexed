//! 管理 WebSocket 服务器：监听、认证（Bearer / 子协议）、会话与通知广播。

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, RwLock};
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::accept_hdr_async;

use crate::backend::ManagementBackend;
use crate::config::ManagementConfig;
use crate::error::rpc_code;
use crate::protocol::{RpcRequest, RpcResponse, dispatch};

/// 通知事件（method + params）。
#[derive(Debug, Clone)]
pub struct Notification {
    /// 完整方法名：minecraft:notification/<domain>/<event>
    pub method: String,
    pub params: Value,
}

impl Notification {
    pub fn new(domain: &str, event: &str, params: Value) -> Self {
        Self { method: format!("minecraft:notification/{domain}/{event}"), params }
    }

    pub fn encode(&self) -> String {
        json!({
            "jsonrpc": "2.0",
            "method": self.method,
            "params": self.params,
        })
        .to_string()
    }
}

/// 通知事件构造器（对齐规范的通知面）。
pub mod notify {
    use super::Notification;
    use crate::model::{IpBan, OperatorEntry, PlayerRef, TypedGameRule, UserBan};
    use serde_json::{Value, json};

    fn player_param(player: &PlayerRef) -> Value { serde_json::to_value(player).unwrap_or(Value::Null) }

    pub fn server_started() -> Notification { Notification::new("server", "started", json!({})) }
    pub fn server_stopping() -> Notification { Notification::new("server", "stopping", json!({})) }
    pub fn server_saving() -> Notification { Notification::new("server", "saving", json!({})) }
    pub fn server_saved() -> Notification { Notification::new("server", "saved", json!({})) }
    pub fn server_status(status: Value) -> Notification { Notification::new("server", "status", status) }
    pub fn server_activity() -> Notification { Notification::new("server", "activity", json!({})) }

    pub fn player_joined(player: &PlayerRef) -> Notification {
        Notification::new("players", "joined", json!({ "player": player_param(player) }))
    }
    pub fn player_left(player: &PlayerRef) -> Notification {
        Notification::new("players", "left", json!({ "player": player_param(player) }))
    }

    pub fn operator_added(op: &OperatorEntry) -> Notification {
        Notification::new("operators", "added", json!({ "player": serde_json::to_value(op).unwrap_or(Value::Null) }))
    }
    pub fn operator_removed(op: &OperatorEntry) -> Notification {
        Notification::new("operators", "removed", json!({ "player": serde_json::to_value(op).unwrap_or(Value::Null) }))
    }

    pub fn allowlist_added(player: &PlayerRef) -> Notification {
        Notification::new("allowlist", "added", json!({ "player": player_param(player) }))
    }
    pub fn allowlist_removed(player: &PlayerRef) -> Notification {
        Notification::new("allowlist", "removed", json!({ "player": player_param(player) }))
    }

    pub fn ip_ban_added(ban: &IpBan) -> Notification {
        Notification::new("ip_bans", "added", json!({ "player": serde_json::to_value(ban).unwrap_or(Value::Null) }))
    }
    pub fn ip_ban_removed(ip: &str) -> Notification {
        Notification::new("ip_bans", "removed", json!({ "player": ip }))
    }

    pub fn ban_added(ban: &UserBan) -> Notification {
        Notification::new("bans", "added", json!({ "player": serde_json::to_value(ban).unwrap_or(Value::Null) }))
    }
    pub fn ban_removed(player: &PlayerRef) -> Notification {
        Notification::new("bans", "removed", json!({ "player": player_param(player) }))
    }

    pub fn gamerule_updated(rule: &TypedGameRule) -> Notification {
        Notification::new("gamerules", "updated", json!({ "gamerule": serde_json::to_value(rule).unwrap_or(Value::Null) }))
    }

    pub fn world_upgrade_started() -> Notification { Notification::new("world", "upgrade_started", json!({})) }
    pub fn world_upgrade_progress(progress: f64) -> Notification {
        Notification::new("world", "upgrade_progress", json!({ "progress": progress.clamp(0.0, 1.0) }))
    }
    pub fn world_upgrade_finished() -> Notification { Notification::new("world", "upgrade_finished", json!({})) }
    pub fn world_upgrade_failed(reason: &str) -> Notification {
        Notification::new("world", "upgrade_failed", json!({ "reason": reason }))
    }
}

/// 会话外发队列（通知广播用）。
type Outbound = Arc<Mutex<tokio::sync::mpsc::UnboundedSender<WsMessage>>>;

/// 管理服务器。
pub struct ManagementServer {
    config: ManagementConfig,
    backend: Arc<dyn ManagementBackend>,
    sessions: Arc<RwLock<HashMap<u64, Outbound>>>,
    next_session: Arc<std::sync::atomic::AtomicU64>,
    local_addr: Arc<std::sync::OnceLock<SocketAddr>>,
    stop_signal: tokio::sync::watch::Sender<bool>,
}

impl ManagementServer {
    pub fn new(config: ManagementConfig, backend: Arc<dyn ManagementBackend>) -> Self {
        let (stop_tx, _) = tokio::sync::watch::channel(false);
        Self {
            config,
            backend,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            next_session: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            local_addr: Arc::new(std::sync::OnceLock::new()),
            stop_signal: stop_tx,
        }
    }

    /// 实际监听地址（启动后有效；port=0 时查随机分配端口）。
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.local_addr.get().copied()
    }

    /// 请求停止（accept 循环退出；活跃会话自然结束）。
    pub fn stop(&self) {
        let _ = self.stop_signal.send(true);
    }

    /// 启动监听（阻塞至停止信号）。幂等保护：重复调用返回。
    pub async fn run(&self) -> crate::Result<()> {
        let listener = TcpListener::bind((self.config.host.as_str(), self.config.port)).await?;
        let addr = listener.local_addr()?;
        let _ = self.local_addr.set(addr);
        log::info!(
            "{}",
            qexed_language::t("qexed.management.listening")
                .replace("%{addr}", &addr.to_string())
        );

        let mut shutdown = self.stop_signal.subscribe();
        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        break;
                    }
                }
                accepted = listener.accept() => {
                    let (stream, peer) = accepted?;
                    let backend = self.backend.clone();
                    let sessions = self.sessions.clone();
                    let next_id = self.next_session.clone();
                    let secret = self.config.secret.clone();
                    let allowed_origins = self.config.allowed_origins.clone();
                    tokio::spawn(async move {
                        if let Err(err) =
                            handle_connection(stream, peer, backend, sessions, next_id, secret, allowed_origins).await
                        {
                            log::debug!("management session {peer} ended: {err}");
                        }
                    });
                }
            }
        }
        Ok(())
    }

    /// 向全部已认证会话广播通知。
    pub async fn broadcast(&self, notification: &Notification) {
        let sessions = self.sessions.read().await;
        for (_, outbound) in sessions.iter() {
            let _ = outbound.lock().await.send(WsMessage::Text(notification.encode().into()));
        }
    }
}

/// 单连接处理：HTTP 升级时做认证与 Origin 校验。
#[allow(clippy::too_many_arguments)]
async fn handle_connection(
    stream: TcpStream,
    _peer: SocketAddr,
    backend: Arc<dyn ManagementBackend>,
    sessions: Arc<RwLock<HashMap<u64, Outbound>>>,
    next_id: Arc<std::sync::atomic::AtomicU64>,
    secret: String,
    allowed_origins: Vec<String>,
) -> crate::Result<()> {
    // 提取认证信息（回调里捕获）
    let secret_cb = secret.clone();
    let origins_cb = allowed_origins;

    let ws = accept_hdr_async(stream, move |request: &Request, response: Response| {
        let verdict = evaluate_auth(request, &secret_cb, &origins_cb);
        let _ = &verdict;
        match verdict {
            AuthVerdict::Ok => {
                let mut accepted = response;
                if let Some(protocol) = request.headers().get("Sec-WebSocket-Protocol").and_then(|v| v.to_str().ok()) {
                    if let Some(token) = protocol.split(',').map(str::trim).find(|p| *p == secret_cb) {
                        accepted.headers_mut().insert(
                            "Sec-WebSocket-Protocol",
                            tokio_tungstenite::tungstenite::http::HeaderValue::from_str(token)
                                .unwrap_or_else(|_| tokio_tungstenite::tungstenite::http::HeaderValue::from_static("minecraft-v1")),
                        );
                    }
                }
                Ok(accepted)
            }
            AuthVerdict::Pending | AuthVerdict::Unauthorized(_) => {
                let mut denied = response.map(|()| Some("unauthorized".to_string()));
                *denied.status_mut() = tokio_tungstenite::tungstenite::http::StatusCode::UNAUTHORIZED;
                // body 由 Response<()> 泛型决定，map 换为 Option<String> 形态在 Err 变体
                Err(denied)
            }
        }
    })
    .await?;

    let session_id = next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel::<WsMessage>();
    sessions.write().await.insert(session_id, Arc::new(Mutex::new(out_tx)));

    let (mut sink, mut stream_rx) = ws.split();

    // 外发泵：通知 + 响应
    let outbound_pump = tokio::spawn(async move {
        while let Some(message) = out_rx.recv().await {
            if sink.send(message).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });

    // 请求循环
    while let Some(message) = stream_rx.next().await {
        let message = message?;
        match message {
            WsMessage::Text(text) => {
                if let Some(response) = handle_text(&backend, &text) {
                    let _ = out_forward(&sessions, session_id, &response).await;
                }
            }
            WsMessage::Ping(payload) => {
                let _ = out_forward_raw(&sessions, session_id, WsMessage::Pong(payload)).await;
            }
            WsMessage::Close(_) => break,
            _ => {}
        }
    }

    sessions.write().await.remove(&session_id);
    outbound_pump.abort();
    Ok(())
}

async fn out_forward(sessions: &Arc<RwLock<HashMap<u64, Outbound>>>, id: u64, response: &RpcResponse) -> bool {
    out_forward_raw(sessions, id, WsMessage::Text(serde_json::to_string(response).unwrap_or_default().into())).await
}

async fn out_forward_raw(sessions: &Arc<RwLock<HashMap<u64, Outbound>>>, id: u64, message: WsMessage) -> bool {
    if let Some(outbound) = sessions.read().await.get(&id) {
        return outbound.lock().await.send(message).is_ok();
    }
    false
}

fn handle_text(backend: &Arc<dyn ManagementBackend>, text: &str) -> Option<RpcResponse> {
    let request: RpcRequest = match serde_json::from_str(text) {
        Ok(request) => request,
        Err(err) => {
            return Some(RpcResponse::err(
                Value::Null,
                rpc_code::PARSE_ERROR,
                format!("parse error: {err}"),
            ))
        }
    };
    // 通知（无 id）不产生响应
    if request.id.is_none() {
        return None;
    }
    Some(dispatch(backend.as_ref(), &request))
}

#[derive(Debug, Clone, Default)]
enum AuthVerdict {
    #[default]
    Pending,
    Ok,
    Unauthorized(String),
}

fn evaluate_auth(request: &Request, secret: &str, allowed_origins: &[String]) -> AuthVerdict {
    // Origin 校验（配置了列表时必须匹配）
    if !allowed_origins.is_empty() {
        let origin = request
            .headers()
            .get("Origin")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        if !allowed_origins.iter().any(|allowed| allowed == origin) {
            return AuthVerdict::Unauthorized(format!("origin not allowed: {origin}"));
        }
    }
    // Bearer
    if let Some(auth_header) = request.headers().get("Authorization").and_then(|v| v.to_str().ok()) {
        if auth_header == format!("Bearer {secret}") {
            return AuthVerdict::Ok;
        }
    }
    // 子协议：minecraft-v1,<secret>
    if let Some(protocols) = request.headers().get("Sec-WebSocket-Protocol").and_then(|v| v.to_str().ok()) {
        for protocol in protocols.split(',') {
            let protocol = protocol.trim();
            if protocol == format!("minecraft-v1,{secret}") || protocol == secret {
                return AuthVerdict::Ok;
            }
        }
    }
    AuthVerdict::Unauthorized("missing or invalid credentials".to_string())
}
