use qexed_config::{
    app::{
        qexed::{Qexed, server::World},
        qexed_content_filter::QexedContentFilter,
        qexed_entity::QexedEntity,
        qexed_entity_rendering::QexedEntityRendering,
        qexed_lan_discovery::QexedLanDiscovery,
        qexed_lobby::QexedLobby,
        qexed_menus::QexedMenus,
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
        qexed.server.entities =
            QexedEntity::load_or_create_default(language.clone(), None, None)?.entities;
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
        Ok(Self { qexed, world })
    }
}

impl From<Qexed> for RuntimeConfig {
    fn from(mut qexed: Qexed) -> Self {
        let world = qexed.server.world.clone();
        qexed.server.world = world.clone();
        Self { qexed, world }
    }
}

impl std::ops::Deref for RuntimeConfig {
    type Target = Qexed;

    fn deref(&self) -> &Self::Target {
        &self.qexed
    }
}
