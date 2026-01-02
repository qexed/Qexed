use dashmap::DashMap;
use qexed_task::{event::task::TaskEasyEvent, message::{MessageSender, return_message::ReturnMessage}};

use crate::message::ManageCommand;
#[derive(Debug)]
pub struct ManageActor{
    pika_map:DashMap<qexed_config::public::pika::PikaConfig,bb8::Pool<bb8_redis::RedisConnectionManager>>
}
impl ManageActor {
    pub fn new()->Self{
        Self { pika_map: Default::default() }
    }
}
#[async_trait::async_trait]
impl TaskEasyEvent<ReturnMessage<ManageCommand>> for ManageActor {
    async fn event(
        &mut self,
        api: &MessageSender<ReturnMessage<ManageCommand>>,
        mut data: ReturnMessage<ManageCommand>,
    ) -> anyhow::Result<bool> {
        match data.data {

            ManageCommand::Command(ref cmd) => {
                cmd.send_chat_message("暂未完成").await?;
                return Ok(false);
            },
            ManageCommand::GetPikaConnect(pika_config, sender) => {

            },
        };
        return Ok(false);
    }
}