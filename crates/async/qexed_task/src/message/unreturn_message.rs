use async_trait::async_trait;

use crate::message::{MessageSender, MessageType};
#[derive(Debug)]
pub struct UnReturnMessage<T> {
    pub data: T,
}

#[async_trait]
impl<T> MessageType<T, (), ()> for UnReturnMessage<T>
where
    T: Send + 'static + Sync + std::fmt::Debug + Unpin
{
    fn build(data: T) -> Self {
        Self { data }
    }

    async fn post(self, send: &MessageSender<Self>) -> anyhow::Result<()> {
        // 发送消息
        send.send(self)
            .map_err(|e| anyhow::anyhow!("Failed to send message: {:?}", e))?;
        Ok(())
    }

    // 无返回时用不到
    async fn get_return_send(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

}
