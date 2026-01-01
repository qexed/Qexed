use std::sync::mpsc::channel;

use async_trait::async_trait;
use tokio::sync::{mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel}, oneshot};
use crate::message::{MessageSender, MessageType};
#[derive(Debug)]
pub struct SteamMessage<T> {
    pub data: T,
    sendu: UnboundedSender<T>,
    recvu: UnboundedReceiver<T>,
}

#[async_trait]
impl<T> MessageType<T, UnboundedReceiver<T>, Option<UnboundedSender<T>>> for SteamMessage<T>
where
    T: Send + 'static + Sync + std::fmt::Debug + Unpin,
{
    fn build(data: T) -> Self {
        let (s, r) = unbounded_channel();
        Self { data, sendu: s,recvu:r}
    }

    async fn post(
        mut self,
        _send: &MessageSender<Self>,
    ) -> anyhow::Result<UnboundedReceiver<T>> {
        Err(anyhow::anyhow!("SteamMessag 消息类型不支持此接口"))
    }

    async fn get_return_send(&mut self) -> anyhow::Result<Option<UnboundedSender<T>>> {
        // 取出发送器，避免多次调用
        Err(anyhow::anyhow!("SteamMessag 消息类型不支持此接口"))
    }


}

impl<T> SteamMessage<T>
where
    T: Send + 'static + Sync + std::fmt::Debug + Unpin,
{
    pub async fn send(&self,data:T)->anyhow::Result<()>{
        self.sendu.send(data)?;
        Ok(())
    }
    pub async fn recv(&mut self)->Option<T>{
        self.recvu.recv().await
    }
}