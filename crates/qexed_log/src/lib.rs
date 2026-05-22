include!(concat!(env!("OUT_DIR"), "/generated_log_config.rs"));

use std::io::{self, Write};

use rust_i18n::t;
use tklog::{ASYNC_LOG, LEVEL, MODE};

rust_i18n::i18n!("locales");

pub async fn log_init() {
    ASYNC_LOG
        .set_console(false)
        .set_level(LEVEL::Info)
        .set_cutmode_by_time(
            &format!("./logs/{}.log", t!("qexed_log.modern.global")),
            MODE::DAY,
            30,
            true,
        )
        .await
        .set_formatter(&format!(
            "{{level}} [{{time}}]: [{}] {{file}} {{message}}\n",
            t!("qexed_log.modern.global")
        ))
        .set_custom_handler(log_handler);
    module().await;
    ASYNC_LOG.uselog();
}

fn log_handler(log: &tklog::LogContext) -> bool {
    if log.level == tklog::LEVEL::Off {
        return false;
    }

    if !qexed_log_console_enabled(&log.modname) {
        return true;
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
            t!(format!("qexed_log.modern.{}", package_name)),
        );
        write_console_log(&prefix, &log.log_body);
    }

    true
}

fn write_console_log(prefix: &str, body: &str) {
    let body = body.trim_end_matches(['\r', '\n']);
    let mut stdout = io::stdout().lock();

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
        LEVEL::Trace => colored::Colorize::magenta("TRACE"),
        LEVEL::Debug => colored::Colorize::blue("DEBUG"),
        LEVEL::Info => colored::Colorize::green("INFO"),
        LEVEL::Warn => colored::Colorize::yellow("WARN"),
        LEVEL::Error => colored::Colorize::bright_red("ERROR"),
        LEVEL::Fatal => colored::Colorize::on_bright_red("FATAL"),
        LEVEL::Off => colored::Colorize::magenta("Off"),
    }
}

fn format_level_space(level: &tklog::LEVEL) -> &str {
    match level {
        LEVEL::Info | LEVEL::Warn => " ",
        _ => "",
    }
}
