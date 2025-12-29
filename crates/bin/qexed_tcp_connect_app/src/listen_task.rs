use std::time::Duration;

use dashmap::DashMap;
use qexed_task::{event::task::TaskEasyEvent, message::{MessageSender, MessageType, return_message::ReturnMessage, unreturn_message::UnReturnMessage}};

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
    
}
#[async_trait::async_trait]
impl TaskEasyEvent<UnReturnMessage<ListenCommand>> for ListenTask {
    async fn event(
        &mut self,
        api: &MessageSender<UnReturnMessage<ListenCommand>>,
        mut data: UnReturnMessage<ListenCommand>,
    ) -> anyhow::Result<bool> {
        match data.data{
            ListenCommand::Start => {
                let listener = tokio::net::TcpListener::bind(&self.config.ip).await?;
                let api = api.clone();
                tokio::spawn(
                    async move {
                        log::info!("[服务] TCP连接入口-数据包监听子任务 激活");
                        while let Ok((stream, addr)) = listener.accept().await {
                            let _ = api.send(UnReturnMessage::build(
                                ListenCommand::NewConnection(stream, addr)
                            ));
                        };
                        let _ = api.send(UnReturnMessage::build(
                            ListenCommand::Close
                        ));
                    }
                );
            },
            ListenCommand::NewConnection(tcp_stream, socket_addr) => {
                let ip = socket_addr.ip();
                if self.config.rate_limit_window_secs==0{
                    ReturnMessage::build(ManagerCommand::NewConnection(tcp_stream, socket_addr)).get(&self.manage).await?;
                    return Ok(false);
                };
                // ratelimit:DashMap<std::net::IpAddr,u32>,
                if let Some(mut v) = self.ratelimit.get_mut(&ip){
                    *v+=1;
                    if *v>=self.config.rate_limit_max_attempts{
                        drop(tcp_stream);
                        return Ok(false);
                    };
                    ReturnMessage::build(ManagerCommand::NewConnection(tcp_stream, socket_addr)).get(&self.manage).await?;
                    return Ok(false);
                };

                self.ratelimit.insert(ip, 0);
                ReturnMessage::build(ManagerCommand::NewConnection(tcp_stream, socket_addr)).get(&self.manage).await?;
                let api = api.clone();
                let rate_limit_window_secs = self.config.rate_limit_window_secs.clone();
                tokio::spawn(
                    async move {
                        tokio::time::sleep(Duration::from_secs(rate_limit_window_secs)).await;
                        let _ = api.send(UnReturnMessage::build(
                            ListenCommand::ClearRatelimit(ip)
                        ));
                    }
                );
            }
            ListenCommand::ClearRatelimit(ip)=>{
                self.ratelimit.remove(&ip);

            }
            ListenCommand::Close => {
                // 暂时不处理
            }
        }

        Ok(false) // 不停止任务
    }
}
