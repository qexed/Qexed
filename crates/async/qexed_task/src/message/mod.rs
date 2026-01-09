pub mod return_message;
pub mod unreturn_message;
pub mod steam_message;
use async_trait::async_trait;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

pub type MessageSender<MessageType> = UnboundedSender<MessageType>;
pub type MessageReceiver<MessageType> = UnboundedReceiver<MessageType>;
pub fn channel<T>()->anyhow::Result<(MessageSender<T>,MessageReceiver<T>)>{
    return Ok(tokio::sync::mpsc::unbounded_channel());
}
#[async_trait]
pub trait MessageType<T, R, S>: Sized + Send + 'static
where
    T: Send + 'static + Sync,
    R: Send + 'static,
    S: Send + 'static,
{
    fn build(data: T) -> Self;
    async fn post(self, send: &MessageSender<Self>) -> anyhow::Result<R>;
    async fn get_return_send(&mut self) -> anyhow::Result<S>;

}