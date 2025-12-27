use qexed_task::message::{MessageSender, unreturn_message::UnReturnMessage};
use tokio::sync::{mpsc::UnboundedSender, oneshot};
use uuid::Uuid;

use crate::message::chunk::ChunkCommand;
#[derive(Debug)]
pub enum RegionCommand {
    Init,
    // 玩家进服
    PlayerJoin{
        uuid:Uuid,
        pos:[i64;3],
        packet_send:UnboundedSender<bytes::Bytes>
    },
    // 获取ChunkApi(非创建)
    GetChunkApi {
        pos: [i64; 2],
        result: oneshot::Sender<crate::message::region::RegionCommandResult>,
    },
    GetOtherWorldChunkApi {
        pos: [i64; 2],
        world: Uuid,
        result: oneshot::Sender<crate::message::region::RegionCommandResult>,
    },
    GetRegionApi{
        pos: [i64; 2],
        result: oneshot::Sender<crate::message::region::RegionCommandResult>,
    },
    GetOtherWorldRegionApi {
        pos: [i64; 2],
        world: Uuid,
        result: oneshot::Sender<crate::message::region::RegionCommandResult>,
    },
    // 跨区块命令
    SendChunkCommand {
        pos: [i64; 2],
        event: ChunkCommand,
    },
    SendOtherWorldChunkCommand {
        pos: [i64; 2],
        world: Uuid,
        event: ChunkCommand,
    },
    SendChunkNeedReturnCommand {
        pos: [i64; 2],
        event: ChunkCommand,
        result:oneshot::Sender<bool>,
    },
    SendOtherWorldChunkNeedReturnCommand {
        pos: [i64; 2],
        world: Uuid,
        event: ChunkCommand,
        result:oneshot::Sender<bool>,
    },
    // 创建区块并返回Api（如已创建直接返回API)
    CreateChunk{
        pos: [i64; 2],
        result: oneshot::Sender<crate::message::region::RegionCommandResult>,
    },
    CreateOtherWorldChunk {
        pos: [i64; 2],
        world: Uuid,
        result: oneshot::Sender<crate::message::region::RegionCommandResult>,
    },
    CreateRegion{
        pos: [i64; 2],
        result: oneshot::Sender<crate::message::region::RegionCommandResult>,
    },
    CreateOtherWorldRegion {
        pos: [i64; 2],
        world: Uuid,
        result: oneshot::Sender<crate::message::region::RegionCommandResult>,
    },
    // 区块关闭事件通知
    ChunkClose{
        pos:[i64;2]
    },
    // 区域关闭事件通知
    RegionClose{
        pos:[i64;2]
    },
    // 当前区域必须关闭命令
    // 这不是请求，而是命令
    RegionCloseCommand{
        result:oneshot::Sender<()>,
    }
    

}
#[derive(Debug)]
pub enum RegionCommandResult {
    GetChunkApiResult {
        success: bool,
        api: Option<MessageSender<UnReturnMessage<ChunkCommand>>>,
    },
    CreateChunkResult {
        success: bool,
        api: Option<MessageSender<UnReturnMessage<ChunkCommand>>>,
    },
    GetRegionApiResult {
        success: bool,
        api: Option<MessageSender<UnReturnMessage<RegionCommand>>>,
    },
    CreateRegionResult {
        success: bool,
        api: Option<MessageSender<UnReturnMessage<RegionCommand>>>,
    },
}
