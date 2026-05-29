use std::{
    fs,
    path::Path,
    sync::{Arc, Mutex, OnceLock},
};

use serde::{Serialize, de::DeserializeOwned};
use wasmtime::Engine;

mod event;
mod files;
mod host;
mod instance;
mod payload;

pub use event::PluginEvent;
use files::{PLUGIN_DIR, plugin_files};
use instance::PluginInstance;
pub use payload::{
    BlockDropPosition, BlockDropQuery, BlockDropResponse, BlockStepPayload, BlockStepPosition,
    ItemEnchantment, MiningSpeedQuery, MiningSpeedResponse, NpcEntityPayload, NpcInteractPayload,
    NpcMutationOp, NpcMutationQuery, NpcMutationResponse, PlaceholderContext, PlaceholderQuery,
    PlaceholderReplacement, PlaceholderResponse, PlayerAction, PlayerInputPayload,
    PlayerMovePayload, PlayerPayloadOwned, PluginCommandDefinition, PluginCommandQuery,
    PluginCommandResponse, PluginEnchantment, ProxyConnectResultPayload,
};
use payload::{
    ChunkPayload, ConfigReloadPayload, LanguagePayload, player_input_state, player_payload,
    player_payload_owned, player_position_payload,
};

use crate::players::OnlinePlayer;

pub struct PluginManager {
    plugins: OnceLock<Mutex<Vec<PluginInstance>>>,
    initialized: Mutex<bool>,
    path: std::path::PathBuf,
}

impl PluginManager {
    pub fn load_default() -> Self {
        Self::from_dir(PLUGIN_DIR)
    }

    pub fn from_dir(path: impl AsRef<Path>) -> Self {
        Self {
            plugins: OnceLock::new(),
            initialized: Mutex::new(false),
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn load_from_dir(path: impl AsRef<Path>) -> Self {
        let manager = Self::from_dir(path);
        manager.ensure_loaded();
        manager
    }

    pub fn ensure_initialized(&self, language: &str) -> bool {
        let mut initialized = self
            .initialized
            .lock()
            .expect("plugin manager initialization state poisoned");
        if *initialized {
            return false;
        }
        self.ensure_loaded();
        self.emit_init();
        self.emit_config_reload("config/qexed.toml");
        self.emit_language_change(language);
        *initialized = true;
        true
    }

    pub fn is_loaded(&self) -> bool {
        self.plugins.get().is_some()
    }

    pub fn plugin_count(&self) -> usize {
        self.plugins
            .get()
            .map(|plugins| plugins.lock().expect("plugin manager poisoned").len())
            .unwrap_or(0)
    }

    fn ensure_loaded(&self) -> &Mutex<Vec<PluginInstance>> {
        self.plugins
            .get_or_init(|| Mutex::new(load_plugins(&self.path)))
    }
}

fn load_plugins(path: &Path) -> Vec<PluginInstance> {
    if let Err(err) = fs::create_dir_all(path) {
        log::warn!(
            "plugin directory create failed: path={}, error={err}",
            path.display()
        );
        return Vec::new();
    }

    let engine = Engine::default();
    let services = Arc::new(host::PluginHostServices::default());
    let mut plugins = plugin_files(path)
        .into_iter()
        .filter_map(
            |file| match PluginInstance::load(&engine, file, services.clone()) {
                Ok(plugin) => Some(plugin),
                Err(err) => {
                    log::warn!("WASM plugin load failed: {err:#}");
                    None
                }
            },
        )
        .collect::<Vec<_>>();

    plugins.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.name.cmp(&right.name))
    });

    if !plugins.is_empty() {
        let summary = plugins
            .iter()
            .map(|plugin| format!("{}({})", plugin.name, plugin.priority))
            .collect::<Vec<_>>()
            .join(", ");
        log::info!("loaded WASM plugins: {summary}");
    }

    plugins
}

impl PluginManager {
    pub fn emit_init(&self) {
        self.emit_empty(PluginEvent::Init);
    }

    pub fn emit_player_join(&self, player: &OnlinePlayer) {
        self.emit_encoded(PluginEvent::PlayerJoin, &player_payload(player));
    }

    pub fn emit_player_leave(&self, player: &OnlinePlayer) {
        self.emit_encoded(PluginEvent::PlayerLeave, &player_payload(player));
    }

    pub fn emit_chunk_load(&self, dimension: &str, chunk_x: i32, chunk_z: i32) {
        self.emit_encoded(
            PluginEvent::ChunkLoad,
            &ChunkPayload {
                dimension: dimension.to_string(),
                chunk_x,
                chunk_z,
            },
        );
    }

    pub fn emit_chunk_unload(&self, dimension: &str, chunk_x: i32, chunk_z: i32) {
        self.emit_encoded(
            PluginEvent::ChunkUnload,
            &ChunkPayload {
                dimension: dimension.to_string(),
                chunk_x,
                chunk_z,
            },
        );
    }

    pub fn emit_config_reload(&self, path: &str) {
        self.emit_encoded(
            PluginEvent::ConfigReload,
            &ConfigReloadPayload {
                path: path.to_string(),
            },
        );
    }

