use log::error;
use qexed_packet::PacketCodec;
use qexed_protocol::to_server::status::ping_start::PingStart;
use qexed_tcp_connect::{PacketRead, PacketSend};
use rand::seq::SliceRandom;
use serde_json::json;
use std::sync::{Arc, atomic::AtomicI32};

pub struct Status {
    pub max_player: i32,
    pub players: Arc<AtomicI32>,
    pub motd: Vec<String>,
    pub favicon: String,
}

impl Status {
    pub fn new(
        r: tokio::sync::mpsc::UnboundedReceiver<(PacketRead, PacketSend)>,
        players: Arc<AtomicI32>,
        max_player: i32,
        motd: Vec<String>,
        favicon: String,
    ) -> tokio::task::JoinHandle<()> {
        let server = Self {
            max_player,
            motd,
            favicon,
            players,
        };
        tokio::spawn(server.listen(r))
    }

    // 修复点1：将串行处理改为每连接独立任务，避免单线程阻塞
    // 修复点2：所有 ? 替换为显式错误处理，任务内永不 panic 退出
    pub async fn listen(
        self,
        mut r: tokio::sync::mpsc::UnboundedReceiver<(PacketRead, PacketSend)>,
    ) {
        while let Some((packet_read, packet_write)) = r.recv().await {
            // 为每个连接生成独立的任务，实现并发处理
            let server = self.clone(); // 需要为 Status 实现 Clone
            tokio::spawn(async move {
                if let Err(e) = server.handle_connection(packet_read, packet_write).await {
                    error!("Failed to handle status connection: {}", e);
                }
            });
        }
    }

    // 将单个连接的处理逻辑分离，便于错误处理
    async fn handle_connection(
        &self,
        mut packet_read: PacketRead,
        mut packet_write: PacketSend,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        loop {
            // 使用 match 处理读取错误：连接关闭则正常退出循环
            let data = match packet_read.read().await {
                Ok(d) => d,
                Err(e) => {
                    log::debug!("Status connection closed: {}", e);
                    return Ok(()); // 连接已关闭，任务正常结束
                }
            };
            let mut buf: bytes::BytesMut = bytes::BytesMut::new();
            buf.extend_from_slice(&data);
            let mut reader = qexed_packet::PacketReader::new(Box::new(&mut buf));
            let mut id: qexed_packet::net_types::VarInt = Default::default();
            id.deserialize(&mut reader)?;

            match id.0 {
                0x00 => {
                    // 解码 PingStart，忽略错误包
                    if let Err(e) = qexed_tcp_connect::decode_packet::<PingStart>(&mut reader) {
                        error!("Failed to decode PingStart: {}", e);
                        continue;
                    }

                    // 修复点3：motd 使用随机选择的值，而非整个 Vec
                    let motd = self
                        .motd
                        .choose(&mut rand::thread_rng())
                        .cloned()
                        .unwrap_or_else(|| "A Minecraft Server".to_string());

                    let status = json!({
                        "version": {
                            "name": format!("Qexed {}", qexed_config::MC_VERSION),
                            "protocol": qexed_config::PROTOCOL_VERSION
                        },
                        "players": {
                            "max": self.max_player,
                            "online": self.players.load(std::sync::atomic::Ordering::SeqCst),
                            "sample": []
                        },
                        "description": {
                            "text": motd  // ✅ 使用随机文本
                        },
                        "favicon": self.favicon,
                        "enforcesSecureChat": true,
                        "previewsChat": true
                    });

                    // 发送响应，若发送失败则记录并继续
                    if let Err(e) = packet_write
                        .send(qexed_protocol::to_client::status::server_info::ServerInfo {
                            response: status,
                        })
                        .await
                    {
                        error!("Failed to send ServerInfo: {}", e);
                    }
                }
                0x01 => {
                    // 解码 Ping
                    let pk = match qexed_tcp_connect::decode_packet::<
                        qexed_protocol::to_server::status::ping::Ping,
                    >(&mut reader)
                    {
                        Ok(p) => p,
                        Err(e) => {
                            error!("Failed to decode Ping: {}", e);
                            continue;
                        }
                    };

                    let server_info =
                        qexed_protocol::to_client::status::ping::Ping { time: pk.time };
                    if let Err(e) = packet_write.send(server_info).await {
                        error!("Failed to send Ping response: {}", e);
                    }
                }
                _ => {
                    // 未知包ID，忽略
                    log::debug!("Unknown packet ID: {}", id.0);
                }
            }
        }
    }
}

// 为 Status 实现 Clone，以便在任务间共享
impl Clone for Status {
    fn clone(&self) -> Self {
        Self {
            max_player: self.max_player,
            players: Arc::clone(&self.players),
            motd: self.motd.clone(),
            favicon: self.favicon.clone(),
        }
    }
}
