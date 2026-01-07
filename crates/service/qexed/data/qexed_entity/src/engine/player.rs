use std::cmp::min;

use qexed_chunk_message::chunk::ChunkCommand;
use qexed_packet::net_types::{Angle, VarInt};
use qexed_task::{
    event::task::TaskEvent,
    message::{
        MessageSender, MessageType, return_message::ReturnMessage,
        unreturn_message::UnReturnMessage,
    },
};
use uuid::Uuid;

use crate::message::{ManagerCommand, TaskCommand};

#[derive(Debug)]
pub struct PlayerActor {
    pub uuid: Uuid,
    pub username: String,
    pub properties: Vec<qexed_protocol::to_client::login::success::Properties>,
    pub data: qexed_data_serde::entity::living_entity::avatar::player::Player,
    pub entity_id: i32,
    pub chunk_api: Option<MessageSender<UnReturnMessage<qexed_chunk_message::chunk::ChunkCommand>>>,
}
impl PlayerActor {
    pub fn new(player: qexed_player::Player) -> Self {
        Self {
            uuid: player.uuid,
            username: player.username,
            properties: player.properties,
            data: player.data,
            entity_id: -1,
            chunk_api: None,
        }
    }
}

#[async_trait::async_trait]
impl TaskEvent<TaskCommand, ReturnMessage<ManagerCommand>> for PlayerActor {
    async fn event(
        &mut self,
        api: &MessageSender<TaskCommand>,
        manage_api: &MessageSender<ReturnMessage<ManagerCommand>>,
        data: TaskCommand,
    ) -> anyhow::Result<bool> {
        match data {
            TaskCommand::Start => {}
            TaskCommand::GetentityID(send) => {
                let _ = send.send(self.entity_id.clone());
            }
            TaskCommand::AssignID(id) => {
                self.entity_id = id;
            }
            TaskCommand::UpdateChunkApi(chunk_api) => self.chunk_api = Some(chunk_api),
            TaskCommand::GetAddEntityPacket(send) => {
                let _ = send.send(qexed_protocol::to_client::play::add_entity::AddEntity {
                    entity_id: VarInt(self.entity_id.clone()),
                    entity_uuid: self.uuid.clone(),
                    r#type: VarInt(crate::EntityId::Player.into()),
                    x: self.data.avatar.living_entity.entity.pos.x,           // self.data.avatar.living_entity.entity.pos.x,
                    y: self.data.avatar.living_entity.entity.pos.y,           // self.data.avatar.living_entity.entity.pos.y,
                    z: self.data.avatar.living_entity.entity.pos.z,           // self.data.avatar.living_entity.entity.pos.z,
                    pitch: Angle(0), // qexed_packet::net_types::Angle::from_degrees(self.data.avatar.living_entity.entity.rotation.pitch),
                    yaw: Angle(0), //qexed_packet::net_types::Angle::from_degrees(self.data.avatar.living_entity.entity.rotation.yaw),
                    head_yaw: Angle(0), // qexed_packet::net_types::Angle::from_degrees(0.0),
                    data: VarInt(0),
                    velocity_x: 0,
                    velocity_y: 0,
                    velocity_z: 0,
                });
            }
            TaskCommand::Close => {
                if let Some(chunk) = &self.chunk_api {
                    let _ = UnReturnMessage::build(ChunkCommand::PlayerLeave {
                        uuid: self.uuid.clone(),
                        pos: [-22, 9, -19],
                    })
                    .post(chunk)
                    .await;
                }
                ReturnMessage::build(ManagerCommand::TaskClose(self.uuid, self.entity_id))
                    .get(&manage_api)
                    .await?;
                return Ok(true);
            }
            TaskCommand::UpdatePlayerPosRot(move_player_pos_rot) => {
                // 保存旧位置用于计算增量
                let prev_x = self.data.avatar.living_entity.entity.pos.x;
                let prev_y = self.data.avatar.living_entity.entity.pos.y;
                let prev_z = self.data.avatar.living_entity.entity.pos.z;

                // 更新到新位置
                self.data.avatar.living_entity.entity.pos.x = move_player_pos_rot.x;
                self.data.avatar.living_entity.entity.pos.y = move_player_pos_rot.feed_y;
                self.data.avatar.living_entity.entity.pos.z = move_player_pos_rot.z;

                // 按照协议规范计算固定点数增量：current * 4096 - prev * 4096
                let delta_x = (move_player_pos_rot.x * 4096.0 - prev_x * 4096.0).round() as i16;
                let delta_y = (move_player_pos_rot.feed_y * 4096.0 - prev_y * 4096.0).round() as i16;
                let delta_z = (move_player_pos_rot.z * 4096.0 - prev_z * 4096.0).round() as i16;
                if let Some(chunk) = &self.chunk_api {
                    let _ = UnReturnMessage::build(ChunkCommand::UpdatePlayerPosRot(
                        self.uuid.clone(),
                        qexed_protocol::to_client::play::move_entity_pos_rot::MoveEntityPosRot {
                            entity_id: VarInt(self.entity_id),
                            delta_x,
                            delta_y, 
                            delta_z,
                            yaw: Angle::from_minecraft_yaw(move_player_pos_rot.yaw),
                            pitch: Angle::from_minecraft_pitch(move_player_pos_rot.pitch),
                            on_ground: false, // 暂定地面
                        },
                        qexed_protocol::to_client::play::rotate_head::RotateHead {
                            entity_id: VarInt(self.entity_id),
                            head_yaw: Angle::from_minecraft_yaw(move_player_pos_rot.yaw),
                        },
                    ))
                    .post(chunk)
                    .await;
                }
            }
            TaskCommand::UpdatePlayerPos(move_player_pos) => {
                // 保存旧位置用于计算增量
                let prev_x = self.data.avatar.living_entity.entity.pos.x;
                let prev_y = self.data.avatar.living_entity.entity.pos.y;
                let prev_z = self.data.avatar.living_entity.entity.pos.z;

                // 更新到新位置
                self.data.avatar.living_entity.entity.pos.x = move_player_pos.x;
                self.data.avatar.living_entity.entity.pos.y = move_player_pos.feed_y;
                self.data.avatar.living_entity.entity.pos.z = move_player_pos.z;

                // 按照协议规范计算固定点数增量：current * 4096 - prev * 4096
                let delta_x = (move_player_pos.x * 4096.0 - prev_x * 4096.0).round() as i16;
                let delta_y = (move_player_pos.feed_y * 4096.0 - prev_y * 4096.0).round() as i16;
                let delta_z = (move_player_pos.z * 4096.0 - prev_z * 4096.0).round() as i16;

                if let Some(chunk) = &self.chunk_api {
                    let _ = UnReturnMessage::build(ChunkCommand::UpdatePlayerPos(
                        self.uuid.clone(),
                        qexed_protocol::to_client::play::move_entity_pos::MoveEntityPos {
                            entity_id: VarInt(self.entity_id),
                            delta_x,
                            delta_y, 
                            delta_z,
                            on_ground: false, // 暂时在地面
                        },
                    ))
                    .post(chunk)
                    .await;
                }
            }
            TaskCommand::UpdatePlayerRot(move_player_rot) => {
                if let Some(chunk) = &self.chunk_api {
                    let _ = UnReturnMessage::build(ChunkCommand::UpdatePlayerRot(
                        self.uuid.clone(),
                        qexed_protocol::to_client::play::move_entity_rot::MoveEntityRot {
                            entity_id: VarInt(self.entity_id),
                            yaw: Angle::from_minecraft_yaw(move_player_rot.yaw),
                            pitch: Angle::from_minecraft_pitch(move_player_rot.pitch),
                            on_ground: false, // 暂定地面
                        },
                        qexed_protocol::to_client::play::rotate_head::RotateHead {
                            entity_id: VarInt(self.entity_id),
                            head_yaw: Angle::from_minecraft_yaw(move_player_rot.yaw),
                        },
                    ))
                    .post(chunk)
                    .await;
                }
            }
        }
        Ok(false)
    }
}
