use std::thread;

use qexed_config::tool::AppConfigTrait;
use rust_i18n::t;
rust_i18n::i18n!("../../locales");
shadow_rs::shadow!(build);
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match run().await{
        Ok(_) => {
            log::debug!("exit");
        },
        Err(err) => {
            log::error!("{}",t!("qexed.error_exit",err=err));
        },
    };
    Ok(())
}
async fn run() -> anyhow::Result<()> {
    let mut _build_type = "";
    #[cfg(debug_assertions)]
    {
        _build_type = "dev-";
    }
    let config = qexed_config::app::qexed::Qexed::load_or_create_default()?;
    rust_i18n::set_locale(&config.language);
    qexed_log::log_init().await;
    // Print full information:
    let info = os_info::get();
    log::info!("{}",t!("qexed.system_running",system=info.os_type(),version=info.version(),arch=std::env::consts::ARCH));
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
    // 检查更新
    tokio::spawn(qexed_update_check::check_version(
        config.update_check.clone(),
    ));
    // 安全性检测(暂时没那么高级)
    thread::spawn(qexed_safe_check::root_check::root_check);
    // 插件初始化
    let plugin_manage = qexed_plugin_manage::new().await?;
    
    // 插件更新检查
    // 适用于群组服预配置插件列表的服务，也适用于插件更新
    match plugin_manage.update_plugins_check(&config.plugin_download).await {
        Ok(v) => {
            if !v {
                log::error!("{}", t!("qexed.plugin_update_check_error_by_false"));
            }
        }
        Err(err) => {
            log::error!("{}", t!("qexed.plugin_update_check_error", err = err));
        }
    };
    // 初始化其他服务
    let (_warden_api,ip_connect_speed_test_api) = match tokio::try_join!(
        qexed_warden::new(),
        qexed_ip_connection_speed_test::new(),
    ){
        Ok(v)=>v,
        Err(err)=>{
            log::error!("{}",err);
            return Err(err);
        }
    };
    log::info!("{}", t!("qexed.modern_init_start"));
    // let (a,b) = (api.0??,api.1??);
    log::info!("{}", t!("qexed.modern_init_finish"));
    // 启动 Tcp 服务器
    let tcp_server = match tokio::net::TcpListener::bind(config.server.ip.clone()).await {
        Ok(v) => {
            log::info!("{}", t!("qexed.listen_ip", ip = config.server.ip.clone()));
            v
        }
        Err(err) => match err.kind() {
            std::io::ErrorKind::AddrInUse => {
                log::error!("{}", t!("qexed.address_in_use", ip = config.server.ip));
                return Err(err.into());
            }
            _ => {
                log::error!("{}", t!("qexed.bind_failed", err = err));
                return Err(err.into());
            }
        },
    };
    if !config.server.online {
        log::warn!("{}", t!("qexed.minecraft_warning.offline_mode"));
        log::warn!("{}", t!("qexed.minecraft_warning.no_authentication"));
        log::warn!("{}", t!("qexed.minecraft_warning.hacker_risk"));
        log::warn!("{}", t!("qexed.minecraft_warning.set_online_mode"));
    }

    loop {
        let (socket, addr) = tcp_server.accept().await?;
        ip_connect_speed_test_api.send(qexed_ip_connection_speed_test::message::Message::NewConnect(socket, addr))?;
    }
}
