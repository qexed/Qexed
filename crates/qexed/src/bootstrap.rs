use clap::Parser;
use qexed_config::tool::AppConfigTrait;
use rust_i18n::t;

pub async fn load() -> anyhow::Result<Option<qexed_config::app::qexed::Qexed>> {
    let args = qexed_config::app::qexed::qexed_args::ServerArgs::parse();
    let config =
        qexed_config::app::qexed::Qexed::load_or_create_default(args.language.clone(), None, None)?;

    rust_i18n::set_locale(&args.language.unwrap_or_else(|| config.language.clone()));
    if args.init_settings {
        return Ok(None);
    }
    #[cfg(windows)]
    set_windows_console_title("Qexed Server");
    qexed_log::log_init().await;
    log_runtime_info();
    log_online_warnings(&config);

    Ok(Some(config))
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

fn log_online_warnings(config: &qexed_config::app::qexed::Qexed) {
    if !config.server.online_mode {
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
