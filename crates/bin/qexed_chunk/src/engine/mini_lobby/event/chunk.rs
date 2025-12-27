use std::{collections::HashMap, path::PathBuf};

use qexed_task::message::{MessageSender, unreturn_message::UnReturnMessage};
use uuid::Uuid;

use crate::{data_type::direction::DirectionMap, message::chunk::ChunkCommand};
#[derive(Debug)]
pub struct ChunkTask{
    // 世界配置文件
    pub config: qexed_config::app::qexed_chunk::engine::mini_lobby::MiniLobbyConfig,
    // 世界目录
    world_root: PathBuf,
    // 世界uuid
    world_uuid: uuid::Uuid,
    // 区块数据
    pub chunk: qexed_region::chunk::nbt::Chunk,
    // 区块坐标 pos
    pub pos:[i64;2],   
    // 相邻区块
    direction_chunk:DirectionMap<MessageSender<UnReturnMessage<ChunkCommand>>>,
    // 跨维度对应区块API
    cross_dimension_counterpart_apis: HashMap<Uuid, MessageSender<UnReturnMessage<ChunkCommand>>>,
    // 当前区块直属玩家API管道
}
impl ChunkTask {
    pub fn new(
    // 世界配置文件
        config: qexed_config::app::qexed_chunk::engine::mini_lobby::MiniLobbyConfig,
        world_root: PathBuf,
        world_uuid: uuid::Uuid,
        pos:[i64;2],
        chunk:qexed_region::chunk::nbt::Chunk
    ) -> Self {
        Self {
            config,
            world_root,
            world_uuid,
            pos,
            chunk,
            direction_chunk:Default::default(),
            cross_dimension_counterpart_apis:Default::default(),
        }
    }
    
}
