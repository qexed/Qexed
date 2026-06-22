use std::{collections::HashMap, time::Duration};

use anyhow::Result;
use qexed_packet::net_types::VarInt;
use qexed_protocol::to_client::play::update_mob_effect::{RemoveMobEffect, UpdateMobEffect};

use crate::play::survival::{DeathMessage, SurvivalDamage, SurvivalState};

#[derive(Debug, Default)]
pub(in crate::play) struct EffectRuntime {
    active: HashMap<String, ActiveEffect>,
}

#[derive(Debug, Clone)]
struct ActiveEffect {
    id: i32,
    amplifier: i32,
    duration_ticks: i32,
}

impl EffectRuntime {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(in crate::play) fn has_effect(&self, effect: &str) -> bool {
        self.active.contains_key(&normalize_effect(effect))
    }

    pub(in crate::play) fn amplifier(&self, effect: &str) -> Option<i32> {
        self.active
            .get(&normalize_effect(effect))
            .map(|effect| effect.amplifier)
    }

    pub(in crate::play) async fn add_effect<W>(
        &mut self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        entity_id: i32,
        effect: &str,
        amplifier: i32,
        duration_ticks: i32,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let Some(id) = effect_id(effect) else {
            return Ok(());
        };
        let key = normalize_effect(effect);
        let active = ActiveEffect {
            id,
            amplifier: amplifier.max(0),
            duration_ticks: duration_ticks.max(1),
        };
        send_update(sink, entity_id, &active).await?;
        self.active.insert(key, active);
        Ok(())
    }

    pub(in crate::play) async fn apply_instant_effect<W>(
        &mut self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        entity_id: i32,
        effect: &str,
        amplifier: i32,
        survival: &mut SurvivalState,
    ) -> Result<SurvivalDamage>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let amount = 4.0 * (amplifier.max(0) + 1) as f32;
        let outcome = match normalize_effect(effect).as_str() {
            "minecraft:instant_health" => {
                if survival.heal(amount) {
                    SurvivalDamage::Damaged
                } else {
                    SurvivalDamage::None
                }
            }
            "minecraft:instant_damage" => survival.apply_damage(amount, DeathMessage::Magic),
            _ => SurvivalDamage::None,
        };
        if outcome.changed() && outcome.death_message().is_none() {
            sink.send(survival.health_packet()).await?;
        }
        if let Some(id) = effect_id(effect) {
            sink.send(UpdateMobEffect {
                entity_id: VarInt(entity_id),
                effect_id: VarInt(id),
                amplifier: VarInt(amplifier.max(0)),
                duration: VarInt(1),
                flags: 0x02,
            })
            .await?;
        }
        Ok(outcome)
    }

    pub(in crate::play) async fn tick<W>(
        &mut self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        player: &crate::players::OnlinePlayer,
        plugins: &crate::plugins::PluginManager,
        config: &qexed_config::app::qexed::server::Gameplay,
        elapsed: Duration,
        survival: &mut SurvivalState,
    ) -> Result<SurvivalDamage>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if !config.potion_effects || self.active.is_empty() {
            return Ok(SurvivalDamage::None);
        }

        let tick_delta = (elapsed.as_millis() / 50).max(1).min(i32::MAX as u128) as i32;
        let mut removed = Vec::new();
        let mut outcome = SurvivalDamage::None;
        for (effect_name, effect) in self.active.iter_mut() {
            let response =
                plugins.apply_potion_effect_tick(crate::plugins::PotionEffectTickQuery {
                    player: qexed_plugin_api::player_payload_owned(player),
                    effect: effect_name.clone(),
                    amplifier: effect.amplifier,
                    duration_ticks: effect.duration_ticks,
                });
            if response.cancel {
                continue;
            }
            if let Some(duration) = response.duration_ticks {
                effect.duration_ticks = duration.max(0);
            }
            if let Some(amplifier) = response.amplifier {
                effect.amplifier = amplifier.max(0);
            }
            if effect.duration_ticks <= 0 {
                removed.push(effect_name.clone());
                continue;
            }
            effect.duration_ticks = effect.duration_ticks.saturating_sub(tick_delta);
            outcome = strongest(
                outcome,
                apply_periodic_effect(effect_name, effect, survival),
            );
            if effect.duration_ticks <= 0 {
                removed.push(effect_name.clone());
            }
        }

        for key in removed {
            if let Some(effect) = self.active.remove(&key) {
                sink.send(RemoveMobEffect {
                    entity_id: VarInt(player.entity_id),
                    effect_id: VarInt(effect.id),
                })
                .await?;
            }
        }
        if outcome.changed() && outcome.death_message().is_none() {
            sink.send(survival.health_packet()).await?;
        }
        Ok(outcome)
    }
}

