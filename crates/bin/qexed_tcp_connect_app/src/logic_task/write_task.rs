use std::time::Duration;

use bytes::Bytes;
use qexed_packet::Packet;
use qexed_task::{event::task::TaskEvent, message::{MessageSender, MessageType, unreturn_message::UnReturnMessage}};

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
impl<T: Packet + Send + Sync+ 'static> TaskEvent<WriteCommand<T>, LogicCommand> for WriteTask {
    async fn event(
        &mut self,
        api: &MessageSender<WriteCommand<T>>,
        manage_api: &MessageSender<LogicCommand>,
        data: WriteCommand<T>,
    ) -> anyhow::Result<bool> {
        match data {
            WriteCommand::Start => {
            },
            WriteCommand::Packet(pk) => {
                match self.packet_write.send(pk).await {
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