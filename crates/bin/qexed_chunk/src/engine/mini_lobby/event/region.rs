use std::{collections::HashMap, path::{Path, PathBuf}};

use dashmap::DashMap;
use qexed_task::message::{MessageSender, MessageType, unreturn_message::UnReturnMessage};
use uuid::Uuid;

use crate::{data_type::direction::DirectionMap, engine::mini_lobby::event::chunk::ChunkTask, message::{chunk::ChunkCommand, region::RegionCommand, world::WorldCommand}};

#[derive(Debug)]
pub struct RegionManage{
    // 配置
    pub config: qexed_config::app::qexed_chunk::engine::mini_lobby::MiniLobbyConfig,
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
        config: qexed_config::app::qexed_chunk::engine::mini_lobby::MiniLobbyConfig,
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
    pub async fn init(&self,
        api: &MessageSender<UnReturnMessage<RegionCommand>>,
        task_map: &DashMap<[i64; 2], MessageSender<UnReturnMessage<ChunkCommand>>>)->anyhow::Result<()>{
            let chunks = self.get_chunks_in_region();
            // 读取地图
            let path = self.world_root.join("region").join(format!("r.{}.{}.mca",self.pos[0].clone(),self.pos[1].clone()));
            let anvil = match qexed_region::region::anvil::Anvil::from_file(&path){
                Ok(anvil)=>anvil,
                Err(_err)=>{
                    log::debug!("区域不存在");
                    qexed_region::region::anvil::Anvil::new(&path, self.pos[0].clone() as i32,self.pos[1].clone() as i32)?
                }
            };
            for i in chunks{
                let chunk = anvil.get_chunk_data(i[0] as i32,i[1] as i32);
                // 若无数据，则空区块
                log::info!("{:?}",chunk);
                
                // pass

                let (chunk_task, chunk_sender) =
                    qexed_task::task::task::Task::new(api.clone(),ChunkTask::new(self.config.clone(),self.world_root.clone(), self.world_uuid.clone(), i.clone()));

                chunk_task.run().await?;
                chunk_sender.send(UnReturnMessage::build(ChunkCommand::Init{data:None}))?;
                task_map.insert(i, chunk_sender);
            }
            // log::info!("{:?}",chunks);
            Ok(())
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
    /// 计算两个坐标间属于当前区域的区块列表
    /// 输出：属于该区域且在当前坐标范围内的区块列表
    pub fn get_chunks_in_region(&self) -> Vec<[i64; 2]> {
        // 获取地图范围
        let map_range = self.config.map_range;
        
        // 计算区块范围
        let chunk_x_min = map_range[0][0].div_euclid(16);
        let chunk_z_min = map_range[0][1].div_euclid(16);
        let chunk_x_max = map_range[1][0].div_euclid(16);
        let chunk_z_max = map_range[1][1].div_euclid(16);
        
        // 计算目标区域的区块范围
        let region_x = self.pos[0];
        let region_z = self.pos[1];
        
        // 该区域包含的区块范围
        let region_chunk_x_min = region_x * 32;
        let region_chunk_z_min = region_z * 32;
        let region_chunk_x_max = region_chunk_x_min + 31; // 包含 0-31
        let region_chunk_z_max = region_chunk_z_min + 31;
        
        // 计算交集范围
        let intersect_x_min = chunk_x_min.max(region_chunk_x_min);
        let intersect_z_min = chunk_z_min.max(region_chunk_z_min);
        let intersect_x_max = chunk_x_max.min(region_chunk_x_max);
        let intersect_z_max = chunk_z_max.min(region_chunk_z_max);
        
        // 收集交集范围内的区块
        let mut chunks_in_region = Vec::new();
        
        for x in intersect_x_min..=intersect_x_max {
            for z in intersect_z_min..=intersect_z_max {
                chunks_in_region.push([x, z]);
            }
        }
        
        chunks_in_region
    }
}
