pub mod config;
pub mod error;

use std::io::Write;

use config::LogConfig;
use qexed_config::Config;
use tklog::ASYNC_LOG;

// 全局日志处理服务
pub async fn init() -> Result<(), error::LogError> {
    // 文档站产物：schema JSON（key 指向文档站正文）
    // let schema = LogConfig::schema_json();
    let config = LogConfig::load_and_create_default(true)?;
    let level = config.level.as_tklog();
    ASYNC_LOG
        .set_level(level)
        .set_cutmode_by_time(
            &config.filename,
            config.mode.as_tklog(),
            config.maxbackups,
            config.compress,
        )
        .await;
    if config.json {
        ASYNC_LOG.set_formatter("{\"level\":\"{level}\",\"time\":\"{time}\",\"file\":\"{file}\",\"message\":\"{message}\"}\n");
    } else {
        ASYNC_LOG.set_console(false).set_custom_handler(log_handler);
    }

    // println!("{schema}");
    ASYNC_LOG.uselog();
    Ok(())
}
fn log_handler(log: &tklog::LogContext) -> bool {
    if log.level == tklog::LEVEL::Off {
        return false;
    }

    let time = chrono::Local::now().format("%H:%M:%S");
    if log.modname == "qexed_wasm_runtime::modern::log" {
        let prefix = format!("{time} [{}] ", format_level(&log.level));
        write_console_log(&prefix, &log.log_body);
    } else {
        let package_name = log.modname.split("::").next().unwrap_or(&log.modname);
        let prefix = format!(
            "{time} [{}]{} [{}] ",
            format_level(&log.level),
            format_level_space(&log.level),
            qexed_language::t(package_name),
        );
        write_console_log(&prefix, &log.log_body);
    }

    true
}

fn write_console_log(prefix: &str, body: &str) {
    let body = body.trim_end_matches(['\r', '\n']);
    let mut stdout = std::io::stdout().lock();

    if body.is_empty() {
        let _ = writeln!(stdout, "{}", prefix.trim_end());
        let _ = stdout.flush();
        return;
    }

    for line in body.lines() {
        let _ = writeln!(stdout, "{prefix}{line}");
    }

    let _ = stdout.flush();
}

fn format_level(level: &tklog::LEVEL) -> colored::ColoredString {
    match level {
        tklog::LEVEL::Trace => colored::Colorize::magenta("TRACE"),
        tklog::LEVEL::Debug => colored::Colorize::blue("DEBUG"),
        tklog::LEVEL::Info => colored::Colorize::green("INFO"),
        tklog::LEVEL::Warn => colored::Colorize::yellow("WARN"),
        tklog::LEVEL::Error => colored::Colorize::bright_red("ERROR"),
        tklog::LEVEL::Fatal => colored::Colorize::on_bright_red("FATAL"),
        tklog::LEVEL::Off => colored::Colorize::magenta("Off"),
    }
}

fn format_level_space(level: &tklog::LEVEL) -> &str {
    match level {
        tklog::LEVEL::Info | tklog::LEVEL::Warn => " ",
        _ => "",
    }
}
