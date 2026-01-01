use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Context;
use dashmap::DashMap;
use qexed_task::message::{MessageSender, unreturn_message::UnReturnMessage};
use uuid::Uuid;

use crate::{ message::{global::GlobalCommand, world::WorldCommand}};
#[derive(Debug)]
pub struct GlobalManage {
    pub config: qexed_config::app::qexed_chunk::ChunkConfig,
    pub worlds_root: PathBuf, // 世界根目录路径
}
impl GlobalManage {
    pub fn new(config: qexed_config::app::qexed_chunk::ChunkConfig) -> Self {
        let worlds_root = Path::new(&config.engine_setting.original.world_dir).to_path_buf();
        Self {
            config,
            worlds_root,
        }
    }
    pub fn init(&self, api: &MessageSender<UnReturnMessage<GlobalCommand>>) -> anyhow::Result<()> {
        log::info!("初始化区块服务");
        // 1. 确保世界根目录存在
        self.ensure_worlds_root()?;
        // // 2. 世界目录判定
        // for (uuid,world) in &self.config.world{
        //     let world_manage= WorldManage::new(world.clone(), self.worlds_root.clone(), uuid.clone());
        //     world_manage.init()?;
        // }
        log::info!("区块服务初始化完成");
        Ok(())
    }
    fn ensure_worlds_root(&self) -> anyhow::Result<()> {
        if !self.worlds_root.exists() {
            log::info!("创建世界根目录: {}", self.worlds_root.display());
            fs::create_dir_all(&self.worlds_root)
                .with_context(|| format!("无法创建世界根目录: {}", self.worlds_root.display()))?;
        }

        // 检查目录是否可写
        self.check_directory_writable(&self.worlds_root)?;

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
    pub fn load_world(
        &self,
        world_uuid: Uuid,
        api: &MessageSender<UnReturnMessage<GlobalCommand>>,
        task_map:&DashMap<uuid::Uuid,MessageSender<UnReturnMessage<WorldCommand>>>,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}