    pub fn emit_language_change(&self, language: &str) {
        self.emit_encoded(
            PluginEvent::LanguageChange,
            &LanguagePayload {
                language: language.to_string(),
            },
        );
    }

    pub fn apply_mining_speed(&self, query: MiningSpeedQuery) -> f32 {
        let mut speed = query.speed;
        for response in
            self.query_encoded::<_, MiningSpeedResponse>(PluginEvent::MiningSpeed, &query)
        {
            if let Some(value) = response
                .speed
                .filter(|value| value.is_finite() && *value > 0.0)
            {
                speed = value;
            }
            if let Some(multiplier) = response
                .multiplier
                .filter(|value| value.is_finite() && *value > 0.0)
            {
                speed *= multiplier;
            }
            if let Some(add) = response.add.filter(|value| value.is_finite()) {
                speed += add;
            }
            speed = speed.max(0.01);
        }
        speed
    }

    pub fn apply_block_drops(&self, query: BlockDropQuery) -> Option<BlockDropResponse> {
        let mut result = None;
        for response in self.query_encoded::<_, BlockDropResponse>(PluginEvent::BlockDrops, &query)
        {
            let current = result.get_or_insert_with(|| BlockDropResponse {
                replace: false,
                items: Vec::new(),
            });
            if response.replace {
                current.replace = true;
                current.items.clear();
            }
            current.items.extend(response.items);
        }
        result
    }

    pub fn plugin_commands(&self) -> Vec<PluginCommandDefinition> {
        let mut commands =
            self.query_empty_encoded::<PluginCommandDefinition>(PluginEvent::Commands);
        commands.retain(|command| {
            let name = command.name.trim();
            !name.is_empty() && !name.chars().any(char::is_whitespace)
        });
        commands.sort_by(|left, right| left.name.cmp(&right.name));
        commands.dedup_by(|left, right| left.name == right.name);
        commands
    }

    pub fn execute_command(
        &self,
        player: &OnlinePlayer,
        command: &str,
        argument: &str,
    ) -> PluginCommandResponse {
        let query = PluginCommandQuery {
            command: command.to_string(),
            argument: argument.to_string(),
            player: player_payload_owned(player),
        };
        let mut result = PluginCommandResponse {
            handled: false,
            actions: Vec::new(),
        };
        for response in
            self.query_encoded::<_, PluginCommandResponse>(PluginEvent::CommandExecute, &query)
        {
            result.handled |= response.handled;
            result.actions.extend(response.actions);
        }
        result
    }

    pub fn query_npc_mutations(&self, reason: &str) -> Vec<NpcMutationOp> {
        let query = NpcMutationQuery {
            reason: reason.to_string(),
        };
        let mut operations = Vec::new();
        for response in
            self.query_encoded::<_, NpcMutationResponse>(PluginEvent::NpcMutations, &query)
        {
            operations.extend(response.operations);
        }
        operations
    }

    pub fn emit_proxy_connect_result(&self, payload: &ProxyConnectResultPayload) {
        self.emit_encoded(PluginEvent::ProxyConnectResult, payload);
    }

    pub fn placeholder_replacements(&self, query: PlaceholderQuery) -> Vec<PlaceholderReplacement> {
        let mut replacements = Vec::new();
        for response in
            self.query_encoded::<_, PlaceholderResponse>(PluginEvent::Placeholders, &query)
        {
            replacements.extend(response.replacements);
        }
        replacements
    }

    pub fn handle_player_block_step(
        &self,
        player: &OnlinePlayer,
        block_state: i32,
        block_name: String,
        position: BlockStepPosition,
    ) -> PluginCommandResponse {
        let query = BlockStepPayload {
            player: player_payload_owned(player),
            dimension: player.dimension.clone(),
            block_state,
            block_name,
            position,
            player_position: player_position_payload(player.position),
        };
        let mut result = PluginCommandResponse {
            handled: false,
            actions: Vec::new(),
        };
        for response in
            self.query_encoded::<_, PluginCommandResponse>(PluginEvent::PlayerBlockStep, &query)
        {
            result.handled |= response.handled;
            result.actions.extend(response.actions);
        }
        result
    }

    pub fn handle_player_move(
        &self,
        player: &OnlinePlayer,
        previous_position: qexed_protocol::to_client::play::add_entity::EntityPosition,
    ) -> PluginCommandResponse {
        let query = PlayerMovePayload {
            player: player_payload_owned(player),
            dimension: player.dimension.clone(),
            previous_position: player_position_payload(previous_position),
            position: player_position_payload(player.position),
        };
        let mut result = PluginCommandResponse {
            handled: false,
            actions: Vec::new(),
        };
        for response in
            self.query_encoded::<_, PluginCommandResponse>(PluginEvent::PlayerMove, &query)
        {
            result.handled |= response.handled;
            result.actions.extend(response.actions);
        }
        result
    }

