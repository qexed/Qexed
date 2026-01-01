use std::time::Duration;

use bytes::Bytes;
use qexed_packet::Packet;
use qexed_task::{event::task::TaskEvent, message::{MessageSender, MessageType, unreturn_message::UnReturnMessage}};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::{logic_task::LogicTask, messages::{LogicCommand, WriteCommand}};

#[derive(Debug)]
pub struct WriteTask {
    addr: std::net::SocketAddr,
    packet_write: qexed_tcp_connect::PacketSend,
}
impl WriteTask {
    pub fn new(
        addr: std::net::SocketAddr,
        packet_write: qexed_tcp_connect::PacketSend,
    )->Self{
        Self { addr,packet_write }
    }
    
}
#[async_trait::async_trait]
impl TaskEvent<WriteCommand, LogicCommand> for WriteTask {
    async fn event(
        &mut self,
        api: &MessageSender<WriteCommand>,
        manage_api: &MessageSender<LogicCommand>,
        data: WriteCommand,
    ) -> anyhow::Result<bool> {
        match data {
            WriteCommand::Start => {
            },
            WriteCommand::RawPacket(bytes) => {
                match self.packet_write.send_raw(bytes).await {
                    Ok(_)=>{
                        return Ok(false)
                    }
                    Err(err)=>{
                        log::error!("[{}] 数据包发送失败,疑似连接被断开",self.addr.ip());
                        log::error!("[{}] {}",self.addr.ip(),err);
                        return Ok(true);
                    }
                }
            },
            WriteCommand::Close => {
                // 由上层逻辑处理触发
                let _ = manage_api.send(LogicCommand::ListenClose(false));
                let _ = self.packet_write.shutdown().await;
            },
        }
        return Ok(false);
    }
}

pub struct TaskFinish {
    api: MessageSender<WriteCommand>,
    manage_api: MessageSender<LogicCommand>,
    other: WriteTask,
    receiver: Option<UnboundedReceiver<WriteCommand>>,
}
impl TaskFinish
{
    pub fn new(
        manage_api: MessageSender<LogicCommand>,
        data: WriteTask,
    ) -> (Self, MessageSender<WriteCommand>) {
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
            // 这里我们后面修改来实现具体业务逻辑
            if let Ok(is_true)=self.other.event(&api, &manage_api, data).await {
                if !is_true{
                    continue;
                }
            }
            let _ = self.other.event(&api, &manage_api, WriteCommand::Close).await;
            receiver.close();
        }
        Ok(())
    }
}
