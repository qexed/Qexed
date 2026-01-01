use tokio::sync::{mpsc::UnboundedSender, oneshot};
use uuid::Uuid;

#[derive(Debug)]
pub enum ChunkCommand{
    Init,
    PlayerJoin{
        uuid:Uuid,
        pos:[i64;3],
        packet_send:UnboundedSender<bytes::Bytes>
    },
    // 区块强制关闭命令(要求同步区块数据)
    CloseCommand{
        result:oneshot::Sender<ChunkData>,
    },
}

#[derive(Debug,Default)]
pub struct ChunkData{
    pub data:Option<Vec<u8>>,
    pub entities:Option<Vec<u8>>,
    pub poi:Option<Vec<u8>>,
    pub region:Option<Vec<u8>>,
}