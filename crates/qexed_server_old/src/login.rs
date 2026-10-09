//! LOGIN 状态：握手 -> login_start -> 压缩协商 -> success -> ack。

use qexed_protocol::to_client;
use qexed_protocol::to_server::handshaking::set_protocol::SetProtocol;
use qexed_protocol::to_server::login::login_acknowledged::LoginAcknowledged;
use qexed_protocol::to_server::login::login_start::LoginStart;

use crate::error::ServerError;
use crate::transport::Connection;

/// 服务器公告的协议版本与 MC 版本。
#[derive(Debug, Clone)]
pub struct ProtocolInfo {
    pub protocol_version: i32,
    pub mc_version: String,
}

/// 登录结果。
pub struct LoginOutcome {
    pub username: String,
    pub uuid: uuid::Uuid,
    pub protocol_version: i32,
}

/// 离线模式 UUID（vanilla 兼容：MD5("OfflinePlayer:{name}")）。见 [`crate::auth::offline_uuid`]。
pub fn offline_uuid(name: &str) -> uuid::Uuid {
    crate::auth::offline_uuid(name)
}

/// 处理 LOGIN 状态（handshake 已解析 next_state=2）。
pub async fn handle_login<R, W>(
    conn: &mut Connection<R, W>,
    handshake: &SetProtocol,
    info: &ProtocolInfo,
    compression_threshold: i32,
    _disconnect_message: impl Into<String>,
) -> Result<LoginOutcome, ServerError>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    if handshake.protocol_version.0 != info.protocol_version {
        send_disconnect(
            conn,
            format!(
                "Unsupported protocol {}. This server expects {} ({})",
                handshake.protocol_version.0, info.protocol_version, info.mc_version,
            ),
        )
        .await?;
        return Err(ServerError::UnexpectedState("protocol mismatch".into()));
    }

    let login_start = conn.read_packet::<LoginStart>("login start").await?;

    // 压缩协商（在 success 之前）
    if compression_threshold >= 0 {
        conn.send(&to_client::login::compress::Compress {
            threshold: qexed_packet::net_types::VarInt(compression_threshold),
        })
        .await?;
        conn.set_compression(compression_threshold);
    }

    let outcome = LoginOutcome {
        username: login_start.username.clone(),
        uuid: offline_uuid(&login_start.username),
        protocol_version: handshake.protocol_version.0,
    };

    conn.send(&to_client::login::success::Success {
        game_profile: qexed_packet::net_types::GameProfile {
            uuid: outcome.uuid,
            username: outcome.username.clone(),
            properties: Vec::new(),
        },
    })
    .await?;

    conn.read_packet::<LoginAcknowledged>("login acknowledged").await?;
    Ok(outcome)
}

pub async fn send_disconnect<R, W>(
    conn: &mut Connection<R, W>,
    reason: String,
) -> Result<(), ServerError>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    conn.send(&to_client::login::disconnect::Disconnect {
        reason: qexed_packet::net_types::JsonValue(serde_json::json!({
            "text": reason,
        })),
    })
    .await
}