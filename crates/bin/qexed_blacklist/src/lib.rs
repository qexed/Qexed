use anyhow::Ok;
use async_trait::async_trait;
use qexed_config::{app::{ qexed_blacklist::BlackList}, public::storage_engine::StorageEngine};
use qexed_task::{event::task::{TaskEasyEvent}, message::{MessageSender, MessageType, return_message::ReturnMessage}};
use tokio::sync::mpsc::UnboundedSender;
#[derive(Debug, Clone)]
pub enum Message {
    CheckPlayerBan(uuid::Uuid,Option<String>)
}

#[derive(Debug)]
pub struct Task{
    pub config:BlackList
}
impl Task {
    pub fn new(config:BlackList)->Self{
        Self { config }
    }
}

#[async_trait]
impl TaskEasyEvent<ReturnMessage<Message>> for Task {
    async fn event(
        &mut self,
        _api: &MessageSender<ReturnMessage<Message>>,
        mut data: ReturnMessage<Message>,
    ) -> anyhow::Result<bool> {
        match data.data{
            Message::CheckPlayerBan(uuid, ref mut bytes) => {
                
                match self.config.storage_engine {
                    StorageEngine::Simple=>{
                        // log::debug!("玩家黑名单列表检测:{:?},目标UUID:{}",self.config.simple.player_list,uuid);
                        if self.config.simple.player_list.contains(&uuid){
                            *bytes = Some(format!("{}",self.config.kick_message));
                        }

                    }
                    _ =>{
                        *bytes = Some("当前引擎暂不支持此选项,请等待官方更新".to_string());
                    }
                    
                }
            },
        };
        if let Some(send) = data.get_return_send().await? {
            let _ = send.send(data.data);
        }
        Ok(false)
    }
}
pub async fn run(config:BlackList)->anyhow::Result<UnboundedSender<ReturnMessage<Message>>>{
    if config.storage_engine!=StorageEngine::Simple{
        return Err(anyhow::anyhow!("暂未支持此引擎"))
    }
    // 假设创建任务服务端
    let task_data = Task::new(config);
    let (task,task_send) = qexed_task::task::task::TaskEasy::new(task_data);
    task.run().await?;
    log::info!("[服务] 黑名单 已启用");
    Ok(task_send)
}