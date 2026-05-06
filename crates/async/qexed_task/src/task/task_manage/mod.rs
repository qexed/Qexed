use dashmap::DashMap;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::{event::task_manage::TaskManageEvent, message::MessageSender};

// 任务管理器
// 创建与回收均由此管理
// 提供查询对应任务的服务
// 回收由任务自己向任务管理器报告
pub struct TaskManage<ID,Task,MessageType,SubMessageType> {
    task_map:DashMap<ID,MessageSender<SubMessageType>>, // 你问我为什么不保存任务而是保存api接口？
    receiver: Option<UnboundedReceiver<MessageType>>,
    api: UnboundedSender<MessageType>,
    other: Task,// 任务管理器本质也是任务,只是他是基于任务实现的
}
impl <ID,Task,MessageType,SubMessageType> TaskManage<ID,Task,MessageType,SubMessageType> 
where
    ID: std::hash::Hash + Eq + Send + Sync + 'static,
    MessageType: Send + Sync + 'static,
    SubMessageType: Send + Sync + 'static,
    Task: Send + Sync + TaskManageEvent<ID,MessageType,SubMessageType>+'static,
    
{
    pub fn new(
        data: Task,
    ) -> (Self, MessageSender<MessageType>) {
        let (w, r) = tokio::sync::mpsc::unbounded_channel();
        (
            Self {
                api: w.clone(),
                other: data,
                receiver: Some(r),
                task_map:DashMap::<ID, MessageSender<SubMessageType>>::new(),
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
        let task_map = self.task_map;
        while let Some(data) = receiver.recv().await {
            // 这里我们后面修改来实现具体业务逻辑
            if self.other.event(&api, &task_map, data).await? {
                receiver.close();
            }
        }
        Ok(())
    }
}
