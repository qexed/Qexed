use std::{
    collections::HashMap,
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
    time::Instant,
};

use anyhow::{Context, Result};
use bytes::BytesMut;
use qexed_packet::{Packet, PacketReader, PacketWriter, net_types::VarInt};
use qexed_protocol::to_client::play::{
    damage_event::DamageEvent, entity_event::EntityEvent, hurt_animation::HurtAnimation,
    set_entity_motion::SetEntityMotion,
};

pub(crate) async fn run(
    config: crate::config::RuntimeConfig,
    shard_id: String,
    listen: String,
) -> Result<()> {
    let entity_ids = Arc::new(crate::entities::EntityIdAllocator::new(
        cluster_shard_entity_id_base(&shard_id),
    ));
    let generator = crate::world::generator::from_config(&config.world);
    let light_algorithm = crate::world::WorldLightAlgorithm::from(&config.world.light_algorithm);
    let world = crate::world::WorldManager::with_generator(
        config.world.path.clone(),
        crate::world::WorldLightMode::from(&config.world.light),
        light_algorithm,
        config.world.read_only,
        generator.clone(),
    )
    .with_worlds(&config.world.worlds)
    .with_instances(&config.world.instances)
    .with_edit_regions(&config.world.edit_regions)
    .with_precompiled_chunks(crate::world::PrecompiledChunkSettings::from(
        &config.world.precompiled_chunks,
    ));
    world.ensure_configured_storage(&config.world)?;
    let entities =
        crate::entities::EntityManager::from_config(&config.server.entities, entity_ids.clone())?;
    let state = Arc::new(ShardState {
        shard_id,
        generator,
        world: Arc::new(world),
        entities: Arc::new(entities),
        players: Arc::new(crate::players::PlayerManager::new(entity_ids)),
        virtual_receivers: Mutex::new(HashMap::new()),
        plugins: Arc::new(crate::plugins::PluginManager::load_default()),
        light_algorithm,
    });
    let listener = TcpListener::bind(&listen)
        .with_context(|| format!("failed to bind cluster shard listener: {listen}"))?;
    log::info!(
        "cluster shard listening: id={}, addr={}",
        state.shard_id,
        listen
    );
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let state = state.clone();
                thread::Builder::new()
                    .name(format!("qexed-cluster-shard-{}", state.shard_id))
                    .spawn(move || {
                        if let Err(err) = handle_connection(stream, &state) {
                            log::warn!(
                                "cluster shard request failed: id={}, error={err:#}",
                                state.shard_id
                            );
                        }
                    })?;
            }
            Err(err) => {
                log::warn!(
                    "cluster shard accept failed: id={}, error={err:#}",
                    state.shard_id
                );
            }
        }
    }
    Ok(())
}

struct ShardState {
    shard_id: String,
    generator: Arc<dyn crate::world::generator::WorldChunkGenerator>,
    world: Arc<crate::world::WorldManager>,
    entities: Arc<crate::entities::EntityManager>,
    players: Arc<crate::players::PlayerManager>,
    virtual_receivers: Mutex<
        HashMap<uuid::Uuid, tokio::sync::mpsc::UnboundedReceiver<crate::players::PlayerEvent>>,
    >,
    plugins: Arc<crate::plugins::PluginManager>,
    light_algorithm: crate::world::WorldLightAlgorithm,
}

