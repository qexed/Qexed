
use async_trait::async_trait;
use qexed_packet::net_types::{Bitset, VarInt};
use qexed_protocol::to_client::play;
use qexed_task::{
    event::task::TaskEvent,
    message::{
        MessageSender, MessageType, return_message::ReturnMessage,
        unreturn_message::UnReturnMessage,
    },
};

use crate::{
    engine::mini_lobby::event::chunk::ChunkTask,
    message::{
        chunk::{ChunkCommand, ChunkData},
        region::RegionCommand,
    },
};
use qexed_tcp_connect::PacketSend;

#[async_trait]
impl TaskEvent<UnReturnMessage<ChunkCommand>, UnReturnMessage<RegionCommand>> for ChunkTask {
    async fn event(
        &mut self,
        api: &MessageSender<UnReturnMessage<ChunkCommand>>,
        manage_api: &MessageSender<UnReturnMessage<RegionCommand>>,
        data: UnReturnMessage<ChunkCommand>,
    ) -> anyhow::Result<bool> {
        match data.data {
            ChunkCommand::Init => {
                // 初始化函数暂时没写
                self.init().await.unwrap();
            }
            ChunkCommand::PlayerJoin {
                uuid,
                pos,
                packet_send,
            } => {
                if let Some(pk) = &self.chunk_packet {
                    packet_send.send(pk.clone())?;
                }
                // 玩家坐标信息更新

                // 所有区块均已加载，直接发送即可、
                let join_pos_chunk =
                    self.player_pos_to_chunk_pos([pos[0] as i32, pos[1] as i32, pos[2] as i32]);


                // 判断是否是当前区块
                if self.isinthischunk(join_pos_chunk) {
                    // 构建 SetChunkCacheCenter 数据包
                    packet_send.send(PacketSend::build_send_packet(qexed_protocol::to_client::play::update_view_position::UpdateViewPosition{
                        chunk_x:qexed_packet::net_types::VarInt(join_pos_chunk[0]),
                        chunk_z:qexed_packet::net_types::VarInt(join_pos_chunk[1]),
                    }).await?)?;
                    packet_send.send(
                        PacketSend::build_send_packet(
                            qexed_protocol::to_client::play::position::Position {
                                teleport_id: qexed_packet::net_types::VarInt(0),
                                x: pos[0] as f64,
                                y: pos[1] as f64,
                                z: pos[2] as f64,
                                dx: 0.0,
                                dy: 0.0,
                                dz: 0.0,
                                yaw: self.config.join_yaw.clone(),
                                pitch: self.config.join_pitch.clone(),
                                flags: Default::default(),
                            },
                        )
                        .await?,
                    )?;
                    // 保存玩家对象到当前区块
                    if let qexed_entity::message::ManagerCommand::GetEntity(_, Some(player_api)) =
                        ReturnMessage::build(qexed_entity::message::ManagerCommand::GetEntity(
                            uuid.clone(),
                            None,
                        ))
                        .get(&self.qexed_entity_api)
                        .await?
                    {
                        let (s,r) = tokio::sync::oneshot::channel();
                        let _ = player_api.send(qexed_entity::message::TaskCommand::GetentityID(s));
                        let entity_id = r.await?;
                        let (s,r) = tokio::sync::oneshot::channel();
                        let _ = player_api.send(qexed_entity::message::TaskCommand::GetAddEntityPacket(s));
                        let pk = PacketSend::build_send_packet(
                            r.await?
                        )
                        .await?;
                        for i in &self.now_chunk_player {
                            let _ = i.value().1.send(pk.clone());
                            let (s,r) = tokio::sync::oneshot::channel();
                            let _ = i.value().0.send(qexed_entity::message::TaskCommand::GetAddEntityPacket(s));
                            let pk2 = PacketSend::build_send_packet(
                                r.await?
                            ).await?;
                            let _ = packet_send.send(pk2);
                        }
                        let _ = player_api.send(qexed_entity::message::TaskCommand::UpdateChunkApi(api.clone()));
                        self.now_chunk_player
                            .insert(uuid.clone(), (player_api, packet_send.clone(),entity_id));
                        
                    }
                }
            } 
            ChunkCommand::UpdatePlayerPos(uuid, move_player_pos) => {
                let pk = PacketSend::build_send_packet(
                    move_player_pos
                ).await?;
                for i in &self.now_chunk_player {
                    let _ = i.value().1.send(pk.clone());
                }
            },
            ChunkCommand::UpdatePlayerRot(uuid, move_player_rot,rotate_head) => {
                let pk = PacketSend::build_send_packet(
                    move_player_rot
                ).await?;
                let head_pk = PacketSend::build_send_packet(
                    rotate_head
                ).await?;
                for i in &self.now_chunk_player {
                    let _ = i.value().1.send(pk.clone());
                    let _ = i.value().1.send(head_pk.clone());
                }
                // log::info!("[{:?}] 移动数据包:{:?}",uuid,move_player_rot);
            },
            ChunkCommand::UpdatePlayerPosRot(uuid, move_player_pos_rot,rotate_head) => {
                let pk = PacketSend::build_send_packet(
                    move_player_pos_rot
                ).await?;
                let head_pk = PacketSend::build_send_packet(
                    rotate_head
                ).await?;
                for i in &self.now_chunk_player {
                    let _ = i.value().1.send(pk.clone());
                    let _ = i.value().1.send(head_pk.clone());
                }
            },
            ChunkCommand::PlayerLeave { uuid, pos } =>{
                log::info!("玩家离开区块测试:{},坐标:{:?}",uuid,pos);
                if let Some(player) = self.now_chunk_player.remove(&uuid){
                    let player_leave_packet = PacketSend::build_send_packet(qexed_protocol::to_client::play::remove_entities::RemoveEntities{
                        uuids:vec![VarInt(player.1.2)],
                    }).await?;
                    for i in &self.now_chunk_player{
                        let _ = i.value().1.send(
                            player_leave_packet.clone()
                        );
                    }
                }
            }
            ChunkCommand::CloseCommand { result } => {
                // 暂时没写数据读写
                result.send(ChunkData::default());
            }
        }
        Ok(false)
    }
}
