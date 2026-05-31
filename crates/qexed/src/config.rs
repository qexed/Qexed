use qexed_config::{
    app::{
        qexed::{Qexed, server::World},
        qexed_content_filter::QexedContentFilter,
        qexed_entity::QexedEntity,
        qexed_entity_rendering::QexedEntityRendering,
        qexed_lan_discovery::QexedLanDiscovery,
        qexed_lobby::QexedLobby,
        qexed_menus::QexedMenus,
        qexed_npc::QexedNpc,
        qexed_permissions::QexedPermissions,
        qexed_placeholders::QexedPlaceholders,
        qexed_player_audit::QexedPlayerAudit,
        qexed_player_data::QexedPlayerData,
        qexed_player_messages::QexedPlayerMessages,
        qexed_plugin_download::QexedPluginDownload,
        qexed_resource_pack::QexedResourcePack,
        qexed_scoreboard::QexedScoreboard,
        qexed_server::QexedServer,
    },
    tool::AppConfigTrait,
};

#[derive(Debug)]
pub struct RuntimeConfig {
    pub qexed: Qexed,
    pub world: World,
    pub npcs: qexed_config::app::qexed::server::Npcs,
}

impl RuntimeConfig {
    pub fn load(language: Option<String>) -> anyhow::Result<Self> {
        let mut qexed = Qexed::load_or_create_default(language.clone(), None, None)?;
        qexed.plugin_download =
            QexedPluginDownload::load_or_create_default(language.clone(), None, None)?
                .plugin_download;
        QexedServer::load_or_create_default(language.clone(), None, None)?
            .apply_to(&mut qexed.server);
        qexed.server.lan_discovery =
            QexedLanDiscovery::load_or_create_default(language.clone(), None, None)?.lan_discovery;
        qexed.server.player_data =
            QexedPlayerData::load_or_create_default(language.clone(), None, None)?.player_data;
        qexed.server.player_messages =
            QexedPlayerMessages::load_or_create_default(language.clone(), None, None)?
                .player_messages;
        qexed.server.player_audit =
            QexedPlayerAudit::load_or_create_default(language.clone(), None, None)?.player_audit;
        qexed.server.content_filter =
            QexedContentFilter::load_or_create_default(language.clone(), None, None)?
                .content_filter;
        qexed.server.permissions =
            QexedPermissions::load_or_create_default(language.clone(), None, None)?.permissions;
        qexed.server.resource_pack =
            QexedResourcePack::load_or_create_default(language.clone(), None, None)?.resource_pack;
        qexed.server.entities = QexedEntity::load_or_create_default(language.clone(), None, None)?
            .entities
            .into();
        let npcs = QexedNpc::load_or_create_default(language.clone(), None, None)?.npcs;
        if npcs.enable {
            qexed.server.entities.enable = true;
            qexed
                .server
                .entities
                .list
                .retain(|entity| entity.kind != qexed_config::app::qexed::server::EntityKind::Npc);
            qexed
                .server
                .entities
                .list
                .extend(npcs.list.iter().cloned().map(npc_to_entity));
        }
        qexed.server.entity_rendering =
            QexedEntityRendering::load_or_create_default(language.clone(), None, None)?
                .entity_rendering;
        qexed.server.scoreboard =
            QexedScoreboard::load_or_create_default(language.clone(), None, None)?.scoreboard;
        qexed.server.menus =
            QexedMenus::load_or_create_default(language.clone(), None, None)?.menus;
        qexed.server.placeholders =
            QexedPlaceholders::load_or_create_default(language.clone(), None, None)?.placeholders;
        qexed.server.lobby =
            QexedLobby::load_or_create_default(language.clone(), None, None)?.lobby;
        let world = World::load_or_create_default(language, None, None)?;
        qexed.server.world = world.clone();
        Ok(Self { qexed, world, npcs })
    }
}

fn npc_to_entity(
    npc: qexed_config::app::qexed::server::Npc,
) -> qexed_config::app::qexed::server::Entity {
    qexed_config::app::qexed::server::Entity {
        id: npc.id,
        kind: qexed_config::app::qexed::server::EntityKind::Npc,
        entity_type: npc.entity_type,
        name: npc.name,
        display_name: npc.display_name,
        skin_textures: npc.skin_textures,
        skin_signature: npc.skin_signature,
        skin_player_id: npc.skin_player_id,
        x: npc.x,
        y: npc.y,
        z: npc.z,
        yaw: npc.yaw,
        pitch: npc.pitch,
        on_ground: npc.on_ground,
        look_at_players: npc.look_at_players,
        main_hand_event: npc.main_hand_event,
        off_hand_event: npc.off_hand_event,
        attack_event: npc.attack_event,
        ..qexed_config::app::qexed::server::Entity::default()
    }
}

impl From<Qexed> for RuntimeConfig {
    fn from(mut qexed: Qexed) -> Self {
        let world = qexed.server.world.clone();
        let npcs = qexed_config::app::qexed::server::Npcs::default();
        qexed.server.world = world.clone();
        Self { qexed, world, npcs }
    }
}

impl std::ops::Deref for RuntimeConfig {
    type Target = Qexed;

    fn deref(&self) -> &Self::Target {
        &self.qexed
    }
}
