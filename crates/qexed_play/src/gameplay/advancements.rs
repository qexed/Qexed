use std::collections::HashSet;

use crate::error::Result;

use crate::config::{CustomAdvancement, CustomAdvancementTrigger, GameplayConfig};
use qexed_packet::net_types::VarInt;
use qexed_protocol::to_client::play::update_advancements::{
    Advancement, AdvancementDisplay, AdvancementMapping, AdvancementProgress, Criteria,
    ProgressMapping, UpdateAdvancements,
};

#[derive(Debug, Default)]
pub(in crate::gameplay) struct AdvancementRuntime {
    definitions: Vec<(CustomAdvancementTrigger, CustomAdvancement)>,
    granted: HashSet<String>,
}

impl AdvancementRuntime {
    pub(super) fn new(config: &GameplayConfig) -> Self {
        let definitions = config
            .custom_advancements
            .iter()
            .filter(|advancement| !advancement.id.trim().is_empty())
            .map(|advancement| (advancement.trigger, advancement.clone()))
            .collect();
        Self {
            definitions,
            granted: HashSet::new(),
        }
    }

    pub(in crate::gameplay) async fn send_initial<W>(
        &mut self,
        sink: &mut qexed_connection::transport::PacketSink<W>,
        player: &qexed_player::OnlinePlayer,
        plugins: &qexed_plugins::PluginManager,
        config: &GameplayConfig,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if !config.advancements {
            return Ok(());
        }
        self.grant_triggers(
            sink,
            player,
            plugins,
            config,
            [CustomAdvancementTrigger::Join],
        )
        .await
    }

    pub(in crate::gameplay) async fn grant_triggers<W>(
        &mut self,
        sink: &mut qexed_connection::transport::PacketSink<W>,
        player: &qexed_player::OnlinePlayer,
        plugins: &qexed_plugins::PluginManager,
        config: &GameplayConfig,
        triggers: impl IntoIterator<Item = CustomAdvancementTrigger>,
    ) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        if !config.advancements {
            return Ok(());
        }
        let mut advancements = Vec::new();
        let mut progress = Vec::new();
        for trigger in triggers {
            for definition in
                self.definitions
                    .iter()
                    .filter_map(|(definition_trigger, definition)| {
                        (*definition_trigger == trigger).then_some(definition)
                    })
            {
                let id = normalized_advancement_id(definition);
                if !self.granted.insert(id.clone()) {
                    continue;
                }
                let response =
                    plugins.apply_advancement_grant(qexed_plugins::api::AdvancementGrantQuery {
                        player: crate::plugin_bridge::player_payload_owned(player),
                        id: id.clone(),
                        title: definition.title.clone(),
                        description: definition.description.clone(),
                    });
                if response.cancel {
                    self.granted.remove(&id);
                    continue;
                }
                advancements.push(AdvancementMapping {
                    key: id.clone(),
                    value: advancement(definition),
                });
                progress.push(ProgressMapping {
                    key: id,
                    value: AdvancementProgress {
                        criteria: vec![Criteria {
                            criterion_identifier: "done".to_string(),
                            date_of_achieving: Some(unix_millis()),
                        }],
                    },
                });
            }
        }
        if advancements.is_empty() && progress.is_empty() {
            return Ok(());
        }
        sink.send(UpdateAdvancements {
            reset_or_clear: false,
            advancement_mapping: advancements,
            identifiers: Vec::new(),
            progress_mapping: progress,
            show_advancements: true,
        })
        .await?;
        Ok(())
    }
}

fn advancement(definition: &CustomAdvancement) -> Advancement {
    Advancement {
        parentid: None,
        display_data: Some(AdvancementDisplay {
            title: text_nbt(&definition.title),
            description: text_nbt(&definition.description),
            icon: crate::inventory::item_id_for_name(&definition.icon)
                .map(|item| crate::inventory::simple_item(item, 1))
                .unwrap_or_else(|| crate::inventory::simple_item(1, 1)),
            frame_type: VarInt(0),
            flags: if definition.toast { 0x02 } else { 0 },
            background_texture: None,
            x_coord: 0.0,
            y_coord: 0.0,
        }),
        nested_requirements: vec![vec!["done".to_string()]],
        sends_telemetry_data: false,
    }
}

fn text_nbt(text: &str) -> qexed_nbt::Tag {
    qexed_nbt::Tag::String(text.to_string().into())
}

fn normalized_advancement_id(definition: &CustomAdvancement) -> String {
    let id = definition.id.trim();
    if id.contains(':') {
        id.to_string()
    } else {
        format!("qexed:{id}")
    }
}

fn unix_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}
