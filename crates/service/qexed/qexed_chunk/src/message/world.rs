use qexed_command::message::CommandData;
use qexed_task::message::{MessageSender, unreturn_message::UnReturnMessage};
use tokio::sync::{mpsc::UnboundedSender, oneshot};
use uuid::Uuid;

use crate::message::{chunk::ChunkCommand, region::{RegionCommand, RegionCommandResult}};

#[derive(Debug)]
pub enum WorldCommand {
    // 初始化命令
    Init,
    // 玩家进服
    PlayerJoin{
        uuid:Uuid,
        pos:[i64;3],
        packet_send:UnboundedSender<bytes::Bytes>
    },
    // 区域管理
    GetRegionApi {
        pos: [i64; 2],
        result: oneshot::Sender<RegionCommandResult>,
    },
    CreateRegion {
        pos: [i64; 2],
        result: oneshot::Sender<RegionCommandResult>,
    },
    
    // 跨世界操作（转发到Global）
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
    
    // 区块操作（转发到对应区域）
    GetChunkApi {
        pos: [i64; 2],
        result: oneshot::Sender<RegionCommandResult>,
    },
    CreateChunk {
        pos: [i64; 2],
        result: oneshot::Sender<RegionCommandResult>,
    },
    SendChunkCommand {
        pos: [i64; 2],
        event: ChunkCommand,
    },
    SendChunkNeedReturnCommand {
        pos: [i64; 2],
        event: ChunkCommand,
        result: oneshot::Sender<bool>,
    },
    
    // 跨世界区块操作（转发到Global）
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
    
    // 世界级事件
    RegionCloseEvent {
        pos: [i64; 2],
    },
    WorldCloseCommand{
        result:oneshot::Sender<()>,
    },

    // 指令:seed
    CommandSeed(CommandData),// 指令事件
}