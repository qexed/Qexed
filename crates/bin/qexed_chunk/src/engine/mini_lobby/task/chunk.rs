use std::f32::consts::E;

use async_trait::async_trait;
use dashmap::DashMap;
use qexed_task::{event::{task::TaskEvent}, message::{MessageSender, return_message::ReturnMessage, unreturn_message::UnReturnMessage}};

use crate::{engine::mini_lobby::event::chunk::ChunkTask, message::{ chunk::{ChunkCommand, ChunkData}, region::RegionCommand}};

#[async_trait]
impl TaskEvent<UnReturnMessage<ChunkCommand>, UnReturnMessage<RegionCommand>>
for ChunkTask{
    async fn event(
        &mut self,
        api: &MessageSender<UnReturnMessage<ChunkCommand>>,
        manage_api: &MessageSender<UnReturnMessage<RegionCommand>>,
        data: UnReturnMessage<ChunkCommand>,
    ) -> anyhow::Result<bool>{
        match data.data {
            ChunkCommand::Init => {
                // 初始化函数暂时没写

                
            },
            ChunkCommand::CloseCommand { result } => {
                // 暂时没写数据读写
                result.send(ChunkData::default());
            },
        }
        Ok(false)
    }
}