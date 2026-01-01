use std::time::Duration;

use dashmap::DashMap;
use qexed_task::{event::task::TaskEasyEvent, message::{MessageSender, MessageType, return_message::ReturnMessage, unreturn_message::UnReturnMessage}};
use rand::Rng;
use tokio::io::AsyncWriteExt;

use crate::messages::{ListenCommand, ManagerCommand};

#[derive(Debug)]
pub struct ListenTask {
    config:qexed_config::app::qexed_tcp_connect_app::TcpConnect,
    manage:MessageSender<ReturnMessage<ManagerCommand>>,
    ratelimit:DashMap<std::net::IpAddr,u32>,
}
impl ListenTask {
    pub fn new(
        config: qexed_config::app::qexed_tcp_connect_app::TcpConnect,
        manage:MessageSender<ReturnMessage<ManagerCommand>>,
        
    )->Self{
        Self{
            config,
            manage,
            ratelimit:Default::default(),
        }
    }
    pub fn clear_task(
        &self,
        api :MessageSender<ListenCommand>,
        rate_limit_window_secs: u64,
        ip: std::net::IpAddr,
    ){
        tokio::spawn(
            async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(rate_limit_window_secs)).await;
                    let _ = api.send(ListenCommand::ClearRatelimit(ip));
                }

            }
        );
    }
    
}
#[async_trait::async_trait]
impl TaskEasyEvent<ListenCommand> for ListenTask {
    async fn event(
        &mut self,
        api: &MessageSender<ListenCommand>,
        mut data: ListenCommand,
    ) -> anyhow::Result<bool> {
        match data {
            ListenCommand::Start => {
                let listener = tokio::net::TcpListener::bind(&self.config.ip).await?;
                let api = api.clone();
                tokio::spawn(
                    async move {
                        log::info!("[服务] TCP连接入口-数据包监听子任务 激活");
                        while let Ok((stream, addr)) = listener.accept().await {
                            let _ = api.send(ListenCommand::NewConnection(stream, addr));
                        };
                        let _ = api.send( ListenCommand::Close);
                    }
                );
            },
            ListenCommand::NewConnection(mut tcp_stream, socket_addr) => {
                let ip = socket_addr.ip();

                if self.config.rate_limit_window_secs==0{
                    ReturnMessage::build(ManagerCommand::NewConnection(tcp_stream, socket_addr)).get(&self.manage).await?;
                    return Ok(false);
                };
                // ratelimit:DashMap<std::net::IpAddr,u32>,
                if let Some(mut v) = self.ratelimit.get_mut(&ip){
                    *v+=1;
                    if *v>=self.config.rate_limit_max_attempts{
                        tcp_stream.shutdown().await?;
                        drop(tcp_stream);
                        if *v==self.config.rate_limit_max_attempts{
                            log::warn!("[{:?}] 连接过于频繁，疑似Dos攻击",socket_addr.ip());
                        } else{
                            let mut rng = rand::thread_rng();
                            if rng.gen_bool(0.01) {
                                log::warn!("[{:?}] 连接过于频繁，疑似Dos攻击 (已触发 {} 次，此为采样日志)", socket_addr.ip(), v.value());
                            }
                        }
                        return Ok(false);
                    };
                    
                    ReturnMessage::build(ManagerCommand::NewConnection(tcp_stream, socket_addr)).get(&self.manage).await?;
                    return Ok(false);
                };

                self.ratelimit.insert(ip, 0);
                ReturnMessage::build(ManagerCommand::NewConnection(tcp_stream, socket_addr)).get(&self.manage).await?;
                let api = api.clone();
                let rate_limit_window_secs = self.config.rate_limit_window_secs.clone();
                self.clear_task(api,rate_limit_window_secs,ip);
            }
            ListenCommand::ClearRatelimit(ip)=>{
                if let Some(mut v) = self.ratelimit.get_mut(&ip){
                    if *v>=self.config.rate_limit_max_attempts*5{
                        *v-=self.config.rate_limit_max_attempts;
                        return Ok(false);
                    }
                }
                self.ratelimit.remove(&ip);
            }
            ListenCommand::Close => {
                // 暂时不处理
            }
        }

        Ok(false) // 不停止任务
    }
}
