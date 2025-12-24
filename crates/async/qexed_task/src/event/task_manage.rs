use async_trait::async_trait;
use dashmap::DashMap;

use crate::message::MessageSender;

#[async_trait]
pub trait TaskManageEvent<ID,MessageType,SubMessageType> {
    async fn event(
        &mut self,
        api: &MessageSender<MessageType>,
        task_map:&DashMap<ID,MessageSender<SubMessageType>>,
        data: MessageType,
    ) -> anyhow::Result<bool>;
}