use qexed_protocol::to_server::play::move_player::{MovePlayerPos, MovePlayerPosRot, MovePlayerRot};
use tokio::sync::{mpsc::UnboundedSender, oneshot};
use uuid::Uuid;

#[derive(Debug)]
pub enum ChunkCommand{
    Init,
    PlayerJoin{ // 玩家进入区块
        uuid:Uuid,
        pos:[i64;3],
        packet_send:UnboundedSender<bytes::Bytes>,
    },
    UpdatePlayerPos(Uuid,qexed_protocol::to_client::play::move_entity_pos::MoveEntityPos),
    UpdatePlayerRot(Uuid,qexed_protocol::to_client::play::move_entity_rot::MoveEntityRot,qexed_protocol::to_client::play::rotate_head::RotateHead),
    UpdatePlayerPosRot(Uuid,qexed_protocol::to_client::play::move_entity_pos_rot::MoveEntityPosRot,qexed_protocol::to_client::play::rotate_head::RotateHead),
    // PlayerMove{ // 玩家转移区块(换位置了)
    //     uuid:Uuid,
    //     pos:[i64;3],
    //     packet_send:UnboundedSender<bytes::Bytes>
    // },
    PlayerLeave{ // 玩家离开区块
        uuid:Uuid,
        pos:[i64;3],
    },
    // 区块强制关闭命令(要求同步区块数据)
    CloseCommand{
        result:oneshot::Sender<ChunkData>,
    },
}

#[derive(Debug,Default)]
pub struct ChunkData{
    pub data:Option<Vec<u8>>,
    pub entities:Option<Vec<u8>>,
    pub poi:Option<Vec<u8>>,
    pub region:Option<Vec<u8>>,
}