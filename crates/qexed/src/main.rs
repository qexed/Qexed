mod gameplay_hooks;
mod management_wiring;
mod items_registry;
mod play_boot;
mod plugin_services;
mod world_adapter;
mod world_rules_adapter;
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

    // 管理协议（MCSMP JSON-RPC over WebSocket）
    {
        let mgmt_config = qexed_management::config::ManagementConfig::load_persistent()
            .unwrap_or_else(|err| {
                log::warn!("management config load failed, using default: {err}");
                let mut fallback = qexed_management::config::ManagementConfig::persistent_default();
                fallback.ensure_secret();
                fallback
            });
        log::info!(
            "{}",
            qexed_language::t("qexed.management.secret_generated")
                .replace("%{secret}", &mgmt_config.secret)
        );
        let backend = std::sync::Arc::new(management_wiring::QexedManagementBackend {
            players: std::sync::Arc::new(qexed_player::PlayerManager::new(std::sync::Arc::new(
                qexed_protocol::types::EntityIdAllocator::default(),
            ))),
            warden: std::sync::Arc::new(qexed_server::warden::WardenManager::from_config(
                qexed_server::config::WardenConfig::default(),
            )),
            motd: std::sync::Arc::new(std::sync::Mutex::new("Qexed Server".to_string())),
            version_name: "26.3",
            protocol_version: qexed_config::PROTOCOL_VERSION,
        });
        let server = qexed_management::server::ManagementServer::new(mgmt_config, backend);
        tokio::spawn(async move {
            if let Err(err) = server.run().await {
                log::warn!("management server stopped: {err}");
            }
        });
    }

    qexed_profiler::init_global(std::sync::Arc::new(qexed_profiler::Profiler::new()));
    // 组装真实运行时并启动服务器主循环
    let server_config = qexed_server::config::ServerConfig::default();
    let services = std::sync::Arc::new(qexed_server::context::ServerServices::load());
    // 连接配置：默认正版验证（v4 语义）；connection.toml 的 online_mode=false 可关闭。
    let mut connection_config = qexed_connection::config::ConnectionConfig::default();
    // 尝试从 connection.toml 覆盖（含 online_mode）。
    use qexed_config::Config as _;
    if let Ok(loaded) = qexed_connection::config::ConnectionConfig::load_and_create_default(true) {
        connection_config = loaded;
    }
    // online_mode 由 connection.toml 驱动（默认 true = 正版验证；
    // session server 认证链已就绪：auth/authenticator.rs hasJoined + 皮肤
    // properties 透传）。本地离线开发在 connection.toml 设 online_mode=false。
    let runtime = std::sync::Arc::new(runtime::QexedRuntime::new(
        server_config.clone(),
        "./world",
    )?);
    // 组装层注入 play 启动器（v4 ServerContext::new 内联的 play::initialize 等价）
    let _ = &runtime;
    let mut connection_ctx =
        qexed_connection::connection::ServerContext::new(connection_config).await?;
    // warden 封禁检查注入
    {
        let warden = std::sync::Arc::new(qexed_server::warden::WardenManager::from_config(
            qexed_server::config::WardenConfig::default(),
        ));
        connection_ctx.set_ban_check(std::sync::Arc::new(move |uuid| {
            warden.ban_for(uuid).map(|record| record.reason)
        }));
    }
    connection_ctx.set_play_launcher(runtime::play_launcher(
        runtime.world.clone(),
        runtime.players.clone(),
    ));
    // 插件域初始化注入：v4 ensure_plugins_initialized（插件 init 事件 + 自定义实体
    // 注册 + NPC 落场）由连接域在首个玩家配置完成时触发一次。
    {
        let runtime_for_plugins = runtime.clone();
        runtime_for_plugins.install_plugin_services();
        connection_ctx.set_plugins_init(runtime_for_plugins.plugins_init_callback(
            &server_config.language.clone(),
        ));
    }
    let _ = &connection_ctx;

    let handler = std::sync::Arc::new(runtime::QexedConnectionHandler {
        context: connection_ctx,
    });

    // 出生点区块预热（后台执行，不阻塞监听）
    tokio::spawn(async move {
    {
        use qexed_config::Config as _;
        use qexed_world::world::WorldManager;
        let warm_world = WorldManager::with_generator(
            "./world",
            qexed_world::world::WorldLightMode::default(),
            qexed_world::world::WorldLightAlgorithm::default(),
            false,
            qexed_world::world::generator::from_config(
                &match qexed_world::config::WorldConfig::load_and_create_default(true) {
                Ok(config) => config,
                Err(err) => {
                    log::warn!("world config load failed, using flat default: {err}");
                    let mut fallback = qexed_world::config::WorldConfig::default();
                    fallback.generator = qexed_world::config::WorldGenerator::VanillaFlat;
                    fallback
                }
            },
            ),
        );
        let _session = warm_world.begin_session();
        let epoch = warm_world.cache_epoch();
        for dx in -1..=1i32 {
            for dz in -1..=1i32 {
                let t0 = std::time::Instant::now();
                match warm_world.generated_network_chunk_for_session("minecraft:overworld", dx, dz, epoch) {
                    Ok(_) => log::info!("spawn chunk ({dx},{dz}) warmed in {:?}", t0.elapsed()),
                    Err(e) => log::warn!("spawn chunk ({dx},{dz}) warmup failed: {e}"),
                }
            }
        }
    }
    });

    log::info!("{}", qexed_language::t("qexed.server.starting").replace("%{version}", shadow::PKG_VERSION));
    qexed_server::server::run(server_config, runtime, services, handler).await?;
    Ok(())
}
