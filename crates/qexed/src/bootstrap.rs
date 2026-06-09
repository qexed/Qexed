use rust_i18n::t;

pub async fn load(
    args: &qexed_config::app::qexed::qexed_args::ServerArgs,
) -> anyhow::Result<Option<crate::config::RuntimeConfig>> {
    let config =
        crate::config::RuntimeConfig::load(args.language.clone(), args.config_path.clone())?;

    rust_i18n::set_locale(
        &args
            .language
            .clone()
            .unwrap_or_else(|| config.language.clone()),
    );
    if args.init_settings {
        return Ok(None);
    }
    #[cfg(windows)]
    set_windows_console_title("Qexed Server");
    qexed_log::log_init(log_level_from_config(config.server.log_level)).await;
    log_runtime_info();
    log_config_info(args, &config);
    log_online_warnings(&config);
    crate::registry_sync::configure_mojang_cache_path(&config.server.mojang_cache_path);
    crate::registry_sync::ensure_data_ready()?;

    Ok(Some(config))
}

fn log_config_info(
    args: &qexed_config::app::qexed::qexed_args::ServerArgs,
    config: &crate::config::RuntimeConfig,
) {
    let config_path = args
        .config_path
        .clone()
        .unwrap_or_else(|| std::path::PathBuf::from("./config"));
    let display_path = std::path::absolute(&config_path).unwrap_or(config_path);
    log::info!("config path: {}", display_path.display());
    log::info!(
        "entity config: enable={}, spawning_enable={}, spawn_rules={}",
        config.server.entities.enable,
        config.server.entities.spawning.enable,
        config.server.entities.spawning.rules.len()
    );
}

fn log_level_from_config(
    level: qexed_config::app::qexed::server::ServerLogLevel,
) -> qexed_log::LogLevel {
    match level {
        qexed_config::app::qexed::server::ServerLogLevel::Trace => qexed_log::LogLevel::Trace,
        qexed_config::app::qexed::server::ServerLogLevel::Debug => qexed_log::LogLevel::Debug,
        qexed_config::app::qexed::server::ServerLogLevel::Info => qexed_log::LogLevel::Info,
        qexed_config::app::qexed::server::ServerLogLevel::Warn => qexed_log::LogLevel::Warn,
        qexed_config::app::qexed::server::ServerLogLevel::Error => qexed_log::LogLevel::Error,
        qexed_config::app::qexed::server::ServerLogLevel::Off => qexed_log::LogLevel::Off,
    }
}

fn log_runtime_info() {
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
            build_type = build_type(),
            branch = shadow_rs::branch(),
            commit_hash = crate::build::SHORT_COMMIT,
            mc_version = qexed_config::MC_VERSION,
        )
    );
    log::info!("{}", t!("qexed.log_init_finish"));
}

fn log_online_warnings(config: &crate::config::RuntimeConfig) {
    if !config.server.online_mode && !config.server.proxy {
        log::warn!("{}", t!("qexed.minecraft_warning.offline_mode"));
        log::warn!("{}", t!("qexed.minecraft_warning.no_authentication"));
        log::warn!("{}", t!("qexed.minecraft_warning.hacker_risk"));
        log::warn!("{}", t!("qexed.minecraft_warning.set_online_mode"));
    }
}

fn build_type() -> &'static str {
    if cfg!(debug_assertions) { "dev-" } else { "" }
}
#[cfg(windows)]
fn set_windows_console_title(title: &str) {
    use windows_sys::Win32::System::Console::SetConsoleTitleW;

    // 将 Rust 字符串转换为 Windows 需要的宽字符指针 (UTF-16)
    let wide_title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();

    unsafe {
        // 直接调用 Windows API
        SetConsoleTitleW(wide_title.as_ptr());
    }
}

// 防止在非 Windows 平台编译报错，定义一个空函数
#[cfg(not(windows))]
fn set_windows_console_title(_title: &str) {}