fn handle_connection(mut stream: TcpStream, state: &ShardState) -> Result<()> {
    let request: crate::cluster_rpc::ClusterRequest = crate::cluster_rpc::read_frame(&mut stream)?;
    let started = Instant::now();
    let response = match request {
        crate::cluster_rpc::ClusterRequest::LoadChunk {
            dimension,
            chunk_x,
            chunk_z,
            light_algorithm,
        } => load_chunk(state, &dimension, chunk_x, chunk_z, light_algorithm),
        crate::cluster_rpc::ClusterRequest::BlockStateAt { dimension, x, y, z } => {
            Ok(block_state_at(state, &dimension, x, y, z))
        }
        crate::cluster_rpc::ClusterRequest::EntityView { players, rendering } => {
            entity_view(state, players, rendering)
        }
        crate::cluster_rpc::ClusterRequest::EntityTick {
            players,
            rendering,
            spawning,
            default_dimension,
            ai_tick_interval_ms,
        } => entity_tick(
            state,
            players,
            rendering,
            spawning,
            &default_dimension,
            ai_tick_interval_ms,
        ),
        crate::cluster_rpc::ClusterRequest::DamageEntity {
            players,
            rendering,
            target_entity_id,
            attacker_entity_id,
            attacker_x,
            attacker_y,
            attacker_z,
            damage,
            knockback,
        } => damage_entity(
            state,
            players,
            rendering,
            target_entity_id,
            attacker_entity_id,
            (attacker_x, attacker_y, attacker_z),
            damage,
            knockback,
        ),
        crate::cluster_rpc::ClusterRequest::DropItem {
            players,
            rendering,
            actor_profile_id,
            dimension,
            x,
            y,
            z,
            yaw,
            pitch,
            on_ground,
            item,
        } => drop_item(
            state,
            players,
            rendering,
            actor_profile_id,
            &dimension,
            qexed_protocol::to_client::play::add_entity::EntityPosition {
                x,
                y,
                z,
                yaw,
                pitch,
                on_ground,
            },
            item,
        ),
        crate::cluster_rpc::ClusterRequest::CollectItems {
            players,
            collector_profile_id,
            collector_entity_id,
            dimension,
            x,
            y,
            z,
            yaw,
            pitch,
            on_ground,
        } => collect_items(
            state,
            players,
            collector_profile_id,
            collector_entity_id,
            &dimension,
            qexed_protocol::to_client::play::add_entity::EntityPosition {
                x,
                y,
                z,
                yaw,
                pitch,
                on_ground,
            },
        ),
    };
    let elapsed = started.elapsed();
    if elapsed >= crate::cluster_rpc::SLOW_RESPONSE_THRESHOLD {
        log::debug!(
            "cluster shard slow request: id={}, elapsed_ms={:.2}",
            state.shard_id,
            elapsed.as_secs_f64() * 1000.0
        );
    }
    let response = response
        .unwrap_or_else(|err| crate::cluster_rpc::ClusterResponse::Error(format!("{err:#}")));
    crate::cluster_rpc::write_frame(&mut stream, &response)
}

fn load_chunk(
    state: &ShardState,
    dimension: &str,
    chunk_x: i32,
    chunk_z: i32,
    requested_light_algorithm: crate::cluster_rpc::ClusterLightAlgorithm,
) -> Result<crate::cluster_rpc::ClusterResponse> {
    let light_algorithm = crate::world::WorldLightAlgorithm::from(requested_light_algorithm);
    let light_algorithm = if light_algorithm == state.light_algorithm {
        state.light_algorithm
    } else {
        light_algorithm
    };
    let generated = state
        .generator
        .generate(dimension, chunk_x, chunk_z, light_algorithm)?;
    let mut map_chunk = BytesMut::new();
    generated
        .packet
        .serialize(&mut PacketWriter::new(&mut map_chunk))?;
    Ok(crate::cluster_rpc::ClusterResponse::Chunk {
        chunk_x: generated.packet.chunk_x,
        chunk_z: generated.packet.chunk_z,
        map_chunk: map_chunk.to_vec(),
        light_dampening: generated.light_dampening,
    })
}

fn block_state_at(
    state: &ShardState,
    dimension: &str,
    x: i32,
    y: i32,
    z: i32,
) -> crate::cluster_rpc::ClusterResponse {
    let position = qexed_packet::net_types::Position { x, y, z };
    crate::cluster_rpc::ClusterResponse::BlockState(
        state.generator.block_state_at(dimension, &position),
    )
}

fn entity_view(
    state: &ShardState,
    players: Vec<crate::cluster_rpc::ClusterPlayerSnapshot>,
    rendering: crate::cluster_rpc::ClusterEntityRendering,
) -> Result<crate::cluster_rpc::ClusterResponse> {
    sync_virtual_players(state, &players);
    let rendering = qexed_config::app::qexed::server::EntityRendering::from(rendering);
    let mut batches = Vec::new();
    for player in &players {
        let packets = state.entities.managed_entity_view_packets(
            &player.dimension,
            qexed_protocol::to_client::play::add_entity::EntityPosition {
                x: player.x,
                y: player.y,
                z: player.z,
                yaw: player.yaw,
                pitch: player.pitch,
                on_ground: player.on_ground,
            },
            &rendering,
        )?;
        if !packets.is_empty() {
            batches.push(crate::cluster_rpc::ClusterPacketBatch {
                profile_id: player.profile_id,
                packets: packets.into_iter().map(|packet| packet.to_vec()).collect(),
            });
        }
    }
    Ok(crate::cluster_rpc::ClusterResponse::EntityPackets(batches))
}

