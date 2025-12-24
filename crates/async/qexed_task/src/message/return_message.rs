use async_trait::async_trait;
use tokio::sync::oneshot;
use crate::message::{MessageSender, MessageType};
#[derive(Debug)]
pub struct ReturnMessage<T> {
    pub data: T,
    sand: Option<oneshot::Sender<T>>,
}

#[async_trait]
impl<T> MessageType<T, oneshot::Receiver<T>, Option<oneshot::Sender<T>>> for ReturnMessage<T>
where
    T: Send + 'static + Sync + std::fmt::Debug + Unpin,
{
    fn build(data: T) -> Self {
        Self { data, sand: None }
    }

    async fn post(
        mut self,
        send: &MessageSender<Self>,
    ) -> anyhow::Result<oneshot::Receiver<T>> {
        let (s, r) = oneshot::channel();
        self.sand = Some(s);

        // 发送消息
        send.send(self)
            .map_err(|e| anyhow::anyhow!("Failed to send message: {:?}", e))?;

        Ok(r)
    }

    async fn get_return_send(&mut self) -> anyhow::Result<Option<oneshot::Sender<T>>> {
        // 取出发送器，避免多次调用
        Ok(self.sand.take())
    }


}

impl<T> ReturnMessage<T>
where
    T: Send + 'static + Sync + std::fmt::Debug + Unpin,
{
    pub async fn get(
        self,
        send: &MessageSender<Self>,
    ) -> anyhow::Result<T> {
        let return_data = self.post(&send).await?;
        return Ok(ReturnMessage::get_return_data(return_data).await?)
    }
    pub async fn get_return_data(result: oneshot::Receiver<T>) -> anyhow::Result<T> {
        match result.await {
            Ok(data) => Ok(data),
            Err(e) => {
                // 如果通道被关闭或发送端被丢弃
                Err(anyhow::anyhow!("Failed to receive return data: {:?}", e))
            }
        }
    }
    // 手动构建,适用于不走管道的情况
    pub fn manual(&mut self)->oneshot::Receiver<T>{
        let (s, r) = oneshot::channel();
        self.sand = Some(s);
        r
    }
}