use qexed_task::message::{MessageSender, unreturn_message::UnReturnMessage};
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::message::{chunk::ChunkCommand, region::RegionCommandResult, world::WorldCommand};

#[derive(Debug)]
pub enum GlobalCommand {
    // 系统级命令
    Init,
    LoadWorld { 
        uuid: Uuid,
        result: oneshot::Sender<bool>, // 增加结果返回
    },
    UnloadWorld { 
        uuid: Uuid,
        result: oneshot::Sender<bool>, // 增加结果返回
    },
    WorldCloseEvent { 
        uuid: Uuid,
    },
    
    // 玩家管理
    PlayerJoin { 
        player_uuid: Uuid, 
        world: Uuid, 
        pos: [i64; 2], 
        view_distance: u32 
    },
    PlayerLeave { 
        player_uuid: Uuid, 
        world: Uuid, 
        pos: [i64; 2], 
        view_distance: u32 
    },
    PlayerChangeWorld { 
        player_uuid: Uuid, 
        from_world: Uuid, 
        to_world: Uuid, 
        pos: [i64; 2] 
    },
    
    // 跨世界操作
    GetOtherWorldRegionApi {
        pos: [i64; 2],
        world: Uuid,
        result: oneshot::Sender<RegionCommandResult>,
    },
    CreateOtherWorldRegion {
        pos: [i64; 2],
        world: Uuid,
        result: oneshot::Sender<RegionCommandResult>,
    },
    GetOtherWorldChunkApi {
        pos: [i64; 2],
        world: Uuid,
        result: oneshot::Sender<RegionCommandResult>,
    },
    SendOtherWorldChunkCommand {
        pos: [i64; 2],
        world: Uuid,
        event: ChunkCommand,
    },
    SendOtherWorldChunkNeedReturnCommand {
        pos: [i64; 2],
        world: Uuid,
        event: ChunkCommand,
        result: oneshot::Sender<bool>,
    },
    CreateOtherWorldChunk {
        pos: [i64; 2],
        world: Uuid,
        result: oneshot::Sender<RegionCommandResult>,
    },
    
    // 世界查询
    GetWorldApi {
        world: Uuid,
        result: oneshot::Sender<Option<MessageSender<UnReturnMessage<WorldCommand>>>>,
    },
    
    // 系统状态
    GetWorldsStatus {
        result: oneshot::Sender<Vec<(Uuid, String)>>, // 世界UUID和状态
    },
}