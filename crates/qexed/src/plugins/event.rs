#[derive(Debug, Clone, Copy)]
pub enum PluginEvent {
    Init,
    PlayerJoin,
    PlayerLeave,
    ChunkLoad,
    ChunkUnload,
    ConfigReload,
    LanguageChange,
    MiningSpeed,
    BlockDrops,
}

impl PluginEvent {
    pub(super) fn export_name(self) -> &'static str {
        match self {
            Self::Init => "qexed_plugin_init",
            Self::PlayerJoin => "qexed_plugin_player_join",
            Self::PlayerLeave => "qexed_plugin_player_leave",
            Self::ChunkLoad => "qexed_plugin_chunk_load",
            Self::ChunkUnload => "qexed_plugin_chunk_unload",
            Self::ConfigReload => "qexed_plugin_config_reload",
            Self::LanguageChange => "qexed_plugin_language_change",
            Self::MiningSpeed => "qexed_plugin_mining_speed",
            Self::BlockDrops => "qexed_plugin_block_drops",
        }
    }
}
