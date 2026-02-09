include!(concat!(env!("OUT_DIR"), "/generated_log_config.rs"));
use rust_i18n::t;
use tklog::{ASYNC_LOG, LEVEL, MODE};
use std::sync::atomic::{AtomicU64, AtomicU32, Ordering};
use std::hash::{Hash, Hasher};
use std::mem::discriminant;
use std::collections::hash_map::DefaultHasher;
use std::cell::RefCell;
use std::time::{SystemTime, UNIX_EPOCH};

rust_i18n::i18n!("../../locales");

// 全局原子状态
static REPEAT_COUNT: AtomicU32 = AtomicU32::new(0);
static LAST_LOG_HASH: AtomicU64 = AtomicU64::new(0);
static LAST_LOG_TIMESTAMP: AtomicU64 = AtomicU64::new(0);

// 使用 RefCell 实现内部可变性
thread_local! {
    static THREAD_LOG_STATE: RefCell<LogState> = RefCell::new(LogState::new());
}

struct LogState {
    count: u32,
    last_hash: u64,
    last_timestamp: u64,
    last_level: String,
    last_pure_level: String, // 新增：存储纯日志级别
    last_modname: String,
    last_body: String,
    last_formatted_time: String,
}

impl LogState {
    fn new() -> Self {
        Self {
            count: 0,
            last_hash: 0,
            last_timestamp: 0,
            last_level: String::new(),
            last_pure_level: String::new(), // 初始化纯级别
            last_modname: String::new(),
            last_body: String::new(),
            last_formatted_time: String::new(),
        }
    }
}

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

    THREAD_LOG_STATE.with(|state_cell| {
        let mut state = state_cell.borrow_mut();
        let current_hash = calculate_log_hash(&log.level, &log.modname, &log.log_body);
        let current_timestamp = get_current_timestamp();
        
        let last_hash = LAST_LOG_HASH.load(Ordering::Acquire);
        let last_timestamp = LAST_LOG_TIMESTAMP.load(Ordering::Acquire);
        
        // 检查是否为重复日志（内容相同）
        if last_hash == current_hash {
            let count = REPEAT_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
            state.count = count;
            
            // 更新时间戳但保持内容不变
            state.last_timestamp = current_timestamp;
            LAST_LOG_TIMESTAMP.store(current_timestamp, Ordering::Release);
            
            // 生成新的时间格式
            state.last_formatted_time = format_current_time();
            
            // 输出带新时间戳的重复日志摘要
            display_repeat_log_with_updated_time(&state);
            return true;
        }
        
        // 如果不是重复日志，先显示之前的重复摘要（如果有）
        if state.count > 1 {
            display_repeat_summary(&state);
        }
        
        // 更新状态为新日志
        update_log_state(&mut state, current_hash, current_timestamp, log);
        display_current_log(log);
        
        true
    })
}

fn calculate_log_hash(level: &tklog::LEVEL, modname: &str, body: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    discriminant(level).hash(&mut hasher);
    modname.hash(&mut hasher);
    body.hash(&mut hasher);
    hasher.finish()
}

fn get_current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn format_current_time() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

fn update_log_state(state: &mut LogState, hash: u64, timestamp: u64, log: &tklog::LogContext) {
    state.count = 1;
    state.last_hash = hash;
    state.last_timestamp = timestamp;
    state.last_level = format_level(&log.level).to_string();
    state.last_pure_level = match log.level {
        LEVEL::Trace => "TRACE",
        LEVEL::Debug => "DEBUG", 
        LEVEL::Info => "INFO",
        LEVEL::Warn => "WARN",
        LEVEL::Error => "ERROR",
        LEVEL::Fatal => "FATAL",
        LEVEL::Off => "Off",
    }.to_string();
    state.last_modname = log.modname.to_string();
    state.last_body = log.log_body.to_string();
    state.last_formatted_time = format_current_time();
    
    LAST_LOG_HASH.store(hash, Ordering::Release);
    LAST_LOG_TIMESTAMP.store(timestamp, Ordering::Release);
    REPEAT_COUNT.store(1, Ordering::Relaxed);
}

fn display_repeat_log_with_updated_time(state: &LogState) {
    let current_time = format_current_time();
    let indent = level_indent_from_str(&state.last_pure_level); // 使用纯级别
    
    if state.last_modname == "qexed_wasm_runtime::modern::log" {
        println!(
            "{} [{}]{} {} (repeated {} times)",
            current_time,
            state.last_level,  // 保持带颜色的输出
            indent,
            state.last_body,
            state.count
        );
    } else {
        let package_name = state.last_modname.split("::").next().unwrap_or(&state.last_modname);
        println!(
            "{} [{}]{} [{}] {} (repeated {} times)",
            current_time,
            state.last_level,  // 保持带颜色的输出
            indent,
            t!(format!("qexed_log.modern.{}", package_name)),
            state.last_body,
            state.count
        );
    }
}

fn display_repeat_summary(state: &LogState) {
    if state.last_modname == "qexed_wasm_runtime::modern::log" {
        println!(
            "{} [{}] Last message repeated {} times",
            state.last_formatted_time,
            state.last_level,
            state.count
        );
    } else {
        let package_name = state.last_modname.split("::").next().unwrap_or(&state.last_modname);
        println!(
            "{} [{}] [{}] Last message repeated {} times",
            state.last_formatted_time,
            state.last_level,
            t!(format!("qexed_log.modern.{}", package_name)),
            state.count
        );
    }
}

fn display_current_log(log: &tklog::LogContext) {
    let current_time = format_current_time();
    
    if log.modname == "qexed_wasm_runtime::modern::log" {
        println!(
            "{} [{}]{} {}",
            current_time,
            format_level(&log.level),
            level_indent(&log.level),
            log.log_body
        );
    } else {
        let package_name = log.modname.split("::").next().unwrap_or(&log.modname);
        println!(
            "{} [{}]{} [{}] {}",
            current_time,
            format_level(&log.level),
            level_indent(&log.level),
            t!(format!("qexed_log.modern.{}", package_name)),
            log.log_body
        );
    }
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

fn level_indent(level: &tklog::LEVEL) -> &str {
    match level {
        LEVEL::Info => " ",
        LEVEL::Warn => " ",
        LEVEL::Off => "  ",
        _ => ""
    }
}

// 修复后的函数：使用纯日志级别字符串
fn level_indent_from_str(level: &str) -> &str {
    match level {
        "INFO" => " ",
        "WARN" => " ",
        "Off" => "  ",
        _ => ""
    }
}

pub fn cleanup_log_state() {
    LAST_LOG_HASH.store(0, Ordering::Release);
    LAST_LOG_TIMESTAMP.store(0, Ordering::Release);
    REPEAT_COUNT.store(0, Ordering::Relaxed);
}