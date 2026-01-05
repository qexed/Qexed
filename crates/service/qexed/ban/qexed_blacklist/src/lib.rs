use async_trait::async_trait;
use qexed_config::{app::qexed_blacklist::BlackList, public::storage_engine::StorageEngine};
use qexed_task::{
    event::task::TaskEasyEvent,
    message::{
        MessageSender, MessageType, return_message::ReturnMessage,
        unreturn_message::UnReturnMessage,
    },
};
use tokio::sync::{mpsc::UnboundedSender, oneshot};
#[derive(Debug, Clone)]
pub enum Message {
    Start,
    CheckPlayerBan(uuid::Uuid, Option<String>),
}

#[derive(Debug)]
pub struct Task {
    pub config: BlackList,
    qexed_database_api: UnboundedSender<UnReturnMessage<qexed_database::message::ManageCommand>>,
    pika_db: Option<bb8::Pool<bb8_redis::RedisConnectionManager>>,
}
impl Task {
    pub fn new(
        config: BlackList,
        qexed_database_api: UnboundedSender<
            UnReturnMessage<qexed_database::message::ManageCommand>,
        >,
    ) -> Self {
        Self {
            config,
            qexed_database_api,
            pika_db: None,
        }
    }
}

#[async_trait]
impl TaskEasyEvent<ReturnMessage<Message>> for Task {
    async fn event(
        &mut self,
        _api: &MessageSender<ReturnMessage<Message>>,
        mut data: ReturnMessage<Message>,
    ) -> anyhow::Result<bool> {
        match data.data {
            Message::Start => match self.config.storage_engine {
                StorageEngine::Simple => {}
                StorageEngine::Pika => {
                    let (w, r) = oneshot::channel();
                    UnReturnMessage::build(qexed_database::message::ManageCommand::GetPikaConnect(
                        self.config.pika.data.clone(),
                        w,
                    ))
                    .post(&self.qexed_database_api)
                    .await?;
                    let db = match r.await? {
                        Ok(v) => v,
                        Err(e) => return Err(e.into()),
                    };
                    self.pika_db = Some(db);
                }
                _ => {}
            },
            Message::CheckPlayerBan(uuid, ref mut bytes) => {
                match self.config.storage_engine {
                    StorageEngine::Simple => {
                        // log::debug!("玩家黑名单列表检测:{:?},目标UUID:{}",self.config.simple.player_list,uuid);
                        if self.config.simple.player_list.contains(&uuid) {
                            *bytes = Some(format!("{}", self.config.kick_message));
                        }
                    }
                    StorageEngine::Pika => {
                        if let Some(db) = &self.pika_db {
                            let redis_key = format!("{}:{}", self.config.pika.key_prefix,&uuid);

                            // 修正1: 使用 .get() 方法从连接池获取连接
                            match db.get().await {
                                Ok(mut conn) => {
                                    // 修正2: 简化 redis 命令的调用方式
                                    // bb8_redis 通常已经包含了 redis crate，可以直接使用 redis::cmd
                                    let is_banned: Result<bool, _> = bb8_redis::redis::cmd("SISMEMBER")
                                        .arg(&redis_key)
                                        .arg(&uuid.to_string())
                                        .query_async(&mut *conn)
                                        .await;

                                    match is_banned {
                                        Ok(true) => {
                                            *bytes = Some(format!("{}", self.config.kick_message));
                                        }
                                        Ok(false) => {
                                            // log::debug!("玩家 {} 不在黑名单中，允许连接", uuid);
                                        }
                                        Err(e) => {
                                            *bytes = Some(
                                                "黑名单系统暂时不可用，请稍后重试".to_string(),
                                            );
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::error!("获取 Pika 连接失败: {}", e);
                                    *bytes = Some("黑名单数据库连接失败，请稍后重试".to_string());
                                }
                            }
                        } else {
                            *bytes =
                                Some("黑名单数据库连接失败,为以防万一,暂时无法进入".to_string());
                        }
                    }
                    _ => {
                        *bytes = Some("当前引擎暂不支持此选项,请等待官方更新".to_string());
                    }
                }
            }
        };
        if let Some(send) = data.get_return_send().await? {
            let _ = send.send(data.data);
        }
        Ok(false)
    }
}
pub async fn run(
    config: BlackList,
    qexed_database_api: UnboundedSender<UnReturnMessage<qexed_database::message::ManageCommand>>,
) -> anyhow::Result<UnboundedSender<ReturnMessage<Message>>> {
    if !matches!(
        config.storage_engine,
        StorageEngine::Simple | StorageEngine::Pika
    ) {
        return Err(anyhow::anyhow!("暂未支持此引擎"));
    }
    // 假设创建任务服务端
    let task_data = Task::new(config, qexed_database_api);
    let (task, task_send) = qexed_task::task::task::TaskEasy::new(task_data);
    task.run().await?;
    ReturnMessage::build(Message::Start).get(&task_send).await?;
    log::info!("[服务] 黑名单 已启用");
    Ok(task_send)
}
