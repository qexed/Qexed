use std::{fs, path::{Path, PathBuf}};

use anyhow::Context;
use qexed_task::message::{MessageSender, unreturn_message::UnReturnMessage};

use crate::message::global::GlobalCommand;

#[derive(Debug)]
pub struct WorldManage {
    // 世界配置文件
    pub config: qexed_config::app::qexed_chunk::world::World,
    // 世界目录
    pub world_root: PathBuf,
    // 世界uuid
    pub world_uuid: uuid::Uuid,
    // 全局api
    pub master_api:MessageSender<UnReturnMessage<GlobalCommand>>,
}
impl WorldManage {
    pub fn new(
        config: qexed_config::app::qexed_chunk::world::World,
        worlds_root: PathBuf,
        world_uuid: uuid::Uuid,
        master_api:MessageSender<UnReturnMessage<GlobalCommand>>
    ) -> Self {
        let world_root: PathBuf = Path::new(&worlds_root).join(world_uuid.clone().to_string()).to_path_buf();
        Self {
            config,
            world_root,
            world_uuid,
            master_api
        }
    }
    pub fn init(&self) -> anyhow::Result<()> {
        log::info!("初始化世界:{}",self.config.name);
        self.ensure_worlds_root()?;
        self.create_world_subdirectories()?;

        self.validate_directory_structure()?;
        log::info!("世界{}初始化完成",self.config.name);
        Ok(())
    }
    fn ensure_worlds_root(&self) -> anyhow::Result<()> {
        if !self.world_root.exists() {
            log::info!("创建{}世界[类型:{}]目录: {}",&self.config.name,&self.config.namespace,self.world_root.display());
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
        let mut required_dirs = vec![
            "region",      // 区域文件 (.mca)
            "data",        // 世界数据 (level.dat, session.lock 等)
        ];
        if self.config.poi{
            // "poi",         // 兴趣点
            required_dirs.push("poi")
        }
        if self.config.entitie{
            // "entities",    // 实体数据
            required_dirs.push("entities")
        }
        
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
        let mut required_dirs = vec![
            "region",      // 区域文件 (.mca)
            "data",        // 世界数据 (level.dat, session.lock 等)
        ];
        if self.config.poi{
            // "poi",         // 兴趣点
            required_dirs.push("poi")
        }
        if self.config.entitie{
            // "entities",    // 实体数据
            required_dirs.push("entities")
        }
        
        
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
}
