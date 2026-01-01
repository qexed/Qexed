use std::time::Duration;

use bytes::Bytes;
use qexed_packet::Packet;
use qexed_task::{
    event::task::TaskEvent,
    message::{MessageSender, MessageType},
};

use crate::messages::{LogicCommand, ReadCommand};

#[derive(Debug)]
pub struct ReadTask {
    addr: std::net::SocketAddr,
    packet_read: Option<qexed_tcp_connect::PacketRead>,
}
impl ReadTask {
    pub fn new(addr: std::net::SocketAddr, packet_read: qexed_tcp_connect::PacketRead) -> Self {
        Self {
            addr,
            packet_read: Some(packet_read),
        }
    }
}
#[async_trait::async_trait]
impl<T: Packet + Send + Sync + 'static> TaskEvent<ReadCommand<T>, LogicCommand> for ReadTask {
    async fn event(
        &mut self,
        api: &MessageSender<ReadCommand<T>>,
        manage_api: &MessageSender<LogicCommand>,
        data: ReadCommand<T>,
    ) -> anyhow::Result<bool> {
        match data {
            ReadCommand::Start => {}
            ReadCommand::Packet(mut pk) => {
                if let Some(packet_send) = &mut self.packet_read {
                    if let Some(send) = pk.get_return_send().await? {
                        if let Err(_) = send.send(qexed_tcp_connect::read_one_packet(packet_send).await?){
                            return Ok(true);
                        };
                    }
                };
            }
            ReadCommand::RawPacket(mut pk) => {
                if let Some(packet_send) = &mut self.packet_read {
                    if let Some(send) = pk.get_return_send().await? {
                        if let Err(_) = send.send(packet_send.read().await?){
                            return Ok(true);
                        };
                    }
                }
            }
            ReadCommand::RawPacketSteam(steam) => {
                if let Some(mut packet_send) = self.packet_read.take() {
                    tokio::spawn(async move {
                        loop {
                            steam.send(packet_send.read().await?).await?;
                        }
                        #[warn(unused)]
                        return anyhow::Ok(())
                    });
                }
            }
            ReadCommand::Close => {
                // 由上层逻辑处理触发
                let _ = manage_api.send(LogicCommand::ListenClose(true));
                return Ok(true);
            }
        }
        return Ok(false);
    }
}
