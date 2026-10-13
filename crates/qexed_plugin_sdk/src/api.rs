//! 宿主服务薄包装：对应 v4 SDK 的自由函数（storage/economy/world/entity/…）。
//!
//! v4 里插件直接调 wasm import 自由函数；v6 服务都在 [`HostApi`] 的 trait 上，
//! 且多带 `plugin_id` 隔离参数。[`Sdk`] 收敛这两点：构造一次，方法名与 v4 对齐。

use qexed_plugin_api::HostApi;
use qexed_plugin_api::payload::{
    GeyserPlayerInfoQuery, GeyserPlayerInfoResponse, HttpRequest, HttpResponse, NpcPosition,
    PathfindingQuery, PathfindingResponse,
};
use qexed_plugin_api::services::{CurrencyInfo, RuntimeEntitySpec, WorldEditRegionSpec};
use qexed_plugin_api::PlayerInfo;

/// 宿主服务便捷包装。通过 [`crate::PluginContext::sdk`] 或直接构造。
pub struct Sdk<'a> {
    api: &'a dyn HostApi,
    plugin_id: &'static str,
}

impl<'a> Sdk<'a> {
    pub fn new(api: &'a dyn HostApi, plugin_id: &'static str) -> Self {
        Self { api, plugin_id }
    }

    // ── 结构化存储（v4 structured_storage_*）───────────────────────────

    pub fn storage_exists(&self, key: &str) -> bool {
        self.api.structured_storage_exists(self.plugin_id, key)
    }

    pub fn storage_get(&self, key: &str) -> Option<Vec<u8>> {
        self.api.structured_storage_get(self.plugin_id, key)
    }

    pub fn storage_set(&self, key: &str, value: &[u8]) -> bool {
        self.api.structured_storage_set(self.plugin_id, key, value)
    }

    pub fn storage_delete(&self, key: &str) -> bool {
        self.api.structured_storage_delete(self.plugin_id, key)
    }

