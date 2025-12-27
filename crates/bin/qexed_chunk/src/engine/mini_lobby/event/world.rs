use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::Context;
use dashmap::DashMap;
use qexed_task::message::{MessageSender, MessageType, unreturn_message::UnReturnMessage};

use crate::{
    engine::mini_lobby::event::region::RegionManage,
    message::{region::RegionCommand, world::WorldCommand},
};

#[derive(Debug)]
pub struct WorldManage {
    // 世界配置文件
    pub config: qexed_config::app::qexed_chunk::engine::mini_lobby::MiniLobbyConfig,
    // 世界目录
    pub world_root: PathBuf,
    // 世界uuid
    pub world_uuid: uuid::Uuid,
}
impl WorldManage {
    pub fn new(
        config: qexed_config::app::qexed_chunk::engine::mini_lobby::MiniLobbyConfig,
    ) -> Self {
        let world_uuid = config.main_world.clone();
        let world_root: PathBuf = Path::new(&config.world_dir)
            .join(world_uuid.clone().to_string())
            .to_path_buf();
        Self {
            config,
            world_root,
            world_uuid,
        }
    }
    pub async fn init(
        &self,
        api: &MessageSender<UnReturnMessage<WorldCommand>>,
        task_map: &DashMap<[i64; 2], MessageSender<UnReturnMessage<RegionCommand>>>,
    ) -> anyhow::Result<()> {
        log::info!("初始化世界");
        self.ensure_worlds_root()?;
        self.create_world_subdirectories()?;

        self.validate_directory_structure()?;

        //  区域范围加载
        let regions: Vec<[i64; 2]> = self.get_all_regions_between();
        let view_regions: Vec<[i64; 2]> = self.calculate_view_regions(12); // 暂定12，后续配置文件中读取
        // 使用 HashSet 自动去重
        let set: HashSet<[i64; 2]> = regions
            .into_iter()
            .chain(view_regions.into_iter())
            .collect();

        let regions = set;
        // 由于服务器视野，我们可能会允许视野事件来动态创建区域(不过数据全空就是了)
        for pos in regions {
            let (manager_task, manager_sender) =
                qexed_task::task::task_manage::TaskManage::new(RegionManage::new(
                    self.config.clone(),
                    self.world_root.clone(),
                    self.world_uuid.clone(),
                    pos.clone(),
                    api.clone(),
                ));

            manager_task.run().await?;
            manager_sender.send(UnReturnMessage::build(RegionCommand::Init))?;
            task_map.insert(pos, manager_sender);
        }
        //
        log::info!("世界初始化完成");
        Ok(())
    }
    fn ensure_worlds_root(&self) -> anyhow::Result<()> {
        if !self.world_root.exists() {
            log::info!(
                "创建{}世界目录: {}",
                &self.config.name,
                self.world_root.display()
            );
            fs::create_dir_all(&self.world_root)
                .with_context(|| format!("无法创建世界目录: {}", self.world_root.display()))?;
        }

        // 检查目录是否可写
        self.check_directory_writable(&self.world_root)?;

        Ok(())
    }
    /// 检查目录是否可写
    fn check_directory_writable(&self, path: &Path) -> anyhow::Result<()> {
        let test_file = path.join(".write_test");
        match fs::write(&test_file, b"test") {
            Ok(_) => {
                fs::remove_file(&test_file)?;
                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!("目录 {} 不可写: {}", path.display(), e)),
        }
    }
    /// 创建世界所需的子目录
    fn create_world_subdirectories(&self) -> anyhow::Result<()> {
        log::info!("为世界 {} 创建子目录结构", self.config.name);

        // Minecraft 必需的核心目录
        let required_dirs = vec![
            "region", // 区域文件 (.mca)
            "data",   // 世界数据 (level.dat, session.lock 等)
        ];

        // 创建必需目录
        for dir in &required_dirs {
            let dir_path = self.world_root.join(dir);
            if !dir_path.exists() {
                fs::create_dir(&dir_path)
                    .with_context(|| format!("无法创建必需目录 {}: {}", dir, dir_path.display()))?;
                log::debug!("创建必需目录: {} -> {}", dir, dir_path.display());
            } else {
                // 验证必需目录是否可写
                self.check_directory_writable(&dir_path)
                    .with_context(|| format!("必需目录 {} 不可写", dir))?;
            }
        }

        log::info!("世界 {} 子目录创建完成", self.config.name);
        Ok(())
    }
    /// 验证目录结构
    fn validate_directory_structure(&self) -> anyhow::Result<()> {
        log::info!("验证世界 {} 的目录结构", self.config.name);

        // 必需目录列表
        // Minecraft 必需的核心目录
        let required_dirs = vec![
            "region", // 区域文件 (.mca)
            "data",   // 世界数据 (level.dat, session.lock 等)
        ];

        for dir_name in required_dirs {
            let dir_path = self.world_root.join(dir_name);

            if !dir_path.exists() {
                return Err(anyhow::anyhow!(
                    "必需目录不存在: {} (世界: {})",
                    dir_path.display(),
                    self.config.name
                ));
            } else if !dir_path.is_dir() {
                return Err(anyhow::anyhow!(
                    "路径不是目录: {} (世界: {})",
                    dir_path.display(),
                    self.config.name
                ));
            }
        }

        // 检查目录权限
        self.check_world_permissions()?;

        log::info!("世界 {} 目录结构验证通过", self.config.name);
        Ok(())
    }
    /// 检查世界目录权限
    fn check_world_permissions(&self) -> anyhow::Result<()> {
        // 检查根目录权限
        self.check_directory_writable(&self.world_root)
            .with_context(|| format!("世界根目录不可写: {}", self.world_root.display()))?;

        // 检查必需子目录权限
        let required_subdirs = ["region", "entities", "poi", "data"];

        for subdir in &required_subdirs {
            let dir_path = self.world_root.join(subdir);
            if dir_path.exists() {
                self.check_directory_writable(&dir_path)
                    .with_context(|| format!("子目录 {} 不可写", subdir))?;
            }
        }

        Ok(())
    }
    pub fn calc_region_pos(&self, chunk_pos: [i64; 2]) -> [i64; 2] {
        // 假设每个区域包含 32x32 个区块
        const REGION_SIZE: i64 = 32;

        [
            chunk_pos[0] / REGION_SIZE - if chunk_pos[0] < 0 { 1 } else { 0 },
            chunk_pos[1] / REGION_SIZE - if chunk_pos[1] < 0 { 1 } else { 0 },
        ]
    }
    /// 计算区块所在的区域位置 - 重命名以避免与event中的方法冲突
    pub fn calc_region_pos_for_world(&self, chunk_pos: [i64; 2]) -> [i64; 2] {
        // 假设每个区域包含 32x32 个区块
        const REGION_SIZE: i64 = 32;

        [
            chunk_pos[0] / REGION_SIZE - if chunk_pos[0] < 0 { 1 } else { 0 },
            chunk_pos[1] / REGION_SIZE - if chunk_pos[1] < 0 { 1 } else { 0 },
        ]
    }
    pub fn get_all_chunks_in_range(&self) -> Vec<[i64; 2]> {
        // 获取地图范围
        let map_range = self.config.map_range;

        // 假设 map_range 的结构是 [[x_min, z_min], [x_max, z_max]]
        let x_min = map_range[0][0];
        let z_min = map_range[0][1];
        let x_max = map_range[1][0];
        let z_max = map_range[1][1];

        // 计算区块范围
        // 注意：这里假设坐标是方块坐标，需要转换为区块坐标
        // 区块坐标 = 方块坐标 / 16（向下取整）
        let chunk_x_min = x_min.div_euclid(16);
        let chunk_z_min = z_min.div_euclid(16);
        let chunk_x_max = x_max.div_euclid(16);
        let chunk_z_max = z_max.div_euclid(16);

        // 收集所有区块坐标
        let mut chunks = Vec::new();

        for x in chunk_x_min..=chunk_x_max {
            for z in chunk_z_min..=chunk_z_max {
                chunks.push([x, z]);
            }
        }

        chunks
    }
    /// 计算两个坐标之间的所有区域
    /// 输入：方块坐标范围 [[x_min, z_min], [x_max, z_max]]
    /// 输出：区域坐标列表
    pub fn get_all_regions_between(&self) -> Vec<[i64; 2]> {
        // 获取地图范围
        let map_range = self.config.map_range;

        // 计算区块范围
        let chunk_x_min = map_range[0][0].div_euclid(16);
        let chunk_z_min = map_range[0][1].div_euclid(16);
        let chunk_x_max = map_range[1][0].div_euclid(16);
        let chunk_z_max = map_range[1][1].div_euclid(16);

        // 计算区域范围
        let region_x_min = chunk_x_min.div_euclid(32);
        let region_z_min = chunk_z_min.div_euclid(32);
        let region_x_max = chunk_x_max.div_euclid(32);
        let region_z_max = chunk_z_max.div_euclid(32);

        // 收集所有区域坐标
        let mut regions = Vec::new();

        for rx in region_x_min..=region_x_max {
            for rz in region_z_min..=region_z_max {
                regions.push([rx, rz]);
            }
        }

        regions
    }
    /// 计算玩家视野所涉及的区域范围
    /// - player_pos: 玩家方块坐标 [x, z]
    /// - view_distance_chunks: 视野距离，以区块为单位的半径
    /// - 返回：区域坐标列表，每个区域坐标为 [region_x, region_z]
    pub fn calculate_view_regions(&self, view_distance_chunks: i64) -> Vec<[i64; 2]> {
        // 计算玩家所在的区块坐标（使用欧几里得除法处理负数）
        let player_chunk_x = self.config.join_pos[0].div_euclid(16);
        let player_chunk_z = self.config.join_pos[2].div_euclid(16);

        // 计算视野范围内的区块边界[1](@ref)
        let min_chunk_x = player_chunk_x - view_distance_chunks;
        let max_chunk_x = player_chunk_x + view_distance_chunks;
        let min_chunk_z = player_chunk_z - view_distance_chunks;
        let max_chunk_z = player_chunk_z + view_distance_chunks;

        // 计算对应的区域坐标范围[6](@ref)
        let min_region_x = min_chunk_x.div_euclid(32);
        let max_region_x = max_chunk_x.div_euclid(32);
        let min_region_z = min_chunk_z.div_euclid(32);
        let max_region_z = max_chunk_z.div_euclid(32);

        // 生成所有区域坐标
        let mut regions = Vec::new();
        for rx in min_region_x..=max_region_x {
            for rz in min_region_z..=max_region_z {
                regions.push([rx, rz]);
            }
        }

        regions
    }
    pub fn player_pos_to_chunk_pos(&self, player_pos: [i32; 3]) -> [i32; 2] {
        let chunk_x = player_pos[0] >> 4; // 等同于 player_pos[0] / 16
        let chunk_z = player_pos[2] >> 4; // 等同于 player_pos[2] / 16
        [chunk_x, chunk_z]
    }
}
