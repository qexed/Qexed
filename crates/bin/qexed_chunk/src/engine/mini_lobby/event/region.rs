use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use dashmap::DashMap;
use qexed_task::message::{MessageSender, MessageType, unreturn_message::UnReturnMessage};
use uuid::Uuid;

use crate::{
    data_type::direction::DirectionMap,
    engine::mini_lobby::event::chunk::ChunkTask,
    message::{chunk::ChunkCommand, region::RegionCommand, world::WorldCommand},
};

#[derive(Debug)]
pub struct RegionManage {
    // 配置
    pub config: qexed_config::app::qexed_chunk::engine::mini_lobby::MiniLobbyConfig,
    // 世界目录
    pub world_root: PathBuf,
    // 世界uuid
    pub world_uuid: uuid::Uuid,
    // 区域坐标 pos
    pub pos: [i64; 2],
    // 相邻区域
    pub direction_region: DirectionMap<MessageSender<UnReturnMessage<RegionCommand>>>,
    // 世界api
    pub master_api: MessageSender<UnReturnMessage<WorldCommand>>,
}
impl RegionManage {
    pub fn new(
        config: qexed_config::app::qexed_chunk::engine::mini_lobby::MiniLobbyConfig,
        world_root: PathBuf,
        world_uuid: uuid::Uuid,
        pos: [i64; 2],
        master_api: MessageSender<UnReturnMessage<WorldCommand>>,
    ) -> Self {
        Self {
            config,
            world_root,
            world_uuid,
            pos,
            direction_region: Default::default(),
            master_api,
        }
    }
    pub async fn init(
        &self,
        api: &MessageSender<UnReturnMessage<RegionCommand>>,
        task_map: &DashMap<[i64; 2], MessageSender<UnReturnMessage<ChunkCommand>>>,
    ) -> anyhow::Result<()> {
        let chunks = self.get_chunks_in_region();
        // 读取地图
        let path = self.world_root.join("region").join(format!(
            "r.{}.{}.mca",
            self.pos[0].clone(),
            self.pos[1].clone()
        ));
        let anvil = match qexed_region::region::anvil::Anvil::from_file(&path) {
            Ok(anvil) => anvil,
            Err(_err) => qexed_region::region::anvil::Anvil::new(
                &path,
                self.pos[0].clone() as i32,
                self.pos[1].clone() as i32,
            )?,
        };
        for i in chunks {
            // 区块只读，我们不在乎内容
            // 有数据就传，没数据就空
            // 反正是只读，玩家咋搞都没事
            let datas = match anvil.get_chunk_data(i[0] as i32, i[1] as i32) {
                Ok(is_chunk) => match is_chunk {
                    Some(chunk) => {
                        qexed_region::region::anvil::Anvil::decompress_chunk_data(&chunk)?
                    }
                    None => vec![],
                },
                Err(_) => vec![],
            };

            let chunk = match qexed_region::chunk::nbt::Chunk::from_nbt_bytes(&datas) {
                Ok(v) => v,
                Err(_) => {
                    log::error!("读取数据损坏,读取失败:[{},{}]", self.pos[0], self.pos[1]);
                    qexed_region::chunk::nbt::Chunk::new(
                        self.pos[0] as i32,
                        self.pos[1] as i32,
                        "empty".to_string(),
                    )
                }
            };
            let (chunk_task, chunk_sender) = qexed_task::task::task::Task::new(
                api.clone(),
                ChunkTask::new(
                    self.config.clone(),
                    self.world_root.clone(),
                    self.world_uuid.clone(),
                    i.clone(),
                    chunk,
                    true,
                ),
            );

            chunk_task.run().await?;
            chunk_sender.send(UnReturnMessage::build(ChunkCommand::Init))?;
            task_map.insert(i, chunk_sender);
        }
        //暂定12，后续配置文件修改
        for i in
            self.get_chunks_in_region_view([self.config.join_pos[0], self.config.join_pos[2]], 12)
        {
            // 处理空区块
            if !task_map.contains_key(&i) {
                let (chunk_task, chunk_sender) = qexed_task::task::task::Task::new(
                    api.clone(),
                    ChunkTask::new(
                        self.config.clone(),
                        self.world_root.clone(),
                        self.world_uuid.clone(),
                        i.clone(),
                        qexed_region::chunk::nbt::Chunk::new(
                            self.pos[0] as i32,
                            self.pos[1] as i32,
                            "empty".to_string(),
                        ),
                        false,
                    ),
                );

                chunk_task.run().await?;
                chunk_sender.send(UnReturnMessage::build(ChunkCommand::Init))?;
                task_map.insert(i, chunk_sender);
            }
        }
        // log::info!("{:?}",chunks);
        Ok(())
    }
    // 计算给定的chunk坐标是否属于指定的region
    pub fn is_chunk_in_region(&self, chunk_pos: [i64; 2]) -> bool {
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
    /// 计算指定区域内玩家视野范围内的区块列表
    /// - region_pos: 区域坐标 [region_x, region_z]
    /// - player_pos: 玩家方块坐标 [x, z]
    /// - view_distance_chunks: 视野距离（以区块为单位的半径）
    /// - 返回：该区域内位于玩家视野中的区块坐标列表
    pub fn get_chunks_in_region_view(
        &self,
        player_pos: [i64; 2],
        view_distance_chunks: i64,
    ) -> Vec<[i64; 2]> {
        // 计算玩家所在的区块坐标
        let player_chunk_x = player_pos[0].div_euclid(16);
        let player_chunk_z = player_pos[1].div_euclid(16);

        // 计算玩家视野范围内的区块边界
        let view_min_x = player_chunk_x - view_distance_chunks;
        let view_max_x = player_chunk_x + view_distance_chunks;
        let view_min_z = player_chunk_z - view_distance_chunks;
        let view_max_z = player_chunk_z + view_distance_chunks;

        // 计算当前区域包含的区块范围
        let region_min_x = self.pos[0] * 32;
        let region_max_x = self.pos[0] * 32 + 31; // 区域包含32个区块，索引0-31
        let region_min_z = self.pos[1] * 32;
        let region_max_z = self.pos[1] * 32 + 31;

        // 计算视野与区域的重叠部分
        let overlap_min_x = view_min_x.max(region_min_x);
        let overlap_max_x = view_max_x.min(region_max_x);
        let overlap_min_z = view_min_z.max(region_min_z);
        let overlap_max_z = view_max_z.min(region_max_z);

        // 如果没有重叠区域，返回空列表
        if overlap_min_x > overlap_max_x || overlap_min_z > overlap_max_z {
            return Vec::new();
        }

        // 生成重叠区域内的所有区块坐标
        let mut chunks = Vec::new();
        for chunk_x in overlap_min_x..=overlap_max_x {
            for chunk_z in overlap_min_z..=overlap_max_z {
                chunks.push([chunk_x, chunk_z]);
            }
        }

        chunks
    }
}