fn entity_tick(
    state: &ShardState,
    players: Vec<crate::cluster_rpc::ClusterPlayerSnapshot>,
    rendering: crate::cluster_rpc::ClusterEntityRendering,
    spawning: crate::cluster_rpc::ClusterEntitySpawning,
    default_dimension: &str,
    ai_tick_interval_ms: u64,
) -> Result<crate::cluster_rpc::ClusterResponse> {
    sync_virtual_players(state, &players);
    let rendering = qexed_config::app::qexed::server::EntityRendering::from(rendering);
    let mut spawning = qexed_config::app::qexed::server::EntitySpawning::from(spawning);
    spawning.ai_tick_interval_ms = ai_tick_interval_ms;
    let spawned = state.entities.spawn_from_rules(
        &state.players,
        &state.world,
        &rendering,
        &spawning,
        default_dimension,
    )?;
    if spawned > 0 {
        log::debug!(
            "cluster shard spawned entities: id={}, count={spawned}",
            state.shard_id
        );
    }
    state.entities.tick_ai(
        &state.players,
        &state.world,
        &state.plugins,
        &rendering,
        ai_tick_interval_ms,
    )?;
    Ok(crate::cluster_rpc::ClusterResponse::EntityPackets(
        drain_virtual_player_packets(state, &players),
    ))
}

fn damage_entity(
    state: &ShardState,
    players: Vec<crate::cluster_rpc::ClusterPlayerSnapshot>,
    rendering: crate::cluster_rpc::ClusterEntityRendering,
    target_entity_id: i32,
    attacker_entity_id: i32,
    attacker: (f64, f64, f64),
    damage: f32,
    knockback: f32,
) -> Result<crate::cluster_rpc::ClusterResponse> {
    sync_virtual_players(state, &players);
    let rendering = qexed_config::app::qexed::server::EntityRendering::from(rendering);
    let result = state.entities.damage_managed_entity(
        &state.players,
        &rendering,
        target_entity_id,
        damage,
    )?;
    if let Some(result) = result {
        let mut packets = vec![
            crate::players::packet_bytes(DamageEvent {
                entity_id: VarInt(target_entity_id),
                source_type_id: VarInt(0),
                source_cause_id: VarInt(attacker_entity_id),
                source_direct_id: VarInt(attacker_entity_id),
                has_source_position: false,
                source_position: None,
            })?,
            crate::players::packet_bytes(HurtAnimation {
                entity_id: VarInt(target_entity_id),
                yaw: result.entity.position.yaw,
            })?,
        ];
        if knockback > 0.0 {
            let dx = result.entity.position.x - attacker.0;
            let dz = result.entity.position.z - attacker.2;
            let length = (dx * dx + dz * dz).sqrt().max(0.0001);
            packets.push(crate::players::packet_bytes(
                SetEntityMotion::from_velocity(
                    target_entity_id,
                    dx / length * f64::from(knockback),
                    0.35,
                    dz / length * f64::from(knockback),
                ),
            )?);
        }
        if result.killed {
            packets.push(crate::players::packet_bytes(EntityEvent {
                entity_id: target_entity_id,
                event_id: 3,
            })?);
        }
        state.players.broadcast_packets(packets);
        return Ok(crate::cluster_rpc::ClusterResponse::EntityDamage {
            handled: true,
            killed: result.killed,
            packets: drain_virtual_player_packets(state, &players),
        });
    }
    Ok(crate::cluster_rpc::ClusterResponse::EntityDamage {
        handled: false,
        killed: false,
        packets: Vec::new(),
    })
}

fn drop_item(
    state: &ShardState,
    players: Vec<crate::cluster_rpc::ClusterPlayerSnapshot>,
    rendering: crate::cluster_rpc::ClusterEntityRendering,
    actor_profile_id: [u8; 16],
    dimension: &str,
    position: qexed_protocol::to_client::play::add_entity::EntityPosition,
    item: Vec<u8>,
) -> Result<crate::cluster_rpc::ClusterResponse> {
    sync_virtual_players(state, &players);
    let mut item_bytes = bytes::BytesMut::from(item.as_slice());
    let mut reader = PacketReader::new(&mut item_bytes);
    let item: qexed_protocol::types::Slot = reader.deserialize()?;
    let actor = uuid::Uuid::from_bytes(actor_profile_id);
    let rendering = qexed_config::app::qexed::server::EntityRendering::from(rendering);
    let updates = state.entities.drop_item_with_rendering(
        &state.players,
        actor,
        dimension,
        position,
        item,
        &rendering,
    )?;
    if updates.is_empty() {
        return Ok(crate::cluster_rpc::ClusterResponse::EntityPackets(
            Vec::new(),
        ));
    }
    Ok(crate::cluster_rpc::ClusterResponse::EntityPackets(
        drain_virtual_player_packets(state, &players),
    ))
}

fn collect_items(
    state: &ShardState,
    players: Vec<crate::cluster_rpc::ClusterPlayerSnapshot>,
    collector_profile_id: [u8; 16],
    collector_entity_id: i32,
    dimension: &str,
    position: qexed_protocol::to_client::play::add_entity::EntityPosition,
) -> Result<crate::cluster_rpc::ClusterResponse> {
    sync_virtual_players(state, &players);
    let collector = uuid::Uuid::from_bytes(collector_profile_id);
    let items = state
        .entities
        .collect_reachable_items(dimension, position)?;
    let mut collected = Vec::new();
    for item in items {
        let mut slot = bytes::BytesMut::new();
        qexed_packet::PacketWriter::new(&mut slot).serialize(&item.item)?;
        collected.push(crate::cluster_rpc::ClusterCollectedItem {
            item: slot.to_vec(),
            count: item.item.item_count.0.max(1),
        });
        let packets = item.pickup_packets(collector_entity_id)?;
        state
            .players
            .broadcast_packets_except(collector, packets.clone());
        state.players.send_packets_to(collector, packets);
    }
    Ok(crate::cluster_rpc::ClusterResponse::CollectedItems {
        items: collected,
        packets: drain_virtual_player_packets(state, &players),
    })
}

fn sync_virtual_players(state: &ShardState, players: &[crate::cluster_rpc::ClusterPlayerSnapshot]) {
    let active = players
        .iter()
        .map(|player| uuid::Uuid::from_bytes(player.profile_id))
        .collect::<std::collections::HashSet<_>>();
    for existing in state.players.list_except(uuid::Uuid::nil()) {
        if !active.contains(&existing.profile.uuid) {
            state.players.leave(existing.profile.uuid);
            state
                .virtual_receivers
                .lock()
                .expect("cluster shard virtual receivers poisoned")
                .remove(&existing.profile.uuid);
        }
    }
    for player in players {
        let profile_id = uuid::Uuid::from_bytes(player.profile_id);
        let position = qexed_protocol::to_client::play::add_entity::EntityPosition {
            x: player.x,
            y: player.y,
            z: player.z,
            yaw: player.yaw,
            pitch: player.pitch,
            on_ground: player.on_ground,
        };
        if state.players.player_by_uuid(profile_id).is_some() {
            state.players.update_position_and_dimension(
                profile_id,
                player.dimension.clone(),
                position,
            );
            continue;
        }
        let profile = qexed_packet::net_types::GameProfile {
            uuid: profile_id,
            username: player.username.clone(),
            properties: Vec::new(),
        };
        let session = state.players.join_with_entity_id(
            player.entity_id,
            profile,
            position,
            player.dimension.clone(),
            Vec::new(),
            "en_us".to_string(),
            crate::players::DEFAULT_DISPLAYED_SKIN_PARTS,
            0,
        );
        state
            .virtual_receivers
            .lock()
            .expect("cluster shard virtual receivers poisoned")
            .insert(profile_id, session.receiver);
    }
}

fn drain_virtual_player_packets(
    state: &ShardState,
    players: &[crate::cluster_rpc::ClusterPlayerSnapshot],
) -> Vec<crate::cluster_rpc::ClusterPacketBatch> {
    let mut batches = Vec::new();
    let mut receivers = state
        .virtual_receivers
        .lock()
        .expect("cluster shard virtual receivers poisoned");
    for player in players {
        let profile_id = uuid::Uuid::from_bytes(player.profile_id);
        let Some(receiver) = receivers.get_mut(&profile_id) else {
            continue;
        };
        let mut packets = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            if let crate::players::PlayerEvent::ClientboundPackets {
                packets: event_packets,
            } = event
            {
                packets.extend(event_packets.into_iter().map(|packet| packet.to_vec()));
            }
        }
        if !packets.is_empty() {
            batches.push(crate::cluster_rpc::ClusterPacketBatch {
                profile_id: player.profile_id,
                packets,
            });
        }
    }
    batches
}

fn cluster_shard_entity_id_base(shard_id: &str) -> i32 {
    const BASE: i32 = 100_000;
    const STRIDE: i32 = 10_000;
    const BUCKETS: i32 = 1_000;

    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in shard_id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    BASE + i32::try_from(hash % BUCKETS as u64).unwrap_or(0) * STRIDE
}
