
use crate::error::Result;
use crate::survival::{DeathMessage, SurvivalDamage, SurvivalState};

use super::items;

const MAX_AIR: i32 = 300;
const DROWN_DAMAGE_AIR: i32 = -20;

#[derive(Debug, Clone)]
pub(in crate::gameplay) struct OxygenRuntime {
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

    pub(in crate::gameplay) async fn tick<W>(
        &mut self,
        sink: &mut qexed_connection::transport::PacketSink<W>,
        player: &qexed_player::OnlinePlayer,
        plugins: &qexed_plugins::PluginManager,
        config: &crate::config::GameplayConfig,
        underwater: bool,
        inventory: &crate::inventory::PlayerInventory,
        effects: &super::effects::EffectRuntime,
        survival: &mut SurvivalState,
        game_mode: crate::config::GameMode,
    ) -> Result<SurvivalDamage>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if !config.oxygen || game_mode != crate::config::GameMode::Survival {
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

        let response = plugins.apply_player_oxygen(qexed_plugins::api::PlayerOxygenTickQuery {
            player: crate::plugin_bridge::player_payload_owned(player),
            dimension: player.dimension.clone(),
            position: crate::plugin_bridge::player_position_payload(player.position),
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