    pub fn handle_player_input(
        &self,
        player: &OnlinePlayer,
        previous_flags: u8,
        flags: u8,
    ) -> PluginCommandResponse {
        let query = PlayerInputPayload {
            player: player_payload_owned(player),
            dimension: player.dimension.clone(),
            position: player_position_payload(player.position),
            previous_input: player_input_state(previous_flags),
            input: player_input_state(flags),
        };
        let mut result = PluginCommandResponse {
            handled: false,
            actions: Vec::new(),
        };
        for response in
            self.query_encoded::<_, PluginCommandResponse>(PluginEvent::PlayerInput, &query)
        {
            result.handled |= response.handled;
            result.actions.extend(response.actions);
        }
        result
    }

    pub fn handle_npc_interact(
        &self,
        player: &OnlinePlayer,
        entity: NpcEntityPayload,
        action: &str,
    ) -> PluginCommandResponse {
        let payload = match encode_plugin_payload(&NpcInteractPayload {
            player: player_payload_owned(player),
            entity,
            action: action.to_string(),
        }) {
            Ok(payload) => payload,
            Err(err) => {
                log::warn!("plugin NPC interact payload encode failed: error={err}");
                return PluginCommandResponse {
                    handled: false,
                    actions: Vec::new(),
                };
            }
        };

        let mut result = PluginCommandResponse {
            handled: false,
            actions: Vec::new(),
        };
        let mut plugins = self
            .ensure_loaded()
            .lock()
            .expect("plugin manager poisoned");
        for plugin in plugins.iter_mut() {
            let response = match plugin.call_event_or_query(PluginEvent::NpcInteract, &payload) {
                Ok(Some(response)) => response,
                Ok(None) => continue,
                Err(err) => {
                    log::warn!(
                        "WASM plugin NPC interact failed: plugin={}, error={err:#}",
                        plugin.name
                    );
                    continue;
                }
            };
            match decode_plugin_response::<PluginCommandResponse>(&response) {
                Ok(response) => {
                    result.handled |= response.handled;
                    result.actions.extend(response.actions);
                }
                Err(err) => log::warn!(
                    "WASM plugin NPC interact response decode failed: plugin={}, error={err}",
                    plugin.name
                ),
            }
        }
        result
    }

    #[cfg(test)]
    pub fn empty_for_tests() -> Self {
        Self::empty()
    }

    fn empty() -> Self {
        let plugins = OnceLock::new();
        let _ = plugins.set(Mutex::new(Vec::new()));
        Self {
            plugins,
            initialized: Mutex::new(true),
            path: std::path::PathBuf::new(),
        }
    }

    fn emit_empty(&self, event: PluginEvent) {
        self.emit(event, &[]);
    }

    fn emit_encoded<T: Serialize>(&self, event: PluginEvent, payload: &T) {
        let payload = match encode_plugin_payload(payload) {
            Ok(payload) => payload,
            Err(err) => {
                log::warn!("plugin event payload encode failed: event={event:?}, error={err}");
                return;
            }
        };
        self.emit(event, &payload);
    }

    fn emit(&self, event: PluginEvent, payload: &[u8]) {
        let mut plugins = self
            .ensure_loaded()
            .lock()
            .expect("plugin manager poisoned");
        for plugin in plugins.iter_mut() {
            if let Err(err) = plugin.call_event(event, payload) {
                log::warn!(
                    "WASM plugin event failed: plugin={}, event={event:?}, error={err:#}",
                    plugin.name
                );
            }
        }
    }

    fn query_encoded<T, R>(&self, event: PluginEvent, payload: &T) -> Vec<R>
    where
        T: Serialize,
        R: DeserializeOwned,
    {
        let payload = match encode_plugin_payload(payload) {
            Ok(payload) => payload,
            Err(err) => {
                log::warn!("plugin query payload encode failed: event={event:?}, error={err}");
                return Vec::new();
            }
        };
        let mut plugins = self
            .ensure_loaded()
            .lock()
            .expect("plugin manager poisoned");
        let mut responses = Vec::new();
        for plugin in plugins.iter_mut() {
            let response = match plugin.call_query(event, &payload) {
                Ok(Some(response)) => response,
                Ok(None) => continue,
                Err(err) => {
                    log::warn!(
                        "WASM plugin query failed: plugin={}, event={event:?}, error={err:#}",
                        plugin.name
                    );
                    continue;
                }
            };
            match decode_plugin_response(&response) {
                Ok(response) => responses.push(response),
                Err(err) => log::warn!(
                    "WASM plugin query response decode failed: plugin={}, event={event:?}, error={err}",
                    plugin.name
                ),
            }
        }
        responses
    }

    fn query_empty_encoded<R>(&self, event: PluginEvent) -> Vec<R>
    where
        R: DeserializeOwned,
    {
        self.query_encoded(event, &())
    }
}

fn encode_plugin_payload<T: Serialize>(payload: &T) -> Result<Vec<u8>, postcard::Error> {
    postcard::to_allocvec(payload)
}

fn decode_plugin_response<R: DeserializeOwned>(response: &[u8]) -> Result<R, postcard::Error> {
    postcard::from_bytes(response)
}

pub(super) struct PluginState {
    name: String,
    services: Arc<host::PluginHostServices>,
}

#[cfg(test)]
mod tests;
