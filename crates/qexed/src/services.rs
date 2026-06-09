use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use anyhow::Result;
use tokio::{sync::watch, task::JoinHandle};

use crate::connection::ServerContext;

const GLOBAL_SERVICE_TICK_INTERVAL: Duration = Duration::from_millis(50);
const SLOW_GLOBAL_SERVICE_TICK: Duration = Duration::from_millis(200);

pub fn spawn_global_services(
    context: ServerContext,
    shutdown: watch::Receiver<bool>,
) -> JoinHandle<()> {
    tokio::spawn(run_global_services(context, shutdown))
}

async fn run_global_services(context: ServerContext, mut shutdown: watch::Receiver<bool>) {
    initialize_plugins(&context).await;

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
                let context = context.clone();
                let started = Instant::now();
                match tokio::task::spawn_blocking(move || run_global_service_tick(&context)).await {
                    Ok(Ok(())) => {
                        let elapsed = started.elapsed();
                        if elapsed >= SLOW_GLOBAL_SERVICE_TICK {
                            log::debug!("global service tick took {}ms", elapsed.as_millis());
                        }
                    }
                    Ok(Err(err)) => {
                        log::warn!("global service tick failed: {err:#}");
                    }
                    Err(err) => {
                        log::warn!("global service tick task failed: {err:#}");
                    }
                }
            }
        }
    }
}

async fn initialize_plugins(context: &ServerContext) {
    let context = context.clone();
    match tokio::task::spawn_blocking(move || context.ensure_plugins_initialized()).await {
        Ok(()) => {}
        Err(err) => log::warn!("plugin initialization task failed: {err:#}"),
    }
}

fn run_global_service_tick(context: &ServerContext) -> Result<()> {
    let spawning = &context.config.server.entities.spawning;
    if let Some(cluster_entities) = &context.cluster_entities {
        cluster_entities.tick(
            &context.players,
            &context.config.server.entity_rendering,
            spawning,
            &context.config.world.default_play_dimension(),
            context.config.world.simulation_distance,
        )?;
    } else {
        context.entities.spawn_from_rules_with_entity_config(
            &context.players,
            &context.world,
            &context.config.server.entity_rendering,
            spawning,
            &context.config.world.default_play_dimension(),
            Some(&context.config.server.entities),
        )?;
        context.entities.tick_ai_with_world_rules(
            &context.players,
            &context.world,
            &context.plugins,
            Some(&context.world_rules),
            &context.config.server.entity_rendering,
            spawning.ai_tick_interval_ms,
        )?;
    }

    let now = Instant::now();
    evacuate_ore_pit_players(context, now);
    let ore_updates = context.ore_pits.tick(&context.world, now);
    broadcast_ore_pit_updates(context, ore_updates)?;
    Ok(())
}

fn broadcast_ore_pit_updates(
    context: &ServerContext,
    updates: Vec<crate::world::OrePitBlockUpdate>,
) -> Result<()> {
    if updates.is_empty() {
        return Ok(());
    }

    let mut light_chunks = HashSet::new();
    for update in updates {
        let chunk_x = update.position.x.div_euclid(16);
        let chunk_z = update.position.z.div_euclid(16);
        let light_update = if context.world.dynamic_light_enabled()
            && light_chunks.insert((update.dimension.clone(), chunk_x, chunk_z))
            && matches!(
                context.world_rules.snapshot(&update.dimension).light,
                qexed_config::app::qexed::server::LightMode::Dynamic
            ) {
            let packet = context
                .world
                .light_update(&update.dimension, chunk_x, chunk_z);
            Some(qexed_tcp_connect::PacketSink::<tokio::io::Sink>::build_send_packet(packet)?)
        } else {
            None
        };
        context.players.broadcast_block_changed(
            uuid::Uuid::nil(),
            &update.dimension,
            update.position,
            update.block_state,
            light_update,
        );
    }
    Ok(())
}

fn evacuate_ore_pit_players(context: &ServerContext, now: Instant) {
    for player in context.players.list_except(uuid::Uuid::nil()) {
        if let Some(target) = context.ore_pits.evacuation_target_for_player(
            &context.world,
            &player.dimension,
            player.position,
            now,
        ) {
            context
                .players
                .teleport_player(player.profile.uuid, player.dimension, target);
        }
    }
}
