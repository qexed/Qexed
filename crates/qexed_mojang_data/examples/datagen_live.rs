//! 手动集成验证：跑完整 init()（data + reports + JRE 自动下载）。
//! 运行：cargo run -p qexed_mojang_data --example datagen_live

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 简易日志：让 lib 里的 log 宏有输出
    let _ = simple_logger();
    // 配置根：仓库 run/config（与 qexed 主程序一致的行为）
    let config_root = std::env::current_dir()?.join("run").join("config");
    qexed_config::init_config_path(config_root)?;

    println!("== qexed_mojang_data init() 开始 ==");
    let started = std::time::Instant::now();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(async_main())?;
    println!("== 完成，耗时 {:?} ==", started.elapsed());

    // 展示 reports
    let dir = qexed_mojang_data::reports_dir()?;
    println!("reports 目录: {}", dir.display());
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        println!(
            "  {:<18} {} bytes",
            entry.file_name().to_string_lossy(),
            entry.metadata()?.len()
        );
    }
    Ok(())
}

async fn async_main() -> Result<(), qexed_mojang_data::error::MojangDataError> {
    qexed_mojang_data::init().await
}

/// log 简易落地：没有初始化真正的 logger 时 log::info 全部被吞。
/// 用一个极简 env_logger 语义：直接打印到 stderr。
fn simple_logger() -> bool {
    struct PrintLogger;
    impl log::Log for PrintLogger {
        fn enabled(&self, metadata: &log::Metadata) -> bool {
            metadata.level() <= log::Level::Info
        }
        fn log(&self, record: &log::Record) {
            if self.enabled(record.metadata()) {
                eprintln!("[{}] {}", record.level(), record.args());
            }
        }
        fn flush(&self) {}
    }
    let ok = log::set_boxed_logger(Box::new(PrintLogger)).is_ok();
    log::set_max_level(log::LevelFilter::Info);
    ok
}
