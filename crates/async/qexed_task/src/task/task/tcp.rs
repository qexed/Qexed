// TCP分支意味着他是基于网络的

use tokio::sync::mpsc::UnboundedReceiver;

use crate::{event::task::{TaskEasyEvent, TaskEvent}, message::{MessageReceiver, MessageSender}};

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

// 暂未写好,不知道咋写了
pub struct TaskEasyTcp<MessageType, Task> {
    api: MessageSender<MessageType>,
    other: Task,
    receiver: Option<MessageReceiver<MessageType>>,
    ip: String,
}
impl<MessageType, TaskData> TaskEasyTcp<MessageType, TaskData>
where
    MessageType: Send + 'static + std::fmt::Debug + Unpin+ serde::Serialize + serde::de::DeserializeOwned,
    TaskData: Send + 'static + std::fmt::Debug + Unpin + TaskEasyEvent<MessageType>, // 添加 Send
{
    pub fn new(
        data: TaskData,
        ip: String,
    ) -> (Self, MessageSender<MessageType>) {
        let (w, r) = tokio::sync::mpsc::unbounded_channel();
        (
            Self {
                api: w.clone(),
                other: data,
                receiver: Some(r),
                ip
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

