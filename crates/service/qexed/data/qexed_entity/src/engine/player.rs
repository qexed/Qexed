use std::cmp::min;

use qexed_packet::net_types::{Angle, VarInt};
use qexed_task::{event::task::TaskEvent, message::{MessageSender, MessageType, return_message::ReturnMessage}};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use uuid::Uuid;

use crate::message::{ManagerCommand, TaskCommand};

#[derive(Debug)]
pub struct PlayerActor{
    pub uuid: Uuid,
    pub username: String,
    pub properties: Vec<qexed_protocol::to_client::login::success::Properties>,
    pub data: qexed_data_serde::entity::living_entity::avatar::player::Player,
    pub entity_id:i32,
}
impl PlayerActor {
    pub fn new(player:qexed_player::Player)->Self{
        Self {
            uuid:player.uuid,
            username:player.username,
            properties:player.properties,
            data:player.data,
            entity_id:-1,
        }
    }
}

#[async_trait::async_trait]
impl TaskEvent<TaskCommand,ReturnMessage<ManagerCommand>>
    for PlayerActor
{
    async fn event(
        &mut self,
        api: &MessageSender<TaskCommand>,
        manage_api: &MessageSender<ReturnMessage<ManagerCommand>>,
        data: TaskCommand,
    ) -> anyhow::Result<bool> {
        match data {
            TaskCommand::Start => {

            },
            TaskCommand::GetentityID(send) => {
                let _ = send.send(self.entity_id.clone());
            },
            TaskCommand::AssignID(id)=>{
                self.entity_id = id;
            },
            TaskCommand::GetAddEntityPacket(send) => {
                let _ = send.send(qexed_protocol::to_client::play::add_entity::AddEntity {
                    entity_id: VarInt(self.entity_id.clone()),
                    entity_uuid: self.uuid.clone(),
                    r#type:VarInt(crate::EntityId::Player.into()),
                    x: -22.0,// self.data.avatar.living_entity.entity.pos.x,
                    y: 9.0,// self.data.avatar.living_entity.entity.pos.y,
                    z: -19.0,// self.data.avatar.living_entity.entity.pos.z,
                    pitch:Angle(0),// qexed_packet::net_types::Angle::from_degrees(self.data.avatar.living_entity.entity.rotation.pitch),
                    yaw: Angle(0),//qexed_packet::net_types::Angle::from_degrees(self.data.avatar.living_entity.entity.rotation.yaw),
                    head_yaw:Angle(0),// qexed_packet::net_types::Angle::from_degrees(0.0),
                    data: VarInt(0),
                    velocity_x: 0,
                    velocity_y: 0,
                    velocity_z: 0,
                });
            },
            TaskCommand::Close =>{
                ReturnMessage::build(
                    ManagerCommand::TaskClose(self.uuid,self.entity_id)
                ).get(&manage_api).await?;
                return Ok(true);
            }
        }
        Ok(false)
    }
}