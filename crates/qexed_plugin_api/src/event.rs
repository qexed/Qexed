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
    Commands,
    CommandExecute,
    NpcMutations,
    NpcInteract,
    ProxyConnectResult,
    Placeholders,
    PlayerBlockStep,
    PlayerMove,
    PlayerInput,
}

impl PluginEvent {
    pub fn export_name(self) -> &'static str {
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
            Self::Commands => "qexed_plugin_commands",
            Self::CommandExecute => "qexed_plugin_command_execute",
            Self::NpcMutations => "qexed_plugin_npc_mutations",
            Self::NpcInteract => "qexed_plugin_npc_interact",
            Self::ProxyConnectResult => "qexed_plugin_proxy_connect_result",
            Self::Placeholders => "qexed_plugin_placeholders",
            Self::PlayerBlockStep => "qexed_plugin_player_block_step",
            Self::PlayerMove => "qexed_plugin_player_move",
            Self::PlayerInput => "qexed_plugin_player_input",
        }
    }
}
