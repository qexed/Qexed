use std::thread;

use qexed_config::tool::AppConfigTrait;
use rust_i18n::t;
rust_i18n::i18n!("../../locales");
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = qexed_config::app::qexed::Qexed::load_or_create_default()?;
    rust_i18n::set_locale(&config.language);
    qexed_log::log_init().await;
    log::info!("{}", t!("qexed.log_init_finish"));
    // 检查更新
    tokio::spawn(qexed_update_check::check_version(
        config.update_check.clone(),
    ));
    // 安全性检测(暂时没那么高级)
    thread::spawn(qexed_safe_check::root_check::root_check);
    // 插件更新检查
    // 适用于群组服预配置插件列表的服务，也适用于插件更新
    match qexed_plugin_manage::update_plugins_check(&config.plugin_download).await {
        Ok(v) => {
            if !v {
                log::error!("{}",t!("qexed.plugim_update_check_error_by_false"));
                return Ok(());
            }
        }
        Err(err) => {
            log::error!("{}",t!("qexed.plugim_update_check_error", err=err));
            return Err(err);
        }
    }
    // 启动 Tcp 服务器
    let tcp_server = match tokio::net::TcpListener::bind(config.server.ip.clone()).await {
        Ok(v) => {
            log::info!("{}",t!("qexed.listen_ip",ip=config.server.ip.clone()));
            v
        }
        Err(err) => match err.kind() {
            std::io::ErrorKind::AddrInUse => {
                log::error!("{}",t!("qexed.address_in_use",ip=config.server.ip));
                return Err(err.into());
            }
            _ => {
                log::error!("{}",t!("qexed.bind_failed",err=err));
                return Err(err.into());
            }
        },
    };
    if !config.server.online {
        log::warn!("{}",t!("qexed.minecraft_warning.offline_mode"));
        log::warn!("{}",t!("qexed.minecraft_warning.no_authentication"));
        log::warn!("{}",t!("qexed.minecraft_warning.hacker_risk"));
        log::warn!("{}",t!("qexed.minecraft_warning.set_online_mode"));
    }
    let _ = tcp_server;
    loop {}
}

// [22:00:44 WARN]: **** SERVER IS RUNNING IN OFFLINE/INSECURE MODE!
// [22:00:44 WARN]: The server will make no attempt to authenticate usernames. Beware.
// [22:00:44 WARN]: While this makes the game possible to play without internet access, it also opens up the ability for hackers to connect with any username they choose.
// [22:00:44 WARN]: To change this, set "online-mode" to "true" in the server.properties file.
