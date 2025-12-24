use async_trait::async_trait;
use bytes::Bytes;
use qexed_packet::PacketCodec;
use qexed_player::Player;
use qexed_protocol::to_server::play::{keep_alive::KeepAlive, pong::Pong};
use qexed_task::{
    event::task::TaskEvent,
    message::{
        MessageSender, MessageType, return_message::ReturnMessage,
        unreturn_message::UnReturnMessage,
    },
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use uuid::Uuid;

use crate::message::{ManagerMessage, TaskMessage};

#[derive(Debug)]
pub struct QexedPacketSplitActor {
    uuid: Uuid,
    player: Option<Player>,
    packet_read: Option<UnboundedReceiver<Vec<u8>>>,
    packet_write: Option<UnboundedSender<Bytes>>,
    qexed_ping_api: Option<UnboundedSender<UnReturnMessage<qexed_ping::message::TaskCommand>>>,
    qexed_heartbeat_api:
        Option<UnboundedSender<UnReturnMessage<qexed_heartbeat::message::TaskCommand>>>,
    qexed_chat_api:Option<UnboundedSender<UnReturnMessage<qexed_chat::message::TaskMessage>>>,
    qexed_command_api:Option<UnboundedSender<UnReturnMessage<qexed_command::message::TaskCommand>>>,
}
impl QexedPacketSplitActor {
    pub fn new(uuid: Uuid) -> Self {
        Self {
            uuid,
            player: None,
            packet_read: None,
            packet_write: None,
            qexed_ping_api: None,
            qexed_heartbeat_api: None,
            qexed_chat_api:None,
            qexed_command_api:None,
        }
    }
}
#[async_trait]
impl TaskEvent<ReturnMessage<TaskMessage>, ReturnMessage<ManagerMessage>>
    for QexedPacketSplitActor
{
    async fn event(
        &mut self,
        _api: &MessageSender<ReturnMessage<TaskMessage>>,
        manage_api: &MessageSender<ReturnMessage<ManagerMessage>>,
        mut data: ReturnMessage<TaskMessage>,
    ) -> anyhow::Result<bool> {
        match data.data {
            TaskMessage::Start(
                ref player,
                ref mut unbounded_receiver,
                ref mut unbounded_sender,
                // ref mut qexed_ping_api,
                ref mut qexed_heartbeat_api,
                ref mut qexed_chat_api,
                ref mut qexed_command_api,
            ) => {
                // 玩家进入了服务器
                self.player = Some(player.clone());
                self.packet_read = unbounded_receiver.take();
                self.packet_write = unbounded_sender.take();
                // self.qexed_ping_api = qexed_ping_api.take();
                self.qexed_heartbeat_api = qexed_heartbeat_api.take();
                self.qexed_chat_api = qexed_chat_api.take();
                self.qexed_command_api = qexed_command_api.take();
                let _packet_write = match self.packet_write.clone() {
                    Some(p) => p,
                    None => {
                        if let Some(send) = data.get_return_send().await? {
                            let _ = send.send(data.data);
                        }
                        return Ok(false);
                    }
                };

                if let Some(send) = data.get_return_send().await? {
                    let _ = send.send(data.data);
                }
                return Ok(false);
            }
            TaskMessage::Run => {
                // 运行阶段
                let mut packet_read = match self.packet_read.take() {
                    Some(p) => p,
                    None => {
                        if let Some(send) = data.get_return_send().await? {
                            let _ = send.send(data.data);
                        }
                        return Ok(false);
                    }
                };
                let mut _packet_write = match &self.packet_write.take() {
                    Some(p) => p.clone(),
                    None => {
                        if let Some(send) = data.get_return_send().await? {
                            let _ = send.send(data.data);
                        }
                        return Ok(false);
                    }
                };
                // let qexed_ping_api = match self.qexed_ping_api.take() {
                //     Some(p) => p,
                //     None => {
                //         if let Some(send) = data.get_return_send().await? {
                //             let _ = send.send(data.data);
                //         }
                //         return Ok(false);
                //     }
                // };
                let qexed_heartbeat_api = match self.qexed_heartbeat_api.take() {
                    Some(p) => p,
                    None => {
                        if let Some(send) = data.get_return_send().await? {
                            let _ = send.send(data.data);
                        }
                        return Ok(false);
                    }
                };
                let qexed_chat_api = match self.qexed_chat_api.take() {
                    Some(p) => p,
                    None => {
                        if let Some(send) = data.get_return_send().await? {
                            let _ = send.send(data.data);
                        }
                        return Ok(false);
                    }
                };
                let qexed_command_api = match self.qexed_command_api.take() {
                    Some(p) => p,
                    None => {
                        if let Some(send) = data.get_return_send().await? {
                            let _ = send.send(data.data);
                        }
                        return Ok(false);
                    }
                };
                
                // UnReturnMessage::build(qexed_ping::message::TaskCommand::UpdatePart(qexed_ping::message::Part::Play)).post(&qexed_ping_api).await?;
                // UnReturnMessage::build(qexed_ping::message::TaskCommand::Start).post(&qexed_ping_api).await?;
                while let Some(raw_data) = packet_read.recv().await {
                    let mut buf: bytes::BytesMut = bytes::BytesMut::new();
                    buf.extend_from_slice(&raw_data);
                    let mut reader = qexed_packet::PacketReader::new(Box::new(&mut buf));
                    let mut id: qexed_packet::net_types::VarInt = Default::default();
                    id.deserialize(&mut reader)?;
                    match id.0 {
                        0x06 => {
                            let pk = qexed_tcp_connect::decode_packet::<
                                qexed_protocol::to_server::play::chat_command::ChatCommand,
                            >(&mut reader)?;
                            let _ = UnReturnMessage::build(qexed_command::message::TaskCommand::Command(pk.command)).post(&qexed_command_api).await;
                            
                        }
                        0x08 => {
                            let pk = qexed_tcp_connect::decode_packet::<
                                qexed_protocol::to_server::play::chat_message::ChatMessage,
                            >(&mut reader)?;
                            let _ = UnReturnMessage::build(qexed_chat::message::TaskMessage::ChatEvent(pk)).post(&qexed_chat_api).await;
                        }
                        0x1b => {
                            let pk = qexed_tcp_connect::decode_packet::<KeepAlive>(&mut reader)?;
                            let _ = UnReturnMessage::build(
                                qexed_heartbeat::message::TaskCommand::Heartbeat(pk.keep_alive_id),
                            )
                            .post(&qexed_heartbeat_api)
                            .await;
                        }
                        // 0x2c => {
                        //     let pk =
                        //         qexed_tcp_connect::decode_packet::<Pong>(&mut reader)?;
                        //         let _ = UnReturnMessage::build(qexed_ping::message::TaskCommand::Pong(pk.id)).post(&qexed_ping_api).await;
                        // }
                        _ => {
                            // log::info!("数据包测试:{:?}",raw_data);
                        }
                    }
                }

                if let Some(send) = data.get_return_send().await? {
                    let _ = send.send(data.data);
                }
                return Ok(false);
            }
            TaskMessage::Close => {
                if let Some(api_ping) = &self.qexed_ping_api {
                    let _ = UnReturnMessage::build(qexed_ping::message::TaskCommand::Close)
                        .post(&api_ping)
                        .await;
                }
                // 向父级发送关闭消息
                ReturnMessage::build(ManagerMessage::PlayerClose(self.uuid))
                    .get(manage_api)
                    .await?;
                if let Some(send) = data.get_return_send().await? {
                    let _ = send.send(data.data);
                }
                return Ok(true);
            }
        }
    }
}
// async fn config
