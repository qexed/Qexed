use qexed_packet::PacketCodec;
use qexed_protocol::to_server::status::ping_start::PingStart;
use qexed_tcp_connect::{PacketRead, PacketSend};
use serde_json::json;

pub struct Status {}
impl Status {
    pub fn new(
        r: tokio::sync::mpsc::UnboundedReceiver<(PacketRead, PacketSend)>,
    ) -> tokio::task::JoinHandle<()> {
        let server = Self {};
        tokio::spawn(server.listen(r))
    }
    pub async fn listen(
        self,
        mut r: tokio::sync::mpsc::UnboundedReceiver<(PacketRead, PacketSend)>,
    ) {
        while let Some((r, s)) = r.recv().await {
            tokio::spawn(Status::task(r, s));
        }
    }
    pub async fn task(
        mut packet_read: PacketRead,
        mut packet_write: PacketSend,
    ) -> anyhow::Result<()> {
        // 将秒转换为 Duration

        loop {
            let data = packet_read.read().await?;
            let mut buf: bytes::BytesMut = bytes::BytesMut::new();
            buf.extend_from_slice(&data);
            let mut reader = qexed_packet::PacketReader::new(Box::new(&mut buf));
            let mut id: qexed_packet::net_types::VarInt = Default::default();
            id.deserialize(&mut reader)?;

            match id.0 {
                0x00 => {
                    qexed_tcp_connect::decode_packet::<PingStart>(&mut reader)?;
                    let status = json!({
                        "version": {
                            "name": format!("Qexed {}",qexed_config::MC_VERSION),
                            "protocol": qexed_config::PROTOCOL_VERSION
                        },
                        "players": {
                            "max": 1,
                            "online": 1,
                            "sample": [] // 可以后续添加在线玩家示例
                        },
                        "description": {
                            "text": "测试"
                        },
                        // "favicon": self.favicon, // 可替换为实际favicon
                        "enforcesSecureChat": true,
                        "previewsChat": true
                    });
                    packet_write
                        .send(qexed_protocol::to_client::status::server_info::ServerInfo {
                            response: status,
                        })
                        .await?;
                }
                0x01 => {
                    let pk = qexed_tcp_connect::decode_packet::<
                        qexed_protocol::to_server::status::ping::Ping,
                    >(&mut reader)?;
                    let server_info =
                        qexed_protocol::to_client::status::ping::Ping { time: pk.time };
                    packet_write.send(server_info).await?;
                }
                _ => {}
            }
        }
    }
}
