use anyhow::Result;

use crate::play::survival::{DeathMessage, SurvivalDamage, SurvivalState};

use super::items;

const MAX_AIR: i32 = 300;
const DROWN_DAMAGE_AIR: i32 = -20;

#[derive(Debug, Clone)]
pub(in crate::play) struct OxygenRuntime {
    air: i32,
    was_underwater: bool,
}

impl Default for OxygenRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl OxygenRuntime {
    pub(super) fn new() -> Self {
        Self {
            air: MAX_AIR,
            was_underwater: false,
        }
    }

    pub(super) fn air(&self) -> i32 {
        self.air
    }

    pub(in crate::play) async fn tick<W>(
        &mut self,
        sink: &mut qexed_tcp_connect::PacketSink<W>,
        player: &crate::players::OnlinePlayer,
        plugins: &crate::plugins::PluginManager,
        config: &qexed_config::app::qexed::server::Gameplay,
        underwater: bool,
        inventory: &crate::inventory::PlayerInventory,
        effects: &super::effects::EffectRuntime,
        survival: &mut SurvivalState,
        game_mode: qexed_config::app::qexed::server::GameMode,
    ) -> Result<SurvivalDamage>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if !config.oxygen || game_mode != qexed_config::app::qexed::server::GameMode::Survival {
            self.air = MAX_AIR;
            return Ok(SurvivalDamage::None);
        }

        let water_breathing = effects.has_effect("minecraft:water_breathing")
            || effects.has_effect("minecraft:conduit_power");
        let respiration = inventory
            .visible_equipment()
            .into_iter()
            .map(|equipment| items::enchantment_level(&equipment.item, "minecraft:respiration"))
            .max()
            .unwrap_or(0);
        if underwater && !water_breathing {
            if respiration > 0 {
                let chance = 1.0 / f64::from(respiration + 1);
                if rand::Rng::gen_bool(&mut rand::thread_rng(), chance) {
                    self.air -= 1;
                }
            } else {
                self.air -= 1;
            }
        } else {
            self.air = (self.air + 4).min(MAX_AIR);
        }

        let response = plugins.apply_player_oxygen(crate::plugins::PlayerOxygenTickQuery {
            player: qexed_plugin_api::player_payload_owned(player),
            dimension: player.dimension.clone(),
            position: qexed_plugin_api::player_position_payload(player.position),
            air: self.air,
            max_air: MAX_AIR,
            underwater,
        });
        if response.cancel {
            return Ok(SurvivalDamage::None);
        }
        if let Some(air) = response.air {
            self.air = air.clamp(DROWN_DAMAGE_AIR, MAX_AIR);
        }

        if underwater && !self.was_underwater {
            self.was_underwater = true;
        } else if !underwater {
            self.was_underwater = false;
        }

        if self.air > DROWN_DAMAGE_AIR {
            return Ok(SurvivalDamage::None);
        }
        self.air = 0;
        let damage = survival.apply_damage(2.0, DeathMessage::Drown);
        if damage.changed() && damage.death_message().is_none() {
            sink.send(survival.health_packet()).await?;
        }
        Ok(damage)
    }
}