fn apply_periodic_effect(
    effect_name: &str,
    effect: &ActiveEffect,
    survival: &mut SurvivalState,
) -> SurvivalDamage {
    let interval = match effect_name {
        "minecraft:regeneration" => Some((50 >> effect.amplifier).max(1)),
        "minecraft:poison" => Some((25 >> effect.amplifier).max(1)),
        "minecraft:wither" => Some((40 >> effect.amplifier).max(1)),
        "minecraft:saturation" => Some(1),
        _ => None,
    };
    let Some(interval) = interval else {
        return SurvivalDamage::None;
    };
    if effect.duration_ticks % interval != 0 {
        return SurvivalDamage::None;
    }
    match effect_name {
        "minecraft:regeneration" | "minecraft:saturation" => {
            if survival.heal((effect.amplifier + 1) as f32) {
                SurvivalDamage::Damaged
            } else {
                SurvivalDamage::None
            }
        }
        "minecraft:poison" => survival.apply_damage(1.0, DeathMessage::Magic),
        "minecraft:wither" => survival.apply_damage(1.0, DeathMessage::Wither),
        _ => SurvivalDamage::None,
    }
}

async fn send_update<W>(
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    entity_id: i32,
    effect: &ActiveEffect,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    sink.send(UpdateMobEffect {
        entity_id: VarInt(entity_id),
        effect_id: VarInt(effect.id),
        amplifier: VarInt(effect.amplifier),
        duration: VarInt(effect.duration_ticks),
        flags: 0x02,
    })
    .await?;
    Ok(())
}

fn strongest(current: SurvivalDamage, next: SurvivalDamage) -> SurvivalDamage {
    match (current, next) {
        (SurvivalDamage::Died(_), _) => current,
        (_, SurvivalDamage::Died(_)) => next,
        (SurvivalDamage::Damaged, _) => SurvivalDamage::Damaged,
        (_, SurvivalDamage::Damaged) => SurvivalDamage::Damaged,
        _ => SurvivalDamage::None,
    }
}

fn normalize_effect(effect: &str) -> String {
    let effect = effect.trim();
    if effect.contains(':') {
        effect.to_string()
    } else {
        format!("minecraft:{effect}")
    }
}

fn effect_id(effect: &str) -> Option<i32> {
    Some(match normalize_effect(effect).as_str() {
        "minecraft:speed" => 1,
        "minecraft:slowness" => 2,
        "minecraft:haste" => 3,
        "minecraft:mining_fatigue" => 4,
        "minecraft:strength" => 5,
        "minecraft:instant_health" => 6,
        "minecraft:instant_damage" => 7,
        "minecraft:jump_boost" => 8,
        "minecraft:nausea" => 9,
        "minecraft:regeneration" => 10,
        "minecraft:resistance" => 11,
        "minecraft:fire_resistance" => 12,
        "minecraft:water_breathing" => 13,
        "minecraft:invisibility" => 14,
        "minecraft:blindness" => 15,
        "minecraft:night_vision" => 16,
        "minecraft:hunger" => 17,
        "minecraft:weakness" => 18,
        "minecraft:poison" => 19,
        "minecraft:wither" => 20,
        "minecraft:health_boost" => 21,
        "minecraft:absorption" => 22,
        "minecraft:saturation" => 23,
        "minecraft:glowing" => 24,
        "minecraft:levitation" => 25,
        "minecraft:luck" => 26,
        "minecraft:unluck" => 27,
        "minecraft:slow_falling" => 28,
        "minecraft:conduit_power" => 29,
        "minecraft:dolphins_grace" => 30,
        "minecraft:bad_omen" => 31,
        "minecraft:hero_of_the_village" => 32,
        "minecraft:darkness" => 33,
        _ => return None,
    })
}
