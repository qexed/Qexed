use async_trait::async_trait;

use crate::message::MessageSender;

// 任务
#[async_trait]
pub trait TaskEvent<MessageType,ManageMessageType> {
    async fn event(
        &mut self,
        api: &MessageSender<MessageType>,
        manage_api: &MessageSender<ManageMessageType>,
        data: MessageType,
    ) -> anyhow::Result<bool>;
    async fn finish(
        &mut self,
        api: &MessageSender<MessageType>,
        manage_api: &MessageSender<ManageMessageType>,
    ) -> anyhow::Result<()>{
        Ok(())
    }
}


// 任务（简化版
#[async_trait]
pub trait TaskEasyEvent<MessageType> {
    async fn event(
        &mut self,
        api: &MessageSender<MessageType>,
        data: MessageType,
    ) -> anyhow::Result<bool>;
    async fn finish(
        &mut self,
        api: &MessageSender<MessageType>,
    ) -> anyhow::Result<()>{
        Ok(())
    }
}
