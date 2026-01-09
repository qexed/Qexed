use tokio::task::JoinHandle;

use crate::{
    event::task_manage::TaskManageEvent, message::{MessageReceiver, MessageSender}
};

pub struct TaskManage<MessageType, Task> {
    api: MessageSender<MessageType>,
    other: Task,
    receiver: Option<MessageReceiver<MessageType>>,
    cancel_token: tokio_util::sync::CancellationToken,
}
impl<MessageType, TaskData> TaskManage<MessageType, TaskData>
where
    MessageType: Send + 'static + Unpin,
    TaskData: Send + 'static + Unpin + TaskManageEvent<MessageType>, // 添加 Send
{
    pub fn new(
        data: TaskData,
    ) -> anyhow::Result<(
        Self,
        MessageSender<MessageType>,
        tokio_util::sync::CancellationToken,
    )> {
        let (w, r) = crate::message::channel()?;
        let cancel_token = tokio_util::sync::CancellationToken::new();
        Ok((
            Self {
                api: w.clone(),

                other: data,
                receiver: Some(r),
                cancel_token: cancel_token.clone(),
            },
            w,
            cancel_token,
        ))
    }
    // 请注意:下面的所有权转移并不是失误,是刻意的设计
    pub async fn run(self) -> anyhow::Result<JoinHandle<anyhow::Result<()>>> {
        Ok(tokio::spawn(self.listen()))
    }
    async fn listen(mut self) -> anyhow::Result<()> {
        // 更简洁的错误处理
        let mut receiver = self
            .receiver
            .take()
            .ok_or_else(|| anyhow::anyhow!("接收管道只能使用一次"))?;

        let api = self.api;
        let cancel_token = self.cancel_token;

        // 使用Result来控制循环退出
        let result: anyhow::Result<()> = loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    log::info!("监听任务被取消");
                    break Ok(());
                }
                msg = receiver.recv() => {
                    match msg {
                        Some(data) => {
                            // 使用match处理Result，避免?在select中提前返回
                            match self.other.event(&api, data).await {
                                Ok(should_continue) => {
                                    if should_continue {
                                        log::info!("业务逻辑要求停止监听");
                                        break Ok(());
                                    }
                                }
                                Err(e) => {
                                    log::error!("事件处理错误: {}", e);
                                    break Err(e);
                                }
                            }
                        }
                        None => {
                            log::info!("消息通道关闭，监听结束");
                            break Ok(());
                        }
                    }
                }
            }
        };
        // 使用match处理Result，避免?在select中提前返回
        match self.other.finish(&api).await {
            Ok(_) => {

            }
            Err(e) => {
                log::error!("结束事件处理错误: {}", e);
            }
        }
        // 确保取消token被触发
        cancel_token.cancel();
        result
    }
}