    /// 读 JSON 值（v4 structured_storage_get_typed 的 serde_json 版）。
    pub fn storage_get_json<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        let raw = self.api.structured_storage_get(self.plugin_id, key)?;
        serde_json::from_slice(&raw).ok()
    }

    /// 写 JSON 值。
    pub fn storage_set_json<T: serde::Serialize>(&self, key: &str, value: &T) -> bool {
        let Ok(raw) = serde_json::to_vec(value) else { return false };
        self.api.structured_storage_set(self.plugin_id, key, &raw)
    }

    // ── 经济（v4 economy_*，currency 简写 "default" 之外的显式名）──────

    pub fn economy_register_currency(
        &self,
        id: &str,
        name: &str,
        symbol: &str,
        fractional_digits: i32,
    ) -> bool {
        self.api
            .economy_register_currency(self.plugin_id, id, name, symbol, fractional_digits)
    }

    pub fn economy_currency_info(&self, currency: &str) -> Option<CurrencyInfo> {
        self.api.economy_currency_info(currency)
    }

    pub fn economy_storage(&self, currency: &str) -> Option<String> {
        self.api.economy_storage(currency)
    }

    pub fn economy_balance(&self, player: &str, currency: &str) -> Option<i64> {
        self.api.economy_balance(player, currency)
    }

    pub fn economy_set_balance(&self, player: &str, currency: &str, amount: i64) -> Option<i64> {
        self.api.economy_set_balance(player, currency, amount)
    }

    pub fn economy_deposit(&self, player: &str, currency: &str, amount: i64) -> Option<i64> {
        self.api.economy_deposit(player, currency, amount)
    }

    pub fn economy_withdraw(&self, player: &str, currency: &str, amount: i64) -> Option<i64> {
        self.api.economy_withdraw(player, currency, amount)
    }

    // ── 世界编辑（v6 为视觉层：客户端方块更新广播）─────────────────────

    pub fn world_set_block(&self, dimension: &str, pos: (i32, i32, i32), block: &str) -> bool {
        self.api.world_set_block(dimension, pos, block)
    }

    pub fn world_set_blocks(&self, blocks: &[(&str, (i32, i32, i32), &str)]) -> usize {
        self.api.world_set_blocks(blocks)
    }

    pub fn world_break_block(&self, dimension: &str, pos: (i32, i32, i32)) -> bool {
        self.api.world_break_block(dimension, pos)
    }

    pub fn world_register_edit_region(&self, region: &WorldEditRegionSpec) -> bool {
        self.api.world_register_edit_region(region)
    }

    // ── 运行时实体 / NPC ───────────────────────────────────────────────

    pub fn entity_upsert(&self, entity: &RuntimeEntitySpec) -> bool {
        self.api.entity_upsert(entity)
    }

    pub fn entity_upsert_with_events(
        &self,
        entity: &RuntimeEntitySpec,
        main_hand_event: &str,
        off_hand_event: &str,
        attack_event: &str,
    ) -> bool {
        self.api
            .entity_upsert_with_events(entity, main_hand_event, off_hand_event, attack_event)
    }

    pub fn entity_move(&self, key: &str, dimension: &str, position: NpcPosition) -> bool {
        self.api.entity_move(key, dimension, position)
    }

    pub fn npc_patrol_step(
        &self,
        key: &str,
        dimension: &str,
        current: NpcPosition,
        target: NpcPosition,
        speed: f64,
    ) -> bool {
        self.api.npc_patrol_step(key, dimension, current, target, speed)
    }

    pub fn entity_remove(&self, key: &str) -> bool {
        self.api.entity_remove(key)
    }

    /// 按服务端实体 id 查 NPC 交互事件名（含实体规格与 key）。
    pub fn npc_event_by_entity_id(
        &self,
        entity_id: i32,
        interaction: qexed_plugin::host_api::action::NpcInteraction,
    ) -> Option<(String, RuntimeEntitySpec, String)> {
        qexed_plugin::action_hub().npc_event_by_entity_id(entity_id, interaction)
    }
    // ── 玩家动作（聊天/动作/传送/物品/弹射物…见 actions 模块）─────────

    /// 原始动作派发。返回实际执行的条数。
    pub fn perform(&self, player: &PlayerInfo, action: qexed_plugin_api::payload::PlayerAction) -> usize {
        self.api.apply_actions(&player.username, &[action])
    }

    /// 批量动作派发（按玩家分段下发，保持相对顺序）。
    pub fn perform_all<'b>(
        &self,
        actions: impl AsRef<[(&'b PlayerInfo, qexed_plugin_api::payload::PlayerAction)]>,
    ) -> usize {
        let mut count = 0;
        for (player, batch) in group_by_player(actions.as_ref()) {
            count += self.api.apply_actions(player, &batch);
        }
        count
    }

    // ── 随机池 / 占位符 / HTTP / 寻路 / 跨插件 ─────────────────────────

    pub fn random_pool_roll(
        &self,
        kind: &str,
        pool_id: &str,
        entries: &[(&str, u64)],
        precompute_count: usize,
    ) -> Option<String> {
        self.api.random_pool_roll(kind, pool_id, entries, precompute_count)
    }

    pub fn placeholder_register(
        &self,
        key: &str,
        resolver: std::sync::Arc<dyn qexed_plugin_api::services::PlaceholderResolver>,
    ) -> bool {
        self.api.placeholder_register(self.plugin_id, key, resolver)
    }

    pub fn placeholders_apply(&self, text: &str, player: Option<&PlayerInfo>) -> String {
        self.api.placeholders_apply(text, player)
    }

    pub fn http_request(&self, request: &HttpRequest) -> Option<HttpResponse> {
        self.api.http_request(request)
    }

    pub fn pathfinding_find(&self, query: &PathfindingQuery) -> Option<PathfindingResponse> {
        self.api.pathfinding_find(query)
    }

    pub fn plugin_service_exists(&self, service: &str) -> bool {
        self.api.plugin_service_exists(service)
    }

    pub fn plugin_call(&self, service: &str, method: &str, payload: &[u8]) -> Option<Vec<u8>> {
        self.api.plugin_call(service, method, payload)
    }

    pub fn plugin_register_service(
        &self,
        service: &str,
        provider: std::sync::Arc<dyn qexed_plugin_api::services::PluginServiceProvider>,
    ) -> bool {
        self.api.plugin_register_service(self.plugin_id, service, provider)
    }

    // ── 配置（v4 config_*，按 plugin_id 隔离的 TOML 文件）──────────────

    pub fn read_config(&self) -> Result<Option<String>, qexed_plugin_api::PluginError> {
        self.api.read_config_toml(self.plugin_id)
    }

    pub fn write_config(&self, toml: &str) -> Result<(), qexed_plugin_api::PluginError> {
        self.api.write_config_toml(self.plugin_id, toml)
    }

    /// 读配置，缺失时写入默认值并返回（v4 config_load_or_create）。
    pub fn config_load_or_create(&self, default_toml: &str) -> Result<String, qexed_plugin_api::PluginError> {
        match self.api.read_config_toml(self.plugin_id)? {
            Some(existing) => Ok(existing),
            None => {
                self.api.write_config_toml(self.plugin_id, default_toml)?;
                Ok(default_toml.to_string())
            }
        }
    }

    // ── 其他 ───────────────────────────────────────────────────────────

    pub fn geyser_player_info(&self, query: &GeyserPlayerInfoQuery) -> GeyserPlayerInfoResponse {
        self.api.geyser_player_info(query)
    }

    pub fn entity_position(&self, key: &str) -> Option<NpcPosition> {
        qexed_plugin::action_hub().entity_position(key).map(
            |(x, y, z, yaw, pitch)| NpcPosition { x, y, z, yaw, pitch },
        )
    }
}


/// 按玩家指针连续段分组（同段动作保持顺序）。
fn group_by_player<'a, 'b>(
    list: &'a [(&'b PlayerInfo, qexed_plugin_api::payload::PlayerAction)],
) -> Vec<(&'a str, Vec<qexed_plugin_api::payload::PlayerAction>)> {
    use qexed_plugin_api::payload::PlayerAction as Pa;
    let mut out: Vec<(&'a str, Vec<Pa>)> = Vec::new();
    for (player, action) in list {
        match out.last_mut() {
            Some((name, batch)) if *name == player.username.as_str() => batch.push(action.clone()),
            _ => out.push((&player.username, vec![action.clone()])),
        }
    }
    out
}
