//! 全局服务循环：50ms tick 驱动实体生成/AI/掉落物等。
//! 迁移自 v4 crates/qexed/src/services.rs。
//!
//! 适配差异（v6）：v4 的 tick 直接操作 context.entities/world/ore_pits/cluster_entities；
//! v6 收敛为 ServerRuntime::global_service_tick（组装层注入真实实现），
//! tick 调度/慢 tick 日志/shutdown 语义完整迁移。

use std::time::{Duration, Instant};

use tokio::{sync::watch, task::JoinHandle};

use crate::context::ServerRuntime;
use crate::error::{Result, ServerError};

const GLOBAL_SERVICE_TICK_INTERVAL: Duration = Duration::from_millis(50);
const SLOW_GLOBAL_SERVICE_TICK: Duration = Duration::from_millis(200);

pub fn spawn_global_services(
    runtime: std::sync::Arc<dyn ServerRuntime>,
    shutdown: watch::Receiver<bool>,
) -> JoinHandle<()> {
    tokio::spawn(run_global_services(runtime, shutdown))
}

async fn run_global_services(runtime: std::sync::Arc<dyn ServerRuntime>, mut shutdown: watch::Receiver<bool>) {
    let mut tick = tokio::time::interval(GLOBAL_SERVICE_TICK_INTERVAL);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    tick.tick().await;

    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
            _ = tick.tick() => {
                let runtime = runtime.clone();
                let started = Instant::now();
                let result = tokio::task::spawn_blocking(move || run_global_service_tick(&*runtime))
                    .await
                    .map_err(|err| ServerError::Join(err.to_string()))
                    .and_then(|inner| inner);
                match result {
                    Ok(()) => {
                        let elapsed = started.elapsed();
                        if elapsed >= SLOW_GLOBAL_SERVICE_TICK {
                            log::debug!("global service tick took {}ms", elapsed.as_millis());
                        }
                    }
                    Err(err) => {
                        log::warn!("global service tick failed: {err}");
                    }
                }
            }
        }
    }
}

fn run_global_service_tick(runtime: &dyn ServerRuntime) -> Result<()> {
    if let Some(cluster_entities) = runtime.cluster_entities() {
        // 集群模式：本地 tick 只处理集群路由之外的本地实体，
        // 完整生成/AI 交由分片（v4 里这里调用 cluster_entities.tick）。
        let players = runtime.player_snapshots();
        if !players.is_empty() {
            let rendering = runtime.cluster_rendering();
            let spawning = runtime.cluster_spawning();
            let default_dimension = runtime.default_dimension();
            let distance = runtime.simulation_distance();
            let result = cluster_entities.tick(
                runtime,
                &rendering,
                &spawning,
                &default_dimension,
                distance,
            );
            if let Err(err) = result {
                log::warn!("cluster entity tick failed: {err}");
            }
        }
    } else {
        // 本地模式：全部子系统 tick 由组装层实现。
        runtime.global_service_tick()?;
    }
    Ok(())
}
