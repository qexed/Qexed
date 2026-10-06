//! 启动引导：配置装载、日志初始化、Mojang 数据就绪检查、l10n 初始化。
//! 迁移自 v4 crates/qexed/src/bootstrap.rs。
//!
//! 适配差异（v6）：
//! - v4 RuntimeConfig（几十个 app 配置聚合）不存在；v6 各域配置由各 crate
//!   自行 load_and_create_default，bootstrap 只负责 server 域 + 通用初始化。
//! - rust_i18n::set_locale → qexed_language::init（本体组装层已调用，
//!   bootstrap 只做语言覆盖检查）。
//! - os_info / shadow_rs / windows_sys 依赖未引：系统信息与构建信息日志、
//!   Windows 控制台标题设置留 TODO（组装层可自行打印）。
//! - qexed_mojang_data::registry_sync::ensure_data_ready 接管 v4 registry_sync 的就绪检查。

use qexed_config::Config;

use crate::config::ServerConfig;
use crate::error::Result;

/// 启动引导结果：init_settings 时返回 None（只落配置文件即退出）。
pub struct Bootstrap {
    pub server_config: ServerConfig,
}

/// v4 bootstrap::load 的 v6 版本。
///
/// * `language_override` —— --language 参数，覆盖配置文件语言。
/// * `init_settings` —— 只初始化配置文件后退出。
pub async fn load(language_override: Option<&str>, init_settings: bool) -> Result<Option<Bootstrap>> {
    let mut server_config = ServerConfig::load_and_create_default(true)?;

    if let Some(language) = language_override {
        server_config.language = language.to_string();
        ServerConfig::save_file(&server_config)?;
    }

    if init_settings {
        return Ok(None);
    }

    log_config_info(&server_config);
    log_online_warnings(&server_config);
    ensure_mojang_data();
    initialize_l10n();

    Ok(Some(Bootstrap { server_config }))
}

fn log_config_info(config: &ServerConfig) {
    let display_path = qexed_config::config_path()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "./config".to_string());
    log::info!(
        "{}",
        qexed_language::t("qexed.server.bootstrap.config_path").replace("%{path}", &display_path)
    );
    log::info!(
        "{}",
        qexed_language::t("qexed.server.bootstrap.server_config")
            .replace("%{ip}", &config.ip)
            .replace("%{max_player}", &config.max_player.to_string())
            .replace("%{online_mode}", &config.online_mode.to_string())
    );
}

fn log_online_warnings(config: &ServerConfig) {
    if !config.online_mode && !config.proxy {
        log::warn!("{}", qexed_language::t("qexed.server.warning.offline_mode"));
        log::warn!("{}", qexed_language::t("qexed.server.warning.no_authentication"));
        log::warn!("{}", qexed_language::t("qexed.server.warning.hacker_risk"));
        log::warn!("{}", qexed_language::t("qexed.server.warning.set_online_mode"));
    }
}

fn ensure_mojang_data() {
    if let Err(err) = qexed_mojang_data::registry_sync::ensure_data_ready() {
        log::warn!(
            "{}",
            qexed_language::t("qexed.server.bootstrap.mojang_data_missing")
                .replace("%{error}", &err.to_string())
        );
    }
}

/// 用 Mojang 语言文件初始化 l10n 模块（缓存目录：<cwd>/cache/mojang/<ver>/data/minecraft/lang）。
fn initialize_l10n() {
    let fallback_language = "en_us".to_string();
    let version = qexed_config::MC_VERSION;
    let candidates = [
        std::path::PathBuf::from(".")
            .join("cache/mojang")
            .join(version)
            .join("data/minecraft/lang"),
        std::path::PathBuf::from(".").join("assets/vanilla_json/minecraft/lang"),
    ];
    for lang_dir in &candidates {
        if lang_dir.is_dir() {
            if let Err(err) = crate::l10n::initialize(lang_dir, &fallback_language) {
                log::warn!(
                    "{}",
                    qexed_language::t("qexed.server.l10n.init_failed")
                        .replace("%{error}", &err.to_string())
                );
            }
            return;
        }
    }
    log::warn!("{}", qexed_language::t("qexed.server.l10n.no_lang_dir"));
}