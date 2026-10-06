mod play_boot;
mod world_adapter;
mod runtime;

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
    chdir_workspace_run_if_cargo()?;
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(runtime_worker_threads())
        .enable_all()
        .build()?
        .block_on(async_main())
}

async fn async_main() -> anyhow::Result<()> {
    if let Err(err) = run().await {
        log::error!("{}", qexed_language::t("qexed.runtime.error").replace("%{error}", &err.to_string()));
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

/// `cargo run --bin qexed` 在仓库根目录时切到 `run/`。
/// 直接运行 qexed.exe（debug / release）不改工作目录。
fn chdir_workspace_run_if_cargo() -> anyhow::Result<()> {
    if std::env::var_os("CARGO").is_none() {
        return Ok(());
    }
    let cwd = normalize_dir(std::env::current_dir()?);
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let workspace = normalize_dir(workspace);
    if cwd != workspace {
        return Ok(());
    }
    let run_dir = workspace.join("run");
    std::fs::create_dir_all(&run_dir)?;
    std::env::set_current_dir(&run_dir)?;
    Ok(())
}

fn normalize_dir(path: impl AsRef<std::path::Path>) -> std::path::PathBuf {
    let path = path.as_ref();
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let text = canonical.to_string_lossy();
    let prefix = ['\\', '\\', '?', '\\'].into_iter().collect::<String>();
    let stripped = text.strip_prefix(&prefix).unwrap_or(text.as_ref());
    std::path::PathBuf::from(stripped)
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
    qexed_mojang_data::init().await?;
    qexed_mojang_data::registry_sync::ensure_data_ready()?;

    // 组装真实运行时并启动服务器主循环
    let server_config = qexed_server::config::ServerConfig::default();
    let services = std::sync::Arc::new(qexed_server::context::ServerServices::load());
    // 连接配置：优先 connection.toml；缺省时用离线模式（本地可连）。
    let mut connection_config = qexed_connection::config::ConnectionConfig::default();
    connection_config.online_mode = false;
    // TODO(config): app_config 宏生成的加载入口接线后，从 connection.toml 覆盖
    let runtime = std::sync::Arc::new(runtime::QexedRuntime::new(
        server_config.clone(),
        "./world",
    )?);
    let mut connection_config = connection_config;
    // 组装层注入 play 启动器（v4 ServerContext::new 内联的 play::initialize 等价）
    let _ = &runtime;
    let mut connection_ctx =
        qexed_connection::connection::ServerContext::new(connection_config).await?;
    connection_ctx.set_play_launcher(runtime::play_launcher(
        runtime.world.clone(),
        runtime.players.clone(),
    ));
    let _ = &connection_ctx;

    let runtime = std::sync::Arc::new(runtime::QexedRuntime::new(
        server_config.clone(),
        "./world",
    )?);
    let handler = std::sync::Arc::new(runtime::QexedConnectionHandler {
        context: connection_ctx,
    });

    log::info!("{}", qexed_language::t("qexed.server.starting").replace("%{version}", shadow::PKG_VERSION));
    qexed_server::server::run(server_config, runtime, services, handler).await?;
    Ok(())
}
