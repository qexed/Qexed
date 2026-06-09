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
        qexed_proxy::QexedProxy,
        qexed_resource_pack::QexedResourcePack,
        qexed_scoreboard::QexedScoreboard,
        qexed_server::QexedServer,
        qexed_warden::QexedWarden,
    },
    tool::AppConfigTrait,
};

#[derive(Debug)]
pub struct RuntimeConfig {
    pub qexed: Qexed,
    pub world: World,
    pub npcs: qexed_config::app::qexed::server::Npcs,
    pub warden: QexedWarden,
}

impl RuntimeConfig {
    pub fn load(
        language: Option<String>,
        config_path: Option<std::path::PathBuf>,
    ) -> anyhow::Result<Self> {
        let mut qexed = Qexed::load_or_create_default(language.clone(), None, config_path.clone())?;
        qexed.plugin_download = QexedPluginDownload::load_or_create_default(
            language.clone(),
            None,
            config_path.clone(),
        )?
        .plugin_download;
        QexedServer::load_or_create_default(language.clone(), None, config_path.clone())?
            .apply_to(&mut qexed.server);
        QexedProxy::load_or_create_default(language.clone(), None, config_path.clone())?
            .apply_to(&mut qexed.server);
        qexed.server.lan_discovery =
            QexedLanDiscovery::load_or_create_default(language.clone(), None, config_path.clone())?
                .lan_discovery;
        qexed.server.player_data =
            QexedPlayerData::load_or_create_default(language.clone(), None, config_path.clone())?
                .player_data;
        qexed.server.player_messages = QexedPlayerMessages::load_or_create_default(
            language.clone(),
            None,
            config_path.clone(),
        )?
        .player_messages;
        qexed.server.player_audit =
            QexedPlayerAudit::load_or_create_default(language.clone(), None, config_path.clone())?
                .player_audit;
        qexed.server.content_filter = QexedContentFilter::load_or_create_default(
            language.clone(),
            None,
            config_path.clone(),
        )?
        .content_filter;
        qexed.server.permissions =
            QexedPermissions::load_or_create_default(language.clone(), None, config_path.clone())?
                .permissions;
        qexed.server.resource_pack =
            QexedResourcePack::load_or_create_default(language.clone(), None, config_path.clone())?
                .resource_pack;
        qexed.server.entities =
            QexedEntity::load_or_create_default(language.clone(), None, config_path.clone())?
                .entities
                .into();
        let npcs =
            QexedNpc::load_or_create_default(language.clone(), None, config_path.clone())?.npcs;
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
        qexed.server.entity_rendering = QexedEntityRendering::load_or_create_default(
            language.clone(),
            None,
            config_path.clone(),
        )?
        .entity_rendering;
        qexed.server.scoreboard =
            QexedScoreboard::load_or_create_default(language.clone(), None, config_path.clone())?
                .scoreboard;
        qexed.server.menus =
            QexedMenus::load_or_create_default(language.clone(), None, config_path.clone())?.menus;
        qexed.server.placeholders =
            QexedPlaceholders::load_or_create_default(language.clone(), None, config_path.clone())?
                .placeholders;
        qexed.server.lobby =
            QexedLobby::load_or_create_default(language.clone(), None, config_path.clone())?.lobby;
        let world = World::load_or_create_default(language, None, config_path.clone())?;
        let warden = QexedWarden::load_or_create_default(None, None, config_path)?;
        qexed.server.world = world.clone();
        Ok(Self {
            qexed,
            world,
            npcs,
            warden,
        })
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
        let warden = QexedWarden::default();
        qexed.server.world = world.clone();
        Self {
            qexed,
            world,
            npcs,
            warden,
        }
    }
}

impl std::ops::Deref for RuntimeConfig {
    type Target = Qexed;

    fn deref(&self) -> &Self::Target {
        &self.qexed
    }
}
