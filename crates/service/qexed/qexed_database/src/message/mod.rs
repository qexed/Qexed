use tokio::sync::oneshot;
#[derive(Debug)]
pub enum ManageCommand {
    Command(qexed_command::message::CommandData),
    GetPikaConnect(
        qexed_config::public::pika::PikaConfig,
        oneshot::Sender<anyhow::Result<bb8::Pool<bb8_redis::RedisConnectionManager>>>,
    ),
}

// pub async fn redis_pool() {
//     // 1. 创建连接管理器
//     let manager = bb8_redis::RedisConnectionManager::new("redis://127.0.0.1/")
//         .expect("Failed to create Redis connection manager");

//     // 2. 使用构建器创建连接池，可在此配置池参数（如最大连接数）
//     let pool = bb8::Pool::builder()
//         .build(manager)
//         .await
//         .expect("Failed to create Redis pool");
//     let mut conn = pool.clone();
//     // 3. 将池设置到全局静态变量中
//     // let _ = REDIS_POOL.set(pool);
// }
