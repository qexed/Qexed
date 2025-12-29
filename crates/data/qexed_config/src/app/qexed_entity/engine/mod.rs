use qexed_config_macros::AutoEnum;
use serde::{Deserialize, Serialize};
pub mod player;
pub mod npc;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, AutoEnum)]
pub enum Engine{
    // Player 玩家引擎
    // 此引擎必须启用，没有任何理由拒绝
    // Qexed的玩家实体
    Player,
    // NPC引擎
    // 此引擎创建的实体将根据设定的机制操作，默认没有AI或由配置文件驱动NPC
    NPC,
    
}