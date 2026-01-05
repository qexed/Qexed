use std::cmp::min;

use qexed_task::{event::task::TaskEvent, message::{MessageSender, return_message::ReturnMessage}};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::message::{ManagerCommand, SpiltIDCommand};

#[derive(Debug)]
pub struct SplitIDActor{
    pub max_id:i32,
    pub unuse_ids:UnboundedReceiver<i32>,
    pub recycle_id:UnboundedSender<i32>,
}
impl SplitIDActor {
    pub fn new()->Self{
        let (s,r) = unbounded_channel();
        Self { max_id: 0, unuse_ids:r , recycle_id: s }
    }
}

#[async_trait::async_trait]
impl TaskEvent<SpiltIDCommand,ReturnMessage<ManagerCommand>>
    for SplitIDActor
{
    async fn event(
        &mut self,
        api: &MessageSender<SpiltIDCommand>,
        _manage_api: &MessageSender<ReturnMessage<ManagerCommand>>,
        data: SpiltIDCommand,
    ) -> anyhow::Result<bool> {
        match data {
            SpiltIDCommand::Start => {

            },
            SpiltIDCommand::New(sender) => {
                if let Ok(id) = self.unuse_ids.try_recv(){
                    let _ = sender.send(id);
                    return Ok(false);
                }
                // 读取100个新id丢到池里
                let id_l =self.max_id;
                self.max_id+=min(100,i32::MAX-self.max_id);
                if self.max_id==i32::MAX{
                    panic!("业务崩溃,实体id耗尽了(原版MC协议限制)");
                    return Ok(false)
                }
                for i in id_l..self.max_id{
                    let _ = self.recycle_id.send(i);
                }
                // 最后重新请求
                let _ = api.send(SpiltIDCommand::New(sender));

            },
            SpiltIDCommand::Recycled(id) => {
                let _ = self.recycle_id.send(id);
                return Ok(false);
            },
        }
        Ok(false)
    }
}