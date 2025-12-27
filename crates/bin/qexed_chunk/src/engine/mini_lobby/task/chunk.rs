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
            ChunkCommand::Init{ data } => {
                // 初始化函数暂时没写
                
                let chunk_data = match data{
                    Some(datas)=>qexed_region::chunk::nbt::Chunk::from_nbt_bytes(&datas),
                    None=>Ok(qexed_region::chunk::nbt::Chunk::new(self.pos[0] as i32, self.pos[1] as i32, "empty".to_string()))
                };
                log::info!("pos:{:?},data:{:?}",self.pos,chunk_data);
                
            },
            ChunkCommand::CloseCommand { result } => {
                // 暂时没写数据读写
                result.send(ChunkData::default());
            },
        }
        Ok(false)
    }
}