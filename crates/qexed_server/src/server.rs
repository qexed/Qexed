//! 服务器启动与单连接分发。

use std::sync::Arc;

use bytes::BytesMut;
use qexed_auth::Authenticator;
use qexed_config::Config;
use qexed_packet::net_types::VarInt;
use qexed_packet::{Packet, PacketReader};
use qexed_protocol::to_server::handshaking::set_protocol::SetProtocol;
use tokio::sync::watch;

use crate::codec::read_packet_or_timeout;
use crate::config;
use crate::consts::HANDSHAKE_TIMEOUT;
use crate::error::ServerError;
use crate::handle::{ServerContext, ServerHandle};
use crate::{login, status};

/// 启动服务器：绑端口、起 accept loop、返回关闭句柄。
pub async fn init() -> Result<ServerHandle, ServerError> {
    let config = config::ServerConfig::load_and_create_default(true)?;
    let listener: tokio::net::TcpListener = qexed_tcp_connect::bind(&config.bind).await?;

    log::info!(
        "server listening on {} (online_mode={})",
        config.bind,
        config.online_mode,
    );

    // PLAY / Chat 是 PLAY 阶段的行为 crate：启动时加载一次
    let play_config = Arc::new(qexed_play::config::PlayConfig::load_and_create_default(
        true,
    )?);
    let chat_config = Arc::new(qexed_chat::config::ChatConfig::load_and_create_default(
        true,
    )?);
    // AI 推理网关（内嵌 EchoEngine / 远端 HTTP）：
    // - 服务启动失败 = 硬失败（本次会话的核心目标就是 jev 接入，静默降级只会把问题藏到运行期）；
    // - 权重未接入（Echo 兜底）= 警告：关键词规则仍生效，语义判定（模型理解）不会生效。
    if let Err(err) = qexed_jev::JevService::init_global().await {
        return Err(ServerError::Protocol(format!(
            "jev service failed to start: {err}"
        )));
    }
    if qexed_jev::config::JevConfig::load_and_create_default(false)
        .ok()
        .is_some_and(|cfg| cfg.embedded.model_path.trim().is_empty())
    {
        log::warn!(
            "jev embedded.model_path is empty: keyword rules active, \
             semantic moderation (model-based) is NOT active"
        );
    }

    // ── 插件系统：扫描 plugins/ 目录，加载 dll/so/dylib 并启用 ──
    // 目录固定 run/plugins（与 config/ 同级约定）；失败仅警告降级，不阻断开服。
    qexed_plugin::set_plugins_path("plugins")
        .map_err(ServerError::Plugin)?;
    if let Err(err) = qexed_plugin::init().await {
        log::warn!("plugin system disabled: {err}");
    }

    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let handle = ServerHandle::from_sender(shutdown_tx.clone());

    let context = ServerContext {
        config: Arc::new(config),
        authenticator: Arc::new(Authenticator::new()?),
        play_config,
        chat_config,
        shutdown: handle.clone(),
    };

    let broadcast_tx = shutdown_tx.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        break;
                    }
                }
                accepted = listener.accept() => {
                    let (stream, peer) = match accepted {
                        Ok(pair) => pair,
                        Err(err) => {
                            log::warn!("accept failed: {err}");
                            continue;
                        }
                    };
                    let ctx = context.clone();
                    tokio::spawn(async move {
                        if let Err(err) = handle_connection(stream, peer, ctx).await {
                            log::debug!("connection ended: peer={peer}, error={err:#}");
                        }
                    });
                }
            }
        }
        log::info!("server accept loop exited");
        // 关闭路径：逆序 on_disable 并卸载全部插件动态库。
        qexed_plugin::shutdown().await;
        // accept loop 因任何原因退出时，也广播一次，避免外部 wait() 挂住。
        let _ = broadcast_tx.send(true);
    });

    Ok(handle)
}

/// 单连接：握手 → 分发（Status / Login）。
async fn handle_connection(
    stream: tokio::net::TcpStream,
    peer: std::net::SocketAddr,
    context: ServerContext,
) -> Result<(), ServerError> {
    let _ = stream.set_nodelay(true);

    let (reader, writer) = tokio::io::split(stream);
    let mut packets = qexed_tcp_connect::PacketStream::new(reader);
    let mut sink = qexed_tcp_connect::PacketSink::new(writer);

    // 1. 握手（带超时）
    let maybe_payload = read_packet_or_timeout(&mut packets, HANDSHAKE_TIMEOUT).await?;
    let Some(payload) = maybe_payload else {
        log::debug!("connection closed before handshake: peer={peer}");
        return Ok(());
    };
    let handshake = decode_handshake(payload)?;

    log::debug!(
        "handshake: peer={peer}, protocol={}, host={}, port={}, next_state={}",
        handshake.protocol_version.0,
        handshake.server_host,
        handshake.server_port,
        handshake.next_state.0,
    );

    // 2. 分发
    match handshake.next_state.0 {
        1 => status::handle_status(&mut packets, &mut sink, &context).await?,
        2 => {
            login::handle_login(handshake, &mut packets, &mut sink, &context, peer.ip()).await?;
        }
        other => log::warn!("unsupported next_state from {peer}: {other}"),
    }

    let _ = sink.flush().await;
    Ok(())
}

fn decode_handshake(mut payload: BytesMut) -> Result<SetProtocol, ServerError> {
    let mut reader = PacketReader::new(&mut payload);
    let packet_id: VarInt = reader
        .deserialize()
        .map_err(|err| ServerError::Protocol(err.to_string()))?;
    if packet_id.0 != SetProtocol::ID {
        return Err(ServerError::Protocol(format!(
            "unexpected handshake packet id: expected {}, got {}",
            SetProtocol::ID,
            packet_id.0
        )));
    }
    let mut handshake = SetProtocol::default();
    handshake
        .deserialize(&mut reader)
        .map_err(|err| ServerError::Protocol(err.to_string()))?;
    Ok(handshake)
}