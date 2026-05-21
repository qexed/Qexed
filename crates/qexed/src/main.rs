use clap::Parser;

use qexed_config::tool::AppConfigTrait;
use qexed_protocol::to_server::handshaking::set_protocol::SetProtocol;
use rust_i18n::t;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_stream::StreamExt;

rust_i18n::i18n!("locales");
shadow_rs::shadow!(build);
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match run().await {
        Ok(_) => {
            log::debug!("exit");
        }
        Err(err) => {
            log::error!("{}", err);
        }
    };
    Ok(())
}
async fn run() -> anyhow::Result<()> {
    let mut _build_type = "";
    #[cfg(debug_assertions)]
    {
        _build_type = "dev-";
    }
    let args = qexed_config::app::qexed::qexed_args::ServerArgs::parse();
    let config = qexed_config::app::qexed::Qexed::load_or_create_default(args.language,None,None)?;
    
    rust_i18n::set_locale(&match args.language{
        Some(v)=>v,
        None=>config.language,
    });
    if args.init_settings {
        return Ok(());
    }
    qexed_log::log_init().await;
    let info = os_info::get();
    log::info!(
        "{}",
        t!(
            "qexed.system_running",
            system = info.os_type(),
            version = info.version(),
            arch = std::env::consts::ARCH
        )
    );
    log::info!(
        "{}",
        t!(
            "qexed.loading",
            name = env!("CARGO_PKG_NAME"),
            tag = {
                if shadow_rs::tag() != "" {
                    shadow_rs::tag()
                } else {
                    "?".to_string()
                }
            },
            build_type = _build_type,
            branch = shadow_rs::branch(),
            commit_hash = build::SHORT_COMMIT,
            mc_version = qexed_config::MC_VERSION,
        )
    );
    log::info!("{}", t!("qexed.log_init_finish"));
    if !config.server.online {
        log::warn!("{}", t!("qexed.minecraft_warning.offline_mode"));
        log::warn!("{}", t!("qexed.minecraft_warning.no_authentication"));
        log::warn!("{}", t!("qexed.minecraft_warning.hacker_risk"));
        log::warn!("{}", t!("qexed.minecraft_warning.set_online_mode"));
    }
    let tcp_server: TcpListener = qexed_tcp_connect::bind(&config.server.ip).await?;
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(
        config.server.max_port_connections as usize,
    ));
    let max_port_connections = config.server.max_port_connections as usize;
    while let Ok((mut stream, addr)) = tcp_server.accept().await {
        let permit = semaphore.clone().acquire_owned().await?;
        log::debug!(
            "{}",
            t!(
                "qexed.debug.new_tcp_connection",
                local = stream.local_addr()?,
                peer = addr,
                count = max_port_connections - semaphore.available_permits()
            )
        );
        tokio::spawn(async move {
            let result: anyhow::Result<()> = async {
                let (r, w) = tokio::io::split(stream);
                let mut packet_read = qexed_tcp_connect::PacketStream::new(r);
                let mut part0 = true;
                while let Some(result) = packet_read.next().await {
                    match result {
                        Ok(mut packet) => {
                            let mut reader = qexed_packet::PacketReader::new(&mut packet);
                            if part0 {
                                qexed_packet_macros::smatch!(
                                    reader,
                                    packet,
                                    SetProtocol=>{
                                        log::info!("心跳数据包:{:?}", packet);
                                        part0=false;
                                    },
                                    _=>{
                                      log::info!("未知的数据包")  
                                    }
                                );
                            }
                        }
                        Err(e) => {
                            log::error!(
                                "{}",
                                t!("qexed.tcp_connect_read_packet_error", ip = addr, err = e)
                            );
                            break;
                        }
                    }
                }
                Ok(())
            }
            .await;
            if let Err(e) = result {
                log::error!("连接处理错误: {}", e);
            }
            log::info!("连接关闭: {addr}");
            drop(permit);
        });
    }
    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
