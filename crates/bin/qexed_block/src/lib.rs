pub mod registry;
// crates/bin/qexed_block/src/lib.rs

// 包含生成的代码
include!(concat!(env!("OUT_DIR"), "/block_registry_generated.rs"));

// 方块注册表管理器
#[derive(Debug, Clone)]
pub struct BlockRegistry {
    // 可以添加状态管理等
}

impl BlockRegistry {
    pub fn new() -> Self {
        Self {}
    }
    
    /// 验证方块ID是否有效
    pub fn is_valid_block_id(&self, id: u32) -> bool {
        get_block_name_by_id(id).is_some()
    }
    
    /// 获取方块信息
    pub fn get_block_info(&self, id: u32) -> Option<BlockInfo> {
        BLOCK_INFO_BY_ID.get(&id).cloned()
    }
    
    /// 解析方块名称
    pub fn parse_block_name(&self, name: &str) -> Option<u32> {
        get_block_id_by_name(name)
    }
}

// 方块位置结构
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

// 方块状态
#[derive(Debug, Clone)]
pub struct BlockState {
    pub id: u32,
    pub position: BlockPos,
    // 可以添加其他属性
}