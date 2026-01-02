use std::net::IpAddr;

use dashmap::DashMap;
use qexed_task::{event::task::TaskEvent, message::{MessageSender, return_message::ReturnMessage}};
use tokio::sync::mpsc::UnboundedSender;

use crate::message::{ManagerMessage, TcpConnectStartMessage};

#[derive(Debug)]
pub struct TcpConnectStartSubActor {
    config: qexed_config::app::qexed_guard::qexed_tcp_connect_app_start::TcpConnectStart,
    qexed_warden_api: UnboundedSender<ReturnMessage<qexed_warden::message::ManagerMessage>>,
    qexed_database_api: UnboundedSender<ReturnMessage<qexed_database::message::ManageCommand>>,

    ban_ip:DashMap<IpAddr,i64>,
}
impl TcpConnectStartSubActor {
    pub fn new(
        config: qexed_config::app::qexed_guard::qexed_tcp_connect_app_start::TcpConnectStart,
        qexed_warden_api: UnboundedSender<ReturnMessage<qexed_warden::message::ManagerMessage>>,
        qexed_database_api:UnboundedSender<ReturnMessage<qexed_database::message::ManageCommand>>
    ) -> Self {
        Self {
            config,
            qexed_warden_api,
            qexed_database_api,
            ban_ip:Default::default(),
        }
    }
    pub async fn check_ip_connect_is_finish(&self,ip:IpAddr,api: MessageSender<TcpConnectStartMessage>){
        tokio::spawn(async move {
            api.send(TcpConnectStartMessage::CheckIpConnectSppedIsFinish(ip))
        });
    }
}

#[async_trait::async_trait]
impl TaskEvent<TcpConnectStartMessage,ReturnMessage<ManagerMessage>> for TcpConnectStartSubActor {
    async fn event(
        &mut self,
        api: &MessageSender<TcpConnectStartMessage>,
        manage_api: &MessageSender<ReturnMessage<ManagerMessage>>,
        mut data: TcpConnectStartMessage,
    ) -> anyhow::Result<bool> {
        match data {
            TcpConnectStartMessage::BanIpConnectSpeedToHigh(ip) => {
                let is_have =  self.ban_ip.contains_key(&ip);
                self.ban_ip.insert(ip, chrono::prelude::Local::now().timestamp());
                if !is_have{
                    self.check_ip_connect_is_finish(ip,api.clone()).await;
                };
                
                return Ok(false);
            }
            TcpConnectStartMessage::CheckIpConnectSppedIsFinish(ip)=>{
                if let Some(t) = self.ban_ip.get(&ip){
                    if t.value() + self.config.check_connect_too_spped_time > chrono::prelude::Local::now().timestamp(){ // 配置文件里面的单位为秒
                        self.ban_ip.remove(&ip);
                    } else {
                        self.check_ip_connect_is_finish(ip,api.clone()).await;
                    }
                }
                return Ok(false);
            }
            TcpConnectStartMessage::CheckIpConnectSpeedToHigh(ip,send) => {
                let _ = send.send(self.ban_ip.contains_key(&ip));
                return Ok(false);
            }
        }
    }
}
