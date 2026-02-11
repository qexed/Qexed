pub mod message;
mod engine;
rust_i18n::i18n!("../../locales");
use std::net::SocketAddr;

use qexed_config::tool::AppConfigTrait;
use rust_i18n::t;
use tokio::{net::TcpStream, sync::mpsc::UnboundedSender};

use crate::{engine::{Engine, NoEngine, simple::SimpleEngine}, message::Message};
pub async fn new(handshaking_packet_split_api: UnboundedSender<(TcpStream, SocketAddr)>) -> anyhow::Result<tokio::sync::mpsc::UnboundedSender<message::Message>> {
    let config: qexed_config::app::qexed_ip_connection_speed_test::QexedIpConnectionSpeedTest = match qexed_config::app::qexed_ip_connection_speed_test::QexedIpConnectionSpeedTest::load_or_create_default(){
        Ok(v)=>v,
        Err(err)=>{
            log::error!("{}",t!("config_load_error",err=err));
            return Err(err);
        }
    };
    match config.data.storage_engine {
        qexed_config::public::storage_engine::StorageEngine::Simple => {
            log::warn!("{}", t!("qexed_ip_connection_speed_test.simple_warning"));
            log::warn!("{}", t!("qexed_ip_connection_speed_test.simple_warning2"));
        }
        qexed_config::public::storage_engine::StorageEngine::Pika => {}
        _ => {
            log::error!(
                "{}",
                t!("qexed_ip_connection_speed_test.engine_incompatible")
            );
            return Err(anyhow::anyhow!(
                "{}",
                t!("qexed_ip_connection_speed_test.engine_incompatible")
            ));
        }
    };
    log::info!("{}", t!("config_init_finish"));
    
    let engine_server: Box<dyn Engine + Send + Sync> = match config.enable {
        false=>{Box::new(NoEngine::default())}
        true => {
            match config.data.storage_engine {
                qexed_config::public::storage_engine::StorageEngine::Simple => {
                    Box::new(SimpleEngine::new(config.clone()))
                },
                qexed_config::public::storage_engine::StorageEngine::Pika => {
                    return Err(anyhow::anyhow!("当前版本不支持Pika数据库，等待兼容"));
                },
                _ =>{
                    Box::new(NoEngine::default())
                }
            }
        },
    };
    let (s, r) = tokio::sync::mpsc::unbounded_channel();
    Server::new(r, config,handshaking_packet_split_api,engine_server);
    Ok(s)
}
pub struct Server {
    config: qexed_config::app::qexed_ip_connection_speed_test::QexedIpConnectionSpeedTest,
    handshaking_packet_split_api: UnboundedSender<(TcpStream, SocketAddr)>,
    engine:Box<dyn engine::Engine + Send + Sync>,
}
impl Server {
    pub fn new(
        r: tokio::sync::mpsc::UnboundedReceiver<Message>,
        config: qexed_config::app::qexed_ip_connection_speed_test::QexedIpConnectionSpeedTest,
        handshaking_packet_split_api: UnboundedSender<(TcpStream, SocketAddr)>,
        engine: Box<dyn engine::Engine + Send + Sync>
    ) -> tokio::task::JoinHandle<()> {

        let server = Self {
            config:config,
            engine:engine,
            handshaking_packet_split_api:handshaking_packet_split_api,
        };
        tokio::spawn(server.listen(r))
    }
    pub async fn listen(mut self, mut r: tokio::sync::mpsc::UnboundedReceiver<Message>) {
        while let Some(event) = r.recv().await {
            match event {
                Message::NewConnect(tcp_stream, socket_addr) => {
                    // log::debug!("{}", t!("qexed.new_tcp_connection", addr = socket_addr));
                    if self.config.enable{
                        if let Err(err) = self.engine.check_ip(socket_addr).await{
                            log::warn!("{}",err);
                            if tcp_stream.set_linger(Some(std::time::Duration::from_nanos(1))).is_err(){
                                continue;
                            }
                            if tcp_stream.set_nodelay(true).is_err(){
                                continue;
                            }
                        }
                    }
                    match self.handshaking_packet_split_api.send((tcp_stream,socket_addr)){
                        Ok(_)=>{},
                        // 这里不应该报错的,报错了后面就无法运行了
                        Err(err)=>panic!("{}", err),
                    };
                }
            }
        }
    }
}
