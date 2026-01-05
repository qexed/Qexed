use std::time::Duration;

use bytes::Bytes;
use qexed_packet::Packet;
use qexed_task::{
    event::task::TaskEvent,
    message::{MessageSender, MessageType},
};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::messages::{LogicCommand, ReadCommand};

#[derive(Debug)]
pub struct ReadTask {
    addr: std::net::SocketAddr,
    packet_read: qexed_tcp_connect::PacketRead,
}
impl ReadTask {
    pub fn new(addr: std::net::SocketAddr, packet_read: qexed_tcp_connect::PacketRead) -> Self {
        Self {
            addr,
            packet_read: packet_read,
        }
    }
}
#[async_trait::async_trait]
impl TaskEvent<ReadCommand, LogicCommand> for ReadTask {
    async fn event(
        &mut self,
        api: &MessageSender<ReadCommand>,
        manage_api: &MessageSender<LogicCommand>,
        data: ReadCommand,
    ) -> anyhow::Result<bool> {
        match data {
            ReadCommand::Start => {}
            ReadCommand::RawPacket(mut pk) => {
                if let Err(_) = pk.send(self.packet_read.read().await?){
                    return Ok(true);
                };
            }
            ReadCommand::SetCompression(is_use,finish)=>{
                self.packet_read.set_compression(is_use);
                let _ = finish.send(());
            }
            // ReadCommand::RawPacketSteam(steam) => {
            //     if let Some(mut packet_send) = self.packet_read.take() {
            //         tokio::spawn(async move {
            //             loop {
            //                 steam.send(packet_send.read().await?).await?;
            //             }
            //             #[warn(unused)]
            //             return anyhow::Ok(())
            //         });
            //     }
            // }
            ReadCommand::Close => {
                // 由上层逻辑处理触发
                let _ = manage_api.send(LogicCommand::ListenClose(true));
                return Ok(true);
            }
        }
        return Ok(false);
    }
}


pub struct TaskFinish {
    api: MessageSender<ReadCommand>,
    manage_api: MessageSender<LogicCommand>,
    other: ReadTask,
    receiver: Option<UnboundedReceiver<ReadCommand>>,
}
impl TaskFinish
{
    pub fn new(
        manage_api: MessageSender<LogicCommand>,
        data: ReadTask,
    ) -> (Self, MessageSender<ReadCommand>) {
        let (w, r) = tokio::sync::mpsc::unbounded_channel();
        (
            Self {
                api: w.clone(),
                manage_api: manage_api,
                other: data,
                receiver: Some(r),
            },
            w,
        )
    }
    // 请注意:下面的所有权转移并不是失误,是刻意的设计
    pub async fn run(self) -> anyhow::Result<()> {
        tokio::spawn(self.listen());
        Ok(())
    }
    async fn listen(mut self) -> anyhow::Result<()> {
        let mut receiver = self
            .receiver
            .take()
            .ok_or_else(|| anyhow::anyhow!("接收管道不存在"))?;
        let api = self.api;
        let manage_api = self.manage_api;
        while let Some(data) = receiver.recv().await {
            // 这里我们后面修改来实现具体业务逻辑
            if let Ok(is_true)=self.other.event(&api, &manage_api, data).await {
                if !is_true{
                    continue;
                }
            }
            let _ = self.other.event(&api, &manage_api, ReadCommand::Close).await;
            receiver.close();
        }
        Ok(())
    }
}
