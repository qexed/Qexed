use tokio::sync::mpsc::UnboundedReceiver;

use crate::{event::task::{TaskEasyEvent, TaskEvent}, message::MessageSender};



// TEST
pub struct Task<MessageType,ManageMessageType, Task> {
    api: MessageSender<MessageType>,
    manage_api: MessageSender<ManageMessageType>,
    other: Task,
    receiver: Option<UnboundedReceiver<MessageType>>,
}
impl<MessageType,ManageMessageType, TaskData> Task<MessageType,ManageMessageType,TaskData>
where
    MessageType: Send + 'static + std::fmt::Debug + Unpin,
    ManageMessageType: Send + 'static + std::fmt::Debug + Unpin,
    TaskData: Send + 'static + std::fmt::Debug + Unpin + TaskEvent<MessageType,ManageMessageType>, // 添加 Send
    
{
    pub fn new(
        manage_api: MessageSender<ManageMessageType>,
        data: TaskData,
    ) -> (Self, MessageSender<MessageType>) {
        let (w, r) = tokio::sync::mpsc::unbounded_channel();
        (
            Self {
                api: w.clone(),
                manage_api: manage_api,
                other: data,
                receiver: Some(r),
            },
            w,
        )
    }
    // 请注意:下面的所有权转移并不是失误,是刻意的设计
    pub async fn run(self) -> anyhow::Result<()> {
        tokio::spawn(self.listen());
        Ok(())
    }
    async fn listen(mut self) -> anyhow::Result<()> {
        let mut receiver = self
            .receiver
            .take()
            .ok_or_else(|| anyhow::anyhow!("接收管道不存在"))?;
        let api = self.api;
        let manage_api = self.manage_api;
        while let Some(data) = receiver.recv().await {
            // 这里我们后面修改来实现具体业务逻辑
            if self.other.event(&api, &manage_api, data).await? {
                receiver.close();
            }
        }
        Ok(())
    }
}

// TEST
pub struct TaskEasy<MessageType, Task> {
    api: MessageSender<MessageType>,
    other: Task,
    receiver: Option<UnboundedReceiver<MessageType>>,
}
impl<MessageType, TaskData> TaskEasy<MessageType, TaskData>
where
    MessageType: Send + 'static + std::fmt::Debug + Unpin,
    TaskData: Send + 'static + std::fmt::Debug + Unpin + TaskEasyEvent<MessageType>, // 添加 Send
{
    pub fn new(
        data: TaskData,
    ) -> (Self, MessageSender<MessageType>) {
        let (w, r) = tokio::sync::mpsc::unbounded_channel();
        (
            Self {
                api: w.clone(),
                other: data,
                receiver: Some(r),
            },
            w,
        )
    }
    // 请注意:下面的所有权转移并不是失误,是刻意的设计
    pub async fn run(self) -> anyhow::Result<()> {
        tokio::spawn(self.listen());
        Ok(())
    }
    async fn listen(mut self) -> anyhow::Result<()> {
        let mut receiver = self
            .receiver
            .take()
            .ok_or_else(|| anyhow::anyhow!("接收管道不存在"))?;
        let api = self.api;
        while let Some(data) = receiver.recv().await {
            // 这里我们后面修改来实现具体业务逻辑
            if self.other.event(&api, data).await? {
                receiver.close();
            }
        }
        Ok(())
    }
}