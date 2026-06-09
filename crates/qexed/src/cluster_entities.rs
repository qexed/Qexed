use std::{collections::HashMap, time::Instant};

use anyhow::Result;

#[derive(Debug, Clone)]
pub(crate) struct ClusterEntityController {
    router: crate::world::ClusterRouter,
    endpoints: HashMap<String, String>,
}

impl ClusterEntityController {
    pub(crate) fn from_config(
        config: &qexed_config::app::qexed::server::WorldCluster,
    ) -> Option<Self> {
        if !config.enable {
            return None;
        }
        let router = crate::world::ClusterRouter::from_config(config)?;
        let endpoints = config
            .shards
            .iter()
            .filter(|shard| shard.endpoint.trim().starts_with("tcp://"))
            .map(|shard| {
                (
                    shard.id.trim().to_string(),
                    shard.endpoint.trim().to_string(),
                )
            })
            .filter(|(id, _)| !id.is_empty())
            .collect::<HashMap<_, _>>();
        (!endpoints.is_empty()).then_some(Self { router, endpoints })
    }

    pub(crate) fn tick(
        &self,
        players: &crate::players::PlayerManager,
        rendering: &qexed_config::app::qexed::server::EntityRendering,
        spawning: &qexed_config::app::qexed::server::EntitySpawning,
        default_dimension: &str,
        simulation_distance: i32,
    ) -> Result<()> {
        let player_snapshots = self.player_snapshots(players);
        if player_snapshots.is_empty() {
            return Ok(());
        }
        let shards = self.shards_near_players(&player_snapshots, simulation_distance);
        if shards.is_empty() {
            return Ok(());
        }
        for shard_id in shards {
            let Some(endpoint) = self.endpoints.get(&shard_id) else {
                continue;
            };
            let request = crate::cluster_rpc::ClusterRequest::EntityTick {
                players: player_snapshots.clone(),
                rendering: rendering.into(),
                spawning: spawning.into(),
                default_dimension: default_dimension.to_string(),
                ai_tick_interval_ms: spawning.ai_tick_interval_ms,
            };
            match self.request(endpoint, &request) {
                Ok(crate::cluster_rpc::ClusterResponse::EntityPackets(batches)) => {
                    if !batches.is_empty() {
                        let packet_count = batches
                            .iter()
                            .map(|batch| batch.packets.len())
                            .sum::<usize>();
                        log::debug!(
                            "cluster entity packets: shard={shard_id}, batches={}, packets={packet_count}",
                            batches.len()
                        );
                    }
                    apply_packet_batches(players, batches);
                }
                Ok(crate::cluster_rpc::ClusterResponse::Error(message)) => {
                    log::warn!(
                        "cluster entity shard tick failed: shard={shard_id}, error={message}"
                    );
                }
                Ok(_) => {
                    log::warn!(
                        "cluster entity shard returned unexpected response: shard={shard_id}"
                    );
                }
                Err(err) => {
                    log::warn!(
                        "cluster entity shard tick unavailable: shard={shard_id}, error={err:#}"
                    );
                }
            }
        }
        Ok(())
    }

    pub(crate) fn spawn_view_for_player(
        &self,
        player: &crate::players::OnlinePlayer,
        rendering: &qexed_config::app::qexed::server::EntityRendering,
        simulation_distance: i32,
    ) -> Vec<bytes::Bytes> {
        let snapshot = player_snapshot(player);
        let shards = self.shards_near_players(std::slice::from_ref(&snapshot), simulation_distance);
        let mut packets = Vec::new();
        for shard_id in shards {
            let Some(endpoint) = self.endpoints.get(&shard_id) else {
                continue;
            };
            let request = crate::cluster_rpc::ClusterRequest::EntityView {
                players: vec![snapshot.clone()],
                rendering: rendering.into(),
            };
            match self.request(endpoint, &request) {
                Ok(crate::cluster_rpc::ClusterResponse::EntityPackets(batches)) => {
                    let packet_count = batches
                        .iter()
                        .map(|batch| batch.packets.len())
                        .sum::<usize>();
                    log::debug!(
                        "cluster entity view packets: shard={shard_id}, batches={}, packets={packet_count}",
                        batches.len()
                    );
                    packets.extend(
                        batches
                            .into_iter()
                            .flat_map(|batch| batch.packets)
                            .map(bytes::Bytes::from),
                    );
                }
                Ok(crate::cluster_rpc::ClusterResponse::Error(message)) => {
                    log::warn!("cluster entity view failed: shard={shard_id}, error={message}");
                }
                Ok(_) => {}
                Err(err) => {
                    log::warn!("cluster entity view unavailable: shard={shard_id}, error={err:#}");
                }
            }
        }
        packets
    }

    pub(crate) fn damage_entity(
        &self,
        players: &crate::players::PlayerManager,
        actor: &crate::players::OnlinePlayer,
        rendering: &qexed_config::app::qexed::server::EntityRendering,
        simulation_distance: i32,
        target_entity_id: i32,
        damage: f32,
    ) -> Result<Option<ClusterEntityDamageOutcome>> {
        let player_snapshots = self.player_snapshots(players);
        if player_snapshots.is_empty() {
            return Ok(None);
        }
        let actor_snapshot = player_snapshot(actor);
        let shards =
            self.shards_near_players(std::slice::from_ref(&actor_snapshot), simulation_distance);
        for shard_id in shards {
            let Some(endpoint) = self.endpoints.get(&shard_id) else {
                continue;
            };
            let request = crate::cluster_rpc::ClusterRequest::DamageEntity {
                players: player_snapshots.clone(),
                rendering: rendering.into(),
                target_entity_id,
                attacker_entity_id: actor.entity_id,
                attacker_x: actor.position.x,
                attacker_y: actor.position.y,
                attacker_z: actor.position.z,
                damage,
                knockback: 0.4,
            };
            match self.request(endpoint, &request)? {
                crate::cluster_rpc::ClusterResponse::EntityDamage {
                    handled,
                    killed,
                    packets,
                } => {
                    apply_packet_batches(players, packets);
                    if handled {
                        return Ok(Some(ClusterEntityDamageOutcome { killed }));
                    }
                }
                crate::cluster_rpc::ClusterResponse::Error(message) => {
                    log::warn!("cluster entity damage failed: shard={shard_id}, error={message}");
                }
                _ => {}
            }
        }
        Ok(None)
    }

    pub(crate) fn drop_item(
        &self,
        players: &crate::players::PlayerManager,
        actor: uuid::Uuid,
        dimension: &str,
        position: qexed_protocol::to_client::play::add_entity::EntityPosition,
        rendering: &qexed_config::app::qexed::server::EntityRendering,
        _simulation_distance: i32,
        item: &qexed_protocol::types::Slot,
    ) -> Result<bool> {
        let Some(endpoint) = self.endpoint_for_position(position) else {
            return Ok(false);
        };
        let mut item_bytes = bytes::BytesMut::new();
        qexed_packet::PacketWriter::new(&mut item_bytes).serialize(item)?;
        let player_snapshots = self.player_snapshots(players);
        let request = crate::cluster_rpc::ClusterRequest::DropItem {
            players: player_snapshots,
            rendering: rendering.into(),
            actor_profile_id: *actor.as_bytes(),
            dimension: dimension.to_string(),
            x: position.x,
            y: position.y,
            z: position.z,
            yaw: position.yaw,
            pitch: position.pitch,
            on_ground: position.on_ground,
            item: item_bytes.to_vec(),
        };
        match self.request(endpoint, &request)? {
            crate::cluster_rpc::ClusterResponse::EntityPackets(batches) => {
                apply_packet_batches(players, batches);
                Ok(true)
            }
            crate::cluster_rpc::ClusterResponse::Error(message) => {
                log::warn!("cluster item drop failed: error={message}");
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    pub(crate) fn collect_items(
        &self,
        players: &crate::players::PlayerManager,
        collector: &crate::players::OnlinePlayer,
        simulation_distance: i32,
    ) -> Result<Vec<qexed_protocol::types::Slot>> {
        let player_snapshots = self.player_snapshots(players);
        if player_snapshots.is_empty() {
            return Ok(Vec::new());
        }
        let collector_snapshot = player_snapshot(collector);
        let shards = self.shards_near_players(
            std::slice::from_ref(&collector_snapshot),
            simulation_distance,
        );
        let mut collected = Vec::new();
        for shard_id in shards {
            let Some(endpoint) = self.endpoints.get(&shard_id) else {
                continue;
            };
            let request = crate::cluster_rpc::ClusterRequest::CollectItems {
                players: player_snapshots.clone(),
                collector_profile_id: *collector.profile.uuid.as_bytes(),
                collector_entity_id: collector.entity_id,
                dimension: collector.dimension.clone(),
                x: collector.position.x,
                y: collector.position.y,
                z: collector.position.z,
                yaw: collector.position.yaw,
                pitch: collector.position.pitch,
                on_ground: collector.position.on_ground,
            };
            match self.request(endpoint, &request)? {
                crate::cluster_rpc::ClusterResponse::CollectedItems { items, packets } => {
                    for item in items {
                        let mut bytes = bytes::BytesMut::from(item.item.as_slice());
                        let mut reader = qexed_packet::PacketReader::new(&mut bytes);
                        let mut slot: qexed_protocol::types::Slot = reader.deserialize()?;
                        slot.item_count.0 = item.count.max(1);
                        collected.push(slot);
                    }
                    apply_packet_batches(players, packets);
                }
                crate::cluster_rpc::ClusterResponse::Error(message) => {
                    log::warn!("cluster item collect failed: shard={shard_id}, error={message}");
                }
                _ => {}
            }
        }
        Ok(collected)
    }

    fn request(
        &self,
        endpoint: &str,
        request: &crate::cluster_rpc::ClusterRequest,
    ) -> Result<crate::cluster_rpc::ClusterResponse> {
        let started = Instant::now();
        let response = crate::cluster_rpc::send_request(
            endpoint,
            request,
            crate::cluster_rpc::DEFAULT_CONNECT_TIMEOUT,
            crate::cluster_rpc::DEFAULT_REQUEST_TIMEOUT,
        )?;
        let elapsed = started.elapsed();
        if elapsed >= crate::cluster_rpc::SLOW_RESPONSE_THRESHOLD {
            log::debug!(
                "cluster entity shard slow response: endpoint={}, elapsed_ms={:.2}",
                endpoint,
                elapsed.as_secs_f64() * 1000.0
            );
        }
        Ok(response)
    }

    fn endpoint_for_position(
        &self,
        position: qexed_protocol::to_client::play::add_entity::EntityPosition,
    ) -> Option<&str> {
        let chunk_x = (position.x.floor() as i32).div_euclid(16);
        let chunk_z = (position.z.floor() as i32).div_euclid(16);
        let shard_id = self.router.route(chunk_x, chunk_z)?;
        self.endpoints.get(shard_id).map(String::as_str)
    }

    fn player_snapshots(
        &self,
        players: &crate::players::PlayerManager,
    ) -> Vec<crate::cluster_rpc::ClusterPlayerSnapshot> {
        players
            .list_except(uuid::Uuid::nil())
            .into_iter()
            .map(|player| player_snapshot(&player))
            .collect()
    }

    fn shards_near_players(
        &self,
        players: &[crate::cluster_rpc::ClusterPlayerSnapshot],
        simulation_distance: i32,
    ) -> Vec<String> {
        let radius = simulation_distance.max(1);
        let mut shards = std::collections::BTreeSet::new();
        for player in players {
            let center_x = (player.x.floor() as i32).div_euclid(16);
            let center_z = (player.z.floor() as i32).div_euclid(16);
            for chunk_x in center_x - radius..=center_x + radius {
                for chunk_z in center_z - radius..=center_z + radius {
                    if let Some(shard_id) = self.router.route(chunk_x, chunk_z)
                        && self.endpoints.contains_key(shard_id)
                    {
                        shards.insert(shard_id.to_string());
                    }
                }
            }
        }
        shards.into_iter().collect()
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ClusterEntityDamageOutcome {
    pub killed: bool,
}

fn player_snapshot(
    player: &crate::players::OnlinePlayer,
) -> crate::cluster_rpc::ClusterPlayerSnapshot {
    crate::cluster_rpc::ClusterPlayerSnapshot {
        profile_id: *player.profile.uuid.as_bytes(),
        entity_id: player.entity_id,
        username: player.profile.username.clone(),
        dimension: player.dimension.clone(),
        x: player.position.x,
        y: player.position.y,
        z: player.position.z,
        yaw: player.position.yaw,
        pitch: player.position.pitch,
        on_ground: player.position.on_ground,
    }
}

fn apply_packet_batches(
    players: &crate::players::PlayerManager,
    batches: Vec<crate::cluster_rpc::ClusterPacketBatch>,
) {
    for batch in batches {
        let profile_id = uuid::Uuid::from_bytes(batch.profile_id);
        let packets = batch
            .packets
            .into_iter()
            .map(bytes::Bytes::from)
            .collect::<Vec<_>>();
        if !packets.is_empty() {
            players.send_packets_to(profile_id, packets);
        }
    }
}
