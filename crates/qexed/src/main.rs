use clap::Parser;
use qexed_club::{arg_fields, ServerArgs};
use shadow_rs::shadow;
use serde::Serialize;

shadow!(shadow);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DocApiVersion {
    hash: &'static str,
    version: &'static str,
    build_date: String,
    args: Vec<qexed_club::ArgField>,
}
fn main() -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(runtime_worker_threads())
        .enable_all()
        .build()?
        .block_on(async_main())
}

async fn async_main() -> anyhow::Result<()> {
    if let Err(err) = run().await {
        log::error!("{err}");
    }
    Ok(())
}

/// shadow 的 COMMIT_DATE 形如 "2026-10-04 02:59:53 +08:00"；文档客户端约定纯日期，
/// 截取前 10 个 ASCII 字符；无 git 信息（"unknown" 等）时原样返回。
fn build_date_of(commit_date: &'static str) -> String {
    if commit_date.len() >= 10 && commit_date.as_bytes()[4] == b'-' {
        commit_date[..10].to_string()
    } else {
        commit_date.to_string()
    }
}

fn runtime_worker_threads() -> usize {
    std::env::var("QEXED_WORKER_THREADS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(2)
}

async fn run() -> anyhow::Result<()> {
    let args = ServerArgs::parse();
    if args.doc_api_version {
        let payload = DocApiVersion {
            hash: shadow::SHORT_COMMIT,
            version: shadow::PKG_VERSION,
            build_date: build_date_of(shadow::COMMIT_DATE),
            args: arg_fields(),
        };
        println!("{}", serde_json::to_string(&payload).expect("doc api version json"));
        return Ok(());
    }
    // 初始化
    qexed_config::init_config_path(args.config_path)?;
    qexed_log::init().await?;
    qexed_language::init(shadow::SHORT_COMMIT, args.language.as_deref()).await?;
    // loop{
    //     log::info!("test");
    //     std::thread::sleep(std::time::Duration::from_secs(5)); // 等待 5 秒
    // }
    Ok(())
}
