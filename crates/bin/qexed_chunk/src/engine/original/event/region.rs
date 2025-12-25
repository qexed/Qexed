use std::{collections::HashMap, path::{Path, PathBuf}};

use qexed_task::message::{MessageSender, unreturn_message::UnReturnMessage};
use uuid::Uuid;

use crate::{data_type::direction::DirectionMap,message::{region::RegionCommand, world::WorldCommand}};

#[derive(Debug)]
pub struct RegionManage{
    // 世界配置文件
    pub config: qexed_config::app::qexed_chunk::world::World,
    // 世界目录
    pub world_root: PathBuf,
    // 世界uuid
    pub world_uuid: uuid::Uuid,
    // 区域坐标 pos
    pub pos:[i64;2],
    // 相邻区域
    pub direction_region:DirectionMap<MessageSender<UnReturnMessage<RegionCommand>>>,
    // 世界api
    pub master_api:MessageSender<UnReturnMessage<WorldCommand>>,
}
impl RegionManage {
    pub fn new(
        config: qexed_config::app::qexed_chunk::world::World,
        world_root: PathBuf,
        world_uuid: uuid::Uuid,
        pos:[i64;2],
        master_api:MessageSender<UnReturnMessage<WorldCommand>>,
    ) -> Self {
        Self {
            config,
            world_root,
            world_uuid,
            pos,
            direction_region:Default::default(),
            master_api,
        }
    }
    // 计算给定的chunk坐标是否属于指定的region
    pub fn is_chunk_in_region(&self,chunk_pos: [i64; 2]) -> bool {
        // 每个region包含的chunk数量（通常是32x32）
        const CHUNKS_PER_REGION: i64 = 32;

        // 计算chunk所在的region坐标
        let region_x = chunk_pos[0].div_euclid(CHUNKS_PER_REGION);
        let region_z = chunk_pos[1].div_euclid(CHUNKS_PER_REGION);

        // 判断是否匹配
        region_x == self.pos[0] && region_z == self.pos[1]
    }
}
