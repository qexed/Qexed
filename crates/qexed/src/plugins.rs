use std::{fs, path::Path, sync::Mutex};

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
    BlockDropPosition, BlockDropQuery, BlockDropResponse, ItemEnchantment, MiningSpeedQuery,
    MiningSpeedResponse, PluginEnchantment,
};
use payload::{ChunkPayload, ConfigReloadPayload, LanguagePayload, player_payload};

use crate::players::OnlinePlayer;

pub struct PluginManager {
    plugins: Mutex<Vec<PluginInstance>>,
}

impl PluginManager {
    pub fn load_default() -> Self {
        Self::load_from_dir(PLUGIN_DIR)
    }

    pub fn load_from_dir(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        if let Err(err) = fs::create_dir_all(path) {
            log::warn!("插件目录创建失败: path={}, error={err}", path.display());
            return Self::empty();
        }

        let engine = Engine::default();
        let mut plugins = plugin_files(path)
            .into_iter()
            .filter_map(|file| match PluginInstance::load(&engine, file) {
                Ok(plugin) => Some(plugin),
                Err(err) => {
                    log::warn!("WASM 插件加载失败: {err:#}");
                    None
                }
            })
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
            log::info!("已加载 WASM 插件: {summary}");
        }

        Self {
            plugins: Mutex::new(plugins),
        }
    }

    pub fn emit_init(&self) {
        self.emit_empty(PluginEvent::Init);
    }

    pub fn emit_player_join(&self, player: &OnlinePlayer) {
        self.emit_json(PluginEvent::PlayerJoin, &player_payload(player));
    }

    pub fn emit_player_leave(&self, player: &OnlinePlayer) {
        self.emit_json(PluginEvent::PlayerLeave, &player_payload(player));
    }

    pub fn emit_chunk_load(&self, dimension: &str, chunk_x: i32, chunk_z: i32) {
        self.emit_json(
            PluginEvent::ChunkLoad,
            &ChunkPayload {
                dimension,
                chunk_x,
                chunk_z,
            },
        );
    }

    pub fn emit_chunk_unload(&self, dimension: &str, chunk_x: i32, chunk_z: i32) {
        self.emit_json(
            PluginEvent::ChunkUnload,
            &ChunkPayload {
                dimension,
                chunk_x,
                chunk_z,
            },
        );
    }

    pub fn emit_config_reload(&self, path: &str) {
        self.emit_json(PluginEvent::ConfigReload, &ConfigReloadPayload { path });
    }

    pub fn emit_language_change(&self, language: &str) {
        self.emit_json(PluginEvent::LanguageChange, &LanguagePayload { language });
    }

    pub fn apply_mining_speed(&self, query: MiningSpeedQuery) -> f32 {
        let mut speed = query.speed;
        for response in self.query_json::<_, MiningSpeedResponse>(PluginEvent::MiningSpeed, &query)
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
        for response in self.query_json::<_, BlockDropResponse>(PluginEvent::BlockDrops, &query) {
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

    #[cfg(test)]
    pub fn empty_for_tests() -> Self {
        Self::empty()
    }

    fn empty() -> Self {
        Self {
            plugins: Mutex::new(Vec::new()),
        }
    }

    fn emit_empty(&self, event: PluginEvent) {
        self.emit(event, &[]);
    }

    fn emit_json<T: Serialize>(&self, event: PluginEvent, payload: &T) {
        let payload = match serde_json::to_vec(payload) {
            Ok(payload) => payload,
            Err(err) => {
                log::warn!("插件事件序列化失败: event={event:?}, error={err}");
                return;
            }
        };
        self.emit(event, &payload);
    }

    fn emit(&self, event: PluginEvent, payload: &[u8]) {
        let mut plugins = self.plugins.lock().expect("plugin manager poisoned");
        for plugin in plugins.iter_mut() {
            if let Err(err) = plugin.call_event(event, payload) {
                log::warn!(
                    "WASM 插件事件执行失败: plugin={}, event={event:?}, error={err:#}",
                    plugin.name
                );
            }
        }
    }

    fn query_json<T, R>(&self, event: PluginEvent, payload: &T) -> Vec<R>
    where
        T: Serialize,
        R: DeserializeOwned,
    {
        let payload = match serde_json::to_vec(payload) {
            Ok(payload) => payload,
            Err(err) => {
                log::warn!("鎻掍欢鏌ヨ搴忓垪鍖栧け璐? event={event:?}, error={err}");
                return Vec::new();
            }
        };
        let mut plugins = self.plugins.lock().expect("plugin manager poisoned");
        let mut responses = Vec::new();
        for plugin in plugins.iter_mut() {
            let response = match plugin.call_query(event, &payload) {
                Ok(Some(response)) => response,
                Ok(None) => continue,
                Err(err) => {
                    log::warn!(
                        "WASM 鎻掍欢鏌ヨ鎵ц澶辫触: plugin={}, event={event:?}, error={err:#}",
                        plugin.name
                    );
                    continue;
                }
            };
            match serde_json::from_slice(&response) {
                Ok(response) => responses.push(response),
                Err(err) => log::warn!(
                    "WASM 鎻掍欢鏌ヨ response JSON 鏃犳晥: plugin={}, event={event:?}, error={err}",
                    plugin.name
                ),
            }
        }
        responses
    }
}

pub(super) struct PluginState {
    name: String,
}

#[cfg(test)]
mod tests;
