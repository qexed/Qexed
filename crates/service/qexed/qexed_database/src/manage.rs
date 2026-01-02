use dashmap::DashMap;
use qexed_task::{
    event::task::TaskEasyEvent,
    message::{MessageSender, return_message::ReturnMessage, unreturn_message::UnReturnMessage},
};

use crate::message::ManageCommand;
#[derive(Debug)]
pub struct ManageActor {
    pika_map: DashMap<
        qexed_config::public::pika::PikaConfig,
        bb8::Pool<bb8_redis::RedisConnectionManager>,
    >,
}
impl ManageActor {
    pub fn new() -> Self {
        Self {
            pika_map: Default::default(),
        }
    }
}
#[async_trait::async_trait]
impl TaskEasyEvent<UnReturnMessage<ManageCommand>> for ManageActor {
    async fn event(
        &mut self,
        api: &MessageSender<UnReturnMessage<ManageCommand>>,
        mut data: UnReturnMessage<ManageCommand>,
    ) -> anyhow::Result<bool> {
        match data.data {
            ManageCommand::Command(ref cmd) => {
                cmd.send_chat_message("暂未完成").await?;
                return Ok(false);
            }
            ManageCommand::GetPikaConnect(pika_config, sender) => {
                // 1. 检查是否已有该配置的连接池
                if let Some(db) = self.pika_map.get(&pika_config) {
                    let _ = sender.send(Ok(db.clone()));
                    return Ok(false);
                }

                // 2. 验证配置
                if let Err(e) = pika_config.validate() {
                    let _ = sender.send(Err(anyhow::anyhow!("Pika配置验证失败: {}", e)));
                    return Ok(false);
                }

                // 3. 根据配置生成连接字符串
                let connection_string = pika_config.connection_params();

                // 4. 创建连接管理器
                let manager = bb8_redis::RedisConnectionManager::new(connection_string)
                    .map_err(|e| anyhow::anyhow!("创建Redis连接管理器失败: {}", e))?;

                // 5. 使用配置中的参数构建连接池
                let pool_builder = bb8::Pool::builder()
                    .max_size(pika_config.pool_max_size)
                    .min_idle(Some(pika_config.pool_min_idle));

                // 6. 构建连接池
                let pool = pool_builder
                    .build(manager)
                    .await
                    .map_err(|e| anyhow::anyhow!("创建Redis连接池失败: {}", e))?;

                // 7. 在独立作用域中测试连接
                {
                    let mut test_conn = pool.get().await
                        .map_err(|e| anyhow::anyhow!("从连接池获取连接失败: {}", e))?;

                    let _: () = bb8_redis::redis::cmd("PING")
                        .query_async(&mut *test_conn)
                        .await
                        .map_err(|e| anyhow::anyhow!("Ping测试失败: {}", e))?;
                } // test_conn 在这里被丢弃，借用结束
                
                // 8. 将新连接池缓存到映射中
                self.pika_map.insert(pika_config.clone(), pool.clone());

                // 9. 通过sender返回连接池
                let _ = sender.send(Ok(pool));
                return Ok(false);
            }
        };
        return Ok(false);
    }
}
