//! 插件管理器（v4 qexed/src/plugins.rs 迁移）。
//!
//! v6 差异：
//! - 加载从 wasmtime Engine/Module 改为 libloading Library（dll/so）。
//! - v4 依赖 qexed_player::OnlinePlayer / crate::entities::DroppedItemEntity /
//!   crate::inventory 的 emit_*/handle_* 入参改为 api 层 payload 类型。
//!   qexed_player::OnlinePlayer 已落地：emit_player_join_of / emit_player_leave_of
//!   提供强类型入口（PlayerPayload: From<&OnlinePlayer>），其余 payload 由
//!   qexed_server 装配层从 v6 类型构造。
//! - v4 crate::l10n 本地化经 host::LocalizeService 注入。

use crate::{api, host};

use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
    panic::{AssertUnwindSafe, catch_unwind},
    path::Path,
    sync::{Arc, Mutex, OnceLock, Weak},
};

use serde::{Serialize, de::DeserializeOwned};

pub use api::{
    AdvancementGrantQuery, AdvancementGrantResponse, BedrockFormResponsePayload, BlockDropPosition,
    BlockDropQuery, BlockDropResponse, BlockStepPayload, BlockStepPosition, ChunkPayload,
    ClickDetectedPayload, ConfigReloadPayload, CraftItemQuery, CraftItemResponse,
    CraftingRecipeQuery, CraftingRecipeResponse, CustomEntityDefinition,
    CustomEntityRegistryResponse, EnchantingOption, EnchantingQuery, EnchantingResponse,
    EntityAiEntityPayload, EntityAiOperation, EntityAiPlayerPayload, EntityAiTickQuery,
    EntityAiTickResponse, FurnaceRecipeQuery, FurnaceRecipeResponse, FurnaceTickPayload,
    ItemDurabilityQuery, ItemDurabilityResponse, ItemEnchantment, ItemStackPayload,
    MiningSpeedQuery, MiningSpeedResponse, NpcEntityPayload, NpcInteractPayload, NpcMutationOp,
    NpcMutationQuery, NpcMutationResponse, PlaceholderContext, PlaceholderQuery,
    PlaceholderReplacement, PlaceholderResponse, PlayerAction, PlayerAttackQuery,
    PlayerAttackResponse, PlayerBlockInteractPayload, PlayerClientPayload, PlayerDeathQuery,
    PlayerDeathResponse, PlayerInputPayload, PlayerInputState, PlayerItemPickupQuery,
    PlayerItemPickupResponse, PlayerMovePayload, PlayerOxygenTickQuery, PlayerOxygenTickResponse,
    PlayerPayload, PlayerPayloadOwned, PlayerPositionPayload, PlayerTickPayload,
    PlayerUseItemPayload, PluginCommandDefinition, PluginCommandQuery, PluginCommandResponse,
    PluginEnchantment, PotionEffectTickQuery, PotionEffectTickResponse,
    ProjectileHitPlayerPayload, ProxyConnectResultPayload, SoundPayload, SoundResponse,
};
pub use api::{LanguagePayload, PluginEvent};

use crate::files::{PLUGIN_DIR, plugin_files};
use crate::instance::PluginInstance;

/// v4 PlayerBlockHitPayload 迁移（方块交互的命中信息）。
#[derive(Debug, Clone, Default)]
pub struct PlayerBlockHitPayload {
    pub sequence: i32,
    pub face: String,
    pub face_id: i32,
    pub cursor_x: f32,
    pub cursor_y: f32,
    pub cursor_z: f32,
    pub inside_block: bool,
    pub world_border_hit: bool,
}

pub struct PluginManager {
    plugins: Arc<OnceLock<Vec<Arc<Mutex<PluginInstance>>>>>,
    supported_events: OnceLock<HashSet<PluginEvent>>,
    initialized: Mutex<bool>,
    path: std::path::PathBuf,
    services: Arc<host::PluginHostServices>,
}

impl PluginManager {
    pub fn load_default() -> Self {
        Self::from_dir(PLUGIN_DIR)
    }

    pub fn from_dir(path: impl AsRef<Path>) -> Self {
        let plugins = Arc::new(OnceLock::new());
        let services = Arc::new(host::PluginHostServices::default());
        services.set_plugin_api(Arc::new(PluginApiRouter {
            plugins: Arc::downgrade(&plugins),
        }));
        host::set_host_services(services.clone());
        Self {
            plugins,
            supported_events: OnceLock::new(),
            initialized: Mutex::new(false),
            path: path.as_ref().to_path_buf(),
            services,
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
        self.emit_config_reload("config/plugins.toml");
        self.emit_language_change(language);
        *initialized = true;
        true
    }

    pub fn is_loaded(&self) -> bool {
        self.plugins.get().is_some()
    }

    pub fn plugin_count(&self) -> usize {
        self.plugins.get().map(Vec::len).unwrap_or(0)
    }

    pub fn plugin_summaries(&self) -> Vec<String> {
        self.ensure_loaded()
            .iter()
            .map(|plugin| {
                let plugin = plugin.lock().expect("plugin manager poisoned");
                plugin.manifest.id.clone()
            })
            .collect()
    }

    fn ensure_loaded(&self) -> &Vec<Arc<Mutex<PluginInstance>>> {
        self.plugins.get_or_init(|| {
            let plugins = load_plugins(&self.path, self.services.clone());
            let supported_events = plugins
                .iter()
                .flat_map(|plugin| {
                    let plugin = plugin.lock().expect("plugin manager poisoned");
                    PluginEvent::ALL
                        .iter()
                        .copied()
                        .filter(|event| plugin.supports_event(*event))
                        .collect::<Vec<_>>()
                })
                .collect::<HashSet<_>>();
            let _ = self.supported_events.set(supported_events);
            plugins
        })
    }

    fn supports_event(&self, event: PluginEvent) -> bool {
        if self.supported_events.get().is_none() {
            self.ensure_loaded();
        }
        self.supported_events
            .get()
            .is_some_and(|events| events.contains(&event))
    }
}

fn load_plugins(
    path: &Path,
    services: Arc<host::PluginHostServices>,
) -> Vec<Arc<Mutex<PluginInstance>>> {
    if let Err(err) = fs::create_dir_all(path) {
        log::warn!(
            "{}",
            qexed_language::t("qexed.plugins.dir.create.failed")
                .replace("%{path}", &path.display().to_string())
                .replace("%{error}", &err.to_string())
        );
        return Vec::new();
    }

    let mut plugins = plugin_files(path)
        .into_iter()
        .filter_map(|file| match PluginInstance::load(file, services.clone()) {
            Ok(plugin) => Some(plugin),
            Err(err) => {
                log::warn!(
                    "{}",
                    qexed_language::t("qexed.plugins.load.failed")
                        .replace("%{error}", &err.to_string())
                );
                None
            }
        })
        .collect::<Vec<_>>();

    plugins = filter_plugins_with_dependencies(plugins);
    plugins = topological_sort_plugins(plugins);

    if !plugins.is_empty() {
        let summary = plugins
            .iter()
            .map(|plugin| format!("{}", plugin.manifest.id))
            .collect::<Vec<_>>()
            .join(" -> ");
        log::info!(
            "{}",
            qexed_language::t("qexed.plugins.load.order")
                .replace("%{plugins}", &summary)
        );
    }

    let plugin_ids = plugins
        .iter()
        .map(|plugin| plugin.manifest.id.clone())
        .collect::<Vec<_>>();
    let services_by_id = plugins
        .iter()
        .flat_map(|plugin| {
            plugin
                .manifest
                .services
                .iter()
                .map(|service| (service.id.clone(), plugin.manifest.id.clone()))
                .collect::<Vec<_>>()
        })
        .filter(|(service, _)| !service.trim().is_empty())
        .collect::<Vec<_>>();
    services.set_plugin_services(plugin_ids, services_by_id);

    plugins
        .into_iter()
        .map(|plugin| Arc::new(Mutex::new(plugin)))
        .collect()
}

/// 过滤硬依赖缺失的插件（级联移除，v4 同名函数迁移）。
fn filter_plugins_with_dependencies(mut plugins: Vec<PluginInstance>) -> Vec<PluginInstance> {
    let mut available: HashSet<String> = plugins.iter().map(|p| p.manifest.id.clone()).collect();

    loop {
        let before = plugins.len();
        plugins.retain(|plugin| {
            let missing = plugin
                .manifest
                .depends
                .iter()
                .find(|dep| !available.contains(dep.id.trim()));
            if let Some(dep) = missing {
                log::warn!(
                    "{}",
                    qexed_language::t("qexed.plugins.dependency.missing")
                        .replace("%{plugin}", &plugin.manifest.id)
                        .replace("%{dependency}", &dep.id)
                );
                return false;
            }
            true
        });
        let next_available: HashSet<String> =
            plugins.iter().map(|p| p.manifest.id.clone()).collect();
        if plugins.len() == before && next_available == available {
            break;
        }
        available = next_available;
    }
    plugins
}

/// 拓扑排序（v4 topological_sort_plugins 迁移，逻辑不变）。
fn topological_sort_plugins(plugins: Vec<PluginInstance>) -> Vec<PluginInstance> {
    if plugins.len() <= 1 {
        return plugins;
    }

    let id_to_idx: HashMap<String, usize> = plugins
        .iter()
        .enumerate()
        .map(|(i, p)| (p.manifest.id.clone(), i))
        .collect();
    let n = plugins.len();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut indegree: Vec<usize> = vec![0; n];

    for (i, plugin) in plugins.iter().enumerate() {
        let mut add_edges = |deps: &[api::PluginDependency]| {
            for dep in deps {
                if let Some(&from) = id_to_idx.get(dep.id.trim()) {
                    if from != i && !adj[from].contains(&i) {
                        adj[from].push(i);
                        indegree[i] = indegree[i].saturating_add(1);
                    }
                }
            }
        };
        add_edges(&plugin.manifest.depends);
        add_edges(&plugin.manifest.optional_depends);
        for after_id in &plugin.manifest.load_after {
            if let Some(&from) = id_to_idx.get(after_id.trim()) {
                if from != i && !adj[from].contains(&i) {
                    adj[from].push(i);
                    indegree[i] = indegree[i].saturating_add(1);
                }
            }
        }
    }

    let mut queue: VecDeque<usize> = indegree
        .iter()
        .enumerate()
        .filter(|(_, deg)| **deg == 0)
        .map(|(i, _)| i)
        .collect();

    queue.make_contiguous().sort_by(|&a, &b| {
        plugins[b]
            .priority
            .cmp(&plugins[a].priority)
            .then_with(|| plugins[a].manifest.id.cmp(&plugins[b].manifest.id))
    });

    let mut sorted = Vec::with_capacity(n);

    while let Some(u) = queue.pop_front() {
        sorted.push(u);
        let neighbors = std::mem::take(&mut adj[u]);
        for &v in &neighbors {
            indegree[v] = indegree[v].saturating_sub(1);
            if indegree[v] == 0 {
                let pos = queue
                    .iter()
                    .position(|&x| {
                        plugins[v].priority > plugins[x].priority
                            || (plugins[v].priority == plugins[x].priority
                                && plugins[v].manifest.id < plugins[x].manifest.id)
                    })
                    .unwrap_or(queue.len());
                queue.insert(pos, v);
            }
        }
    }

    if sorted.len() < n {
        let unsorted: Vec<&str> = (0..n)
            .filter(|i| !sorted.contains(i))
            .map(|i| plugins[i].manifest.id.as_str())
            .collect();
        log::warn!(
            "{}",
            qexed_language::t("qexed.plugins.cycle.detected")
                .replace("%{plugins}", &unsorted.join(", "))
        );
        let mut remaining: Vec<usize> = (0..n).filter(|i| !sorted.contains(i)).collect();
        remaining.sort_by(|&a, &b| {
            plugins[b]
                .priority
                .cmp(&plugins[a].priority)
                .then_with(|| plugins[a].manifest.id.cmp(&plugins[b].manifest.id))
        });
        sorted.extend(remaining);
    }

    let mut items: Vec<Option<PluginInstance>> = plugins.into_iter().map(Some).collect();
    sorted
        .into_iter()
        .map(|idx| {
            items[idx]
                .take()
                .expect("topological sort index is duplicate")
        })
        .collect()
}

#[derive(Debug)]
struct PluginApiRouter {
    plugins: Weak<OnceLock<Vec<Arc<Mutex<PluginInstance>>>>>,
}

impl host::PluginApiService for PluginApiRouter {
    fn call(&self, caller: &str, service: &str, method: &str, payload: &[u8]) -> Option<Vec<u8>> {
        let plugins = self.plugins.upgrade()?;
        let plugins = plugins.get()?;
        let query = api::PluginApiCallQuery {
            service: service.to_string(),
            method: method.to_string(),
            payload: payload.to_vec(),
            caller: caller.to_string(),
        };
        let encoded = serde_json::to_vec(&query).ok()?;

        for plugin in plugins {
            let Ok(mut plugin) = plugin.try_lock() else {
                continue;
            };
            if plugin.manifest.id == caller {
                continue;
            }
            let provides_service = plugin
                .manifest
                .services
                .iter()
                .any(|definition| definition.id == service);
            if !provides_service || !plugin.supports_event(PluginEvent::ApiCall) {
                continue;
            }
            let response = plugin.call_query(PluginEvent::ApiCall, &encoded).ok()??;
            let response: api::PluginApiCallResponse = serde_json::from_slice(&response).ok()?;
            if response.ok {
                return Some(response.payload);
            }
            if !response.error.trim().is_empty() {
                log::warn!(
                    "{}",
                    qexed_language::t("qexed.plugins.api.call.failed")
                        .replace("%{caller}", caller)
                        .replace("%{service}", service)
                        .replace("%{method}", method)
                        .replace("%{provider}", &plugin.manifest.id)
                        .replace("%{error}", &response.error)
                );
            }
            return None;
        }
        None
    }
}

impl PluginManager {
    pub fn emit_init(&self) {
        self.emit_empty(PluginEvent::Init);
    }

    pub fn emit_player_join(&self, player: &PlayerPayload) {
        self.emit_encoded(PluginEvent::PlayerJoin, player);
    }

    pub fn emit_player_leave(&self, player: &PlayerPayload) {
        self.emit_encoded(PluginEvent::PlayerLeave, player);
    }

    /// 强类型入口（TODO(player) 清偿）：直接接收 qexed_player::OnlinePlayer。
    pub fn emit_player_join_of(&self, player: &qexed_player::OnlinePlayer) {
        self.emit_player_join(&PlayerPayload::from(player));
    }

    /// 强类型入口（TODO(player) 清偿）：直接接收 qexed_player::OnlinePlayer。
    pub fn emit_player_leave_of(&self, player: &qexed_player::OnlinePlayer) {
        self.emit_player_leave(&PlayerPayload::from(player));
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
                break_positions: Vec::new(),
            });
            if response.replace {
                current.replace = true;
                current.items.clear();
            }
            current.items.extend(response.items);
            current.break_positions.extend(response.break_positions);
        }
        result
    }

    pub fn apply_crafting_recipe(
        &self,
        query: CraftingRecipeQuery,
    ) -> Option<CraftingRecipeResponse> {
        let mut result = None;
        for response in
            self.query_encoded::<_, CraftingRecipeResponse>(PluginEvent::CraftingRecipe, &query)
        {
            let current = result.get_or_insert_with(CraftingRecipeResponse::default);
            if response.replace {
                current.replace = true;
                current.result = response.result;
            } else if response.result.is_some() {
                current.result = response.result;
            }
        }
        result
    }

    pub fn handle_craft_item(&self, query: CraftItemQuery) -> CraftItemResponse {
        let mut result = CraftItemResponse::default();
        for response in self.query_encoded::<_, CraftItemResponse>(PluginEvent::CraftItem, &query) {
            result.cancel |= response.cancel;
            if response.result.is_some() {
                result.result = response.result;
            }
            result.actions.extend(response.actions);
        }
        result
    }

    pub fn apply_enchanting_options(&self, query: EnchantingQuery) -> Option<EnchantingResponse> {
        let mut result = None;
        for response in
            self.query_encoded::<_, EnchantingResponse>(PluginEvent::EnchantingOptions, &query)
        {
            let current = result.get_or_insert_with(EnchantingResponse::default);
            if response.replace {
                current.replace = true;
                current.options.clear();
            }
            current.options.extend(response.options);
        }
        result
    }

    pub fn apply_furnace_recipe(&self, query: FurnaceRecipeQuery) -> Option<FurnaceRecipeResponse> {
        let mut result = None;
        for response in
            self.query_encoded::<_, FurnaceRecipeResponse>(PluginEvent::FurnaceRecipe, &query)
        {
            let current = result.get_or_insert_with(FurnaceRecipeResponse::default);
            if response.replace {
                current.replace = true;
                current.result = response.result;
            } else if response.result.is_some() {
                current.result = response.result;
            }
            if response.cook_time.is_some() {
                current.cook_time = response.cook_time;
            }
            if response.experience.is_some() {
                current.experience = response.experience;
            }
        }
        result
    }

    pub fn emit_furnace_tick(&self, payload: &FurnaceTickPayload) {
        self.emit_encoded(PluginEvent::FurnaceTick, payload);
    }

    pub fn apply_item_durability(&self, query: ItemDurabilityQuery) -> ItemDurabilityResponse {
        let mut result = ItemDurabilityResponse::default();
        for response in
            self.query_encoded::<_, ItemDurabilityResponse>(PluginEvent::ItemDurability, &query)
        {
            result.cancel |= response.cancel;
            if response.amount.is_some() {
                result.amount = response.amount;
            }
        }
        result
    }

    pub fn apply_player_attack(&self, query: PlayerAttackQuery) -> PlayerAttackResponse {
        let mut result = PlayerAttackResponse::default();
        for response in
            self.query_encoded::<_, PlayerAttackResponse>(PluginEvent::PlayerAttack, &query)
        {
            result.cancel |= response.cancel;
            if response.damage.is_some() {
                result.damage = response.damage;
            }
            if response.knockback.is_some() {
                result.knockback = response.knockback;
            }
            if response.fire_ticks.is_some() {
                result.fire_ticks = response.fire_ticks;
            }
            result.actions.extend(response.actions);
        }
        result
    }

    pub fn apply_player_oxygen(&self, query: PlayerOxygenTickQuery) -> PlayerOxygenTickResponse {
        let mut result = PlayerOxygenTickResponse::default();
        for response in
            self.query_encoded::<_, PlayerOxygenTickResponse>(PluginEvent::PlayerOxygenTick, &query)
        {
            result.cancel |= response.cancel;
            if response.air.is_some() {
                result.air = response.air;
            }
            result.actions.extend(response.actions);
        }
        result
    }

    pub fn apply_sound(&self, payload: SoundPayload) -> SoundResponse {
        let mut result = SoundResponse::default();
        for response in self.query_encoded::<_, SoundResponse>(PluginEvent::Sound, &payload) {
            result.cancel |= response.cancel;
            if response.sound.is_some() {
                result.sound = response.sound;
            }
            if response.volume.is_some() {
                result.volume = response.volume;
            }
            if response.pitch.is_some() {
                result.pitch = response.pitch;
            }
        }
        result
    }

    pub fn apply_advancement_grant(
        &self,
        query: AdvancementGrantQuery,
    ) -> AdvancementGrantResponse {
        let mut result = AdvancementGrantResponse::default();
        for response in
            self.query_encoded::<_, AdvancementGrantResponse>(PluginEvent::AdvancementGrant, &query)
        {
            result.cancel |= response.cancel;
        }
        result
    }

    pub fn apply_potion_effect_tick(
        &self,
        query: PotionEffectTickQuery,
    ) -> PotionEffectTickResponse {
        let mut result = PotionEffectTickResponse::default();
        for response in
            self.query_encoded::<_, PotionEffectTickResponse>(PluginEvent::PotionEffectTick, &query)
        {
            result.cancel |= response.cancel;
            if response.duration_ticks.is_some() {
                result.duration_ticks = response.duration_ticks;
            }
            if response.amplifier.is_some() {
                result.amplifier = response.amplifier;
            }
            result.actions.extend(response.actions);
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
        player: &PlayerPayloadOwned,
        command: &str,
        argument: &str,
    ) -> PluginCommandResponse {
        let query = PluginCommandQuery {
            command: command.to_string(),
            argument: argument.to_string(),
            player: player.clone(),
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

    pub fn custom_entities(&self) -> Vec<CustomEntityDefinition> {
        let mut definitions = Vec::new();
        for response in
            self.query_empty_encoded::<CustomEntityRegistryResponse>(PluginEvent::CustomEntities)
        {
            definitions.extend(response.entities);
        }
        definitions.retain(|definition| !definition.id.trim().is_empty());
        definitions.sort_by(|left, right| left.id.cmp(&right.id));
        definitions.dedup_by(|left, right| left.id == right.id);
        definitions
    }

    pub fn handle_entity_ai_tick(&self, query: EntityAiTickQuery) -> Vec<EntityAiOperation> {
        let mut operations = Vec::new();
        for response in
            self.query_encoded::<_, EntityAiTickResponse>(PluginEvent::EntityAiTick, &query)
        {
            operations.extend(response.operations);
        }
        operations
    }

    pub fn emit_proxy_connect_result(&self, payload: &ProxyConnectResultPayload) {
        self.emit_encoded(PluginEvent::ProxyConnectResult, payload);
    }

    pub fn emit_bedrock_form_response(&self, payload: &BedrockFormResponsePayload) {
        self.emit_encoded(PluginEvent::BedrockFormResponse, payload);
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
        player: &PlayerPayloadOwned,
        dimension: &str,
        player_position: PlayerPositionPayload,
        block_state: i32,
        block_name: String,
        position: BlockStepPosition,
    ) -> PluginCommandResponse {
        let query = BlockStepPayload {
            player: player.clone(),
            dimension: dimension.to_string(),
            block_state,
            block_name,
            position,
            player_position,
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

    pub fn handle_player_block_interact(
        &self,
        player: &PlayerPayloadOwned,
        dimension: &str,
        player_position: PlayerPositionPayload,
        block_state: i32,
        block_name: String,
        position: BlockDropPosition,
        hand: String,
        hit: PlayerBlockHitPayload,
        input: PlayerInputState,
        client: PlayerClientPayload,
    ) -> PluginCommandResponse {
        let query = PlayerBlockInteractPayload {
            player: player.clone(),
            dimension: dimension.to_string(),
            block_state,
            block_name,
            position,
            player_position,
            hand,
            sequence: hit.sequence,
            face: hit.face,
            face_id: hit.face_id,
            cursor_x: hit.cursor_x,
            cursor_y: hit.cursor_y,
            cursor_z: hit.cursor_z,
            inside_block: hit.inside_block,
            world_border_hit: hit.world_border_hit,
            input,
            client,
        };
        let mut result = PluginCommandResponse {
            handled: false,
            actions: Vec::new(),
        };
        for response in
            self.query_encoded::<_, PluginCommandResponse>(PluginEvent::PlayerBlockInteract, &query)
        {
            result.handled |= response.handled;
            result.actions.extend(response.actions);
        }
        result
    }

    pub fn handle_player_move(
        &self,
        player: &PlayerPayloadOwned,
        dimension: &str,
        previous_position: PlayerPositionPayload,
        position: PlayerPositionPayload,
    ) -> PluginCommandResponse {
        let query = PlayerMovePayload {
            player: player.clone(),
            dimension: dimension.to_string(),
            previous_position,
            position,
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

    pub fn handle_player_tick(
        &self,
        player: &PlayerPayloadOwned,
        dimension: &str,
        position: PlayerPositionPayload,
        tick_millis: u64,
    ) -> PluginCommandResponse {
        let query = PlayerTickPayload {
            player: player.clone(),
            dimension: dimension.to_string(),
            position,
            tick_millis,
        };
        let mut result = PluginCommandResponse {
            handled: false,
            actions: Vec::new(),
        };
        for response in
            self.query_encoded::<_, PluginCommandResponse>(PluginEvent::PlayerTick, &query)
        {
            result.handled |= response.handled;
            result.actions.extend(response.actions);
        }
        result
    }

    pub fn handle_player_input(
        &self,
        player: &PlayerPayloadOwned,
        dimension: &str,
        position: PlayerPositionPayload,
        previous_flags: u8,
        flags: u8,
        client: PlayerClientPayload,
    ) -> PluginCommandResponse {
        let query = PlayerInputPayload {
            player: player.clone(),
            dimension: dimension.to_string(),
            position,
            previous_input: api::player_input_state(previous_flags),
            input: api::player_input_state(flags),
            client,
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

    pub fn handle_player_use_item(
        &self,
        player: &PlayerPayloadOwned,
        dimension: &str,
        position: PlayerPositionPayload,
        hand: String,
        action: String,
        item: ItemStackPayload,
        sequence: i32,
        yaw: f32,
        pitch: f32,
        input: PlayerInputState,
        client: PlayerClientPayload,
    ) -> PluginCommandResponse {
        let query = PlayerUseItemPayload {
            player: player.clone(),
            dimension: dimension.to_string(),
            position,
            hand,
            action,
            item,
            sequence,
            yaw,
            pitch,
            input,
            client,
        };
        let mut result = PluginCommandResponse {
            handled: false,
            actions: Vec::new(),
        };
        for response in
            self.query_encoded::<_, PluginCommandResponse>(PluginEvent::PlayerUseItem, &query)
        {
            result.handled |= response.handled;
            result.actions.extend(response.actions);
        }
        result
    }

    pub fn handle_projectile_hit_player(
        &self,
        query: ProjectileHitPlayerPayload,
    ) -> PluginCommandResponse {
        let mut result = PluginCommandResponse {
            handled: false,
            actions: Vec::new(),
        };
        for response in
            self.query_encoded::<_, PluginCommandResponse>(PluginEvent::ProjectileHitPlayer, &query)
        {
            result.handled |= response.handled;
            result.actions.extend(response.actions);
        }
        result
    }

    pub fn emit_click_detected(
        &self,
        player: &PlayerPayloadOwned,
        dimension: &str,
        position: PlayerPositionPayload,
        action: String,
        clicks: u32,
        window_ms: u64,
    ) {
        self.emit_encoded(
            PluginEvent::ClickDetected,
            &ClickDetectedPayload {
                player: player.clone(),
                dimension: dimension.to_string(),
                position,
                action,
                clicks,
                window_ms,
            },
        );
    }

    pub fn handle_player_item_pickup(&self, query: PlayerItemPickupQuery) -> PlayerItemPickupResponse {
        let mut result = PlayerItemPickupResponse {
            cancel: false,
            consume: false,
            actions: Vec::new(),
        };
        for response in
            self.query_encoded::<_, PlayerItemPickupResponse>(PluginEvent::PlayerItemPickup, &query)
        {
            result.cancel |= response.cancel;
            result.consume |= response.consume;
            result.actions.extend(response.actions);
        }
        result
    }

    pub fn handle_player_death(
        &self,
        player: &PlayerPayloadOwned,
        dimension: &str,
        position: PlayerPositionPayload,
        cause: String,
        source_entity_id: i32,
    ) -> PlayerDeathResponse {
        let query = PlayerDeathQuery {
            player: player.clone(),
            dimension: dimension.to_string(),
            position,
            cause,
            source_entity_id,
        };
        let mut result = PlayerDeathResponse {
            cancel: false,
            message: String::new(),
            overlay: false,
        };
        for response in
            self.query_encoded::<_, PlayerDeathResponse>(PluginEvent::PlayerDeath, &query)
        {
            result.cancel |= response.cancel;
            if !response.message.trim().is_empty() {
                result.message = response.message;
                result.overlay = response.overlay;
            }
        }
        result
    }

    pub fn set_pathfinding_service(&self, service: Arc<dyn host::PathfindingService>) {
        self.services.set_pathfinding(service);
    }

    pub fn set_world_edit_service(&self, service: Arc<dyn host::WorldEditService>) {
        self.services.set_world_edit(service);
    }

    pub fn set_entity_control_service(&self, service: Arc<dyn host::EntityControlService>) {
        self.services.set_entity_control(service);
    }

    pub fn set_localize_service(&self, service: Arc<dyn host::LocalizeService>) {
        self.services.set_localizer(service);
    }

    pub fn upsert_geyser_player_info(&self, info: api::GeyserPlayerInfoResponse) {
        self.services.upsert_geyser_player(info);
    }

    pub fn remove_geyser_player_info(&self, uuid: &str, username: &str) {
        self.services.remove_geyser_player(uuid, username);
    }

    pub fn configure_economy(&self, config: &crate::config::EconomyConfig) {
        self.services.configure_economy(config);
    }

    pub fn configure_structured_storage(&self, config: &crate::config::StructuredStorageConfig) {
        self.services.configure_structured_storage(config);
    }

    pub fn handle_npc_interact(
        &self,
        player: &PlayerPayloadOwned,
        entity: NpcEntityPayload,
        action: &str,
        hand: &str,
        configured_event: &str,
    ) -> PluginCommandResponse {
        if !self.supports_event(PluginEvent::NpcInteract) {
            return PluginCommandResponse {
                handled: false,
                actions: Vec::new(),
            };
        }
        let payload = match serde_json::to_vec(&NpcInteractPayload {
            player: player.clone(),
            entity,
            action: action.to_string(),
            hand: hand.to_string(),
            configured_event: configured_event.to_string(),
        }) {
            Ok(payload) => payload,
            Err(err) => {
                log::warn!(
                    "{}",
                    qexed_language::t("qexed.plugins.payload.encode.failed")
                        .replace("%{error}", &err.to_string())
                );
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
        for plugin in self.ensure_loaded() {
            let Ok(mut plugin) = plugin.lock() else {
                log::warn!("{}", qexed_language::t("qexed.plugins.instance.poisoned"));
                continue;
            };
            if !plugin.supports_event(PluginEvent::NpcInteract) {
                continue;
            }
            let plugin_name = plugin.name.clone();
            let response = match catch_unwind(AssertUnwindSafe(|| {
                plugin.call_event_or_query(PluginEvent::NpcInteract, &payload)
            })) {
                Err(_) => {
                    log::warn!(
                        "{}",
                        qexed_language::t("qexed.plugins.event.panicked")
                            .replace("%{plugin}", &plugin_name)
                    );
                    continue;
                }
                Ok(Ok(Some(response))) => response,
                Ok(Ok(None)) => continue,
                Ok(Err(err)) => {
                    log::warn!(
                        "{}",
                        qexed_language::t("qexed.plugins.event.failed")
                            .replace("%{plugin}", &plugin_name)
                            .replace("%{error}", &err.to_string())
                    );
                    continue;
                }
            };
            match serde_json::from_slice::<PluginCommandResponse>(&response) {
                Ok(response) => {
                    result.handled |= response.handled;
                    result.actions.extend(response.actions);
                }
                Err(err) => log::warn!(
                    "{}",
                    qexed_language::t("qexed.plugins.response.decode.failed")
                        .replace("%{plugin}", &plugin_name)
                        .replace("%{error}", &err.to_string())
                ),
            }
        }
        result
    }

    /// 测试辅助：空管理器（跨 crate 测试可见）。
    pub fn empty_for_tests() -> Self {
        Self::empty()
    }

    fn empty() -> Self {
        let plugins = Arc::new(OnceLock::new());
        let _ = plugins.set(Vec::new());
        let services = Arc::new(host::PluginHostServices::default());
        services.set_plugin_api(Arc::new(PluginApiRouter {
            plugins: Arc::downgrade(&plugins),
        }));
        host::set_host_services(services.clone());
        let supported_events = OnceLock::new();
        let _ = supported_events.set(HashSet::new());
        Self {
            plugins,
            supported_events,
            initialized: Mutex::new(true),
            path: std::path::PathBuf::new(),
            services,
        }
    }

    fn emit_empty(&self, event: PluginEvent) {
        self.emit(event, &[]);
    }

    fn emit_encoded<T: Serialize>(&self, event: PluginEvent, payload: &T) {
        let payload = match serde_json::to_vec(payload) {
            Ok(payload) => payload,
            Err(err) => {
                log::warn!(
                    "{}",
                    qexed_language::t("qexed.plugins.payload.encode.failed")
                        .replace("%{error}", &err.to_string())
                );
                return;
            }
        };
        self.emit(event, &payload);
    }

    fn emit(&self, event: PluginEvent, payload: &[u8]) {
        if !self.supports_event(event) {
            return;
        }
        for plugin in self.ensure_loaded() {
            let Ok(mut plugin) = plugin.lock() else {
                log::warn!("{}", qexed_language::t("qexed.plugins.instance.poisoned"));
                continue;
            };
            if !plugin.supports_event(event) {
                continue;
            }
            let plugin_name = plugin.name.clone();
            match catch_unwind(AssertUnwindSafe(|| plugin.call_event(event, payload))) {
                Ok(Ok(())) => {}
                Ok(Err(err)) => {
                    log::warn!(
                        "{}",
                        qexed_language::t("qexed.plugins.event.failed")
                            .replace("%{plugin}", &plugin_name)
                            .replace("%{error}", &err.to_string())
                    );
                }
                Err(_) => {
                    log::warn!(
                        "{}",
                        qexed_language::t("qexed.plugins.event.panicked")
                            .replace("%{plugin}", &plugin_name)
                    );
                }
            }
        }
    }

    fn query_encoded<T, R>(&self, event: PluginEvent, payload: &T) -> Vec<R>
    where
        T: Serialize,
        R: DeserializeOwned,
    {
        if !self.supports_event(event) {
            return Vec::new();
        }
        let payload = match serde_json::to_vec(payload) {
            Ok(payload) => payload,
            Err(err) => {
                log::warn!(
                    "{}",
                    qexed_language::t("qexed.plugins.payload.encode.failed")
                        .replace("%{error}", &err.to_string())
                );
                return Vec::new();
            }
        };
        let mut responses = Vec::new();
        for plugin in self.ensure_loaded() {
            let Ok(mut plugin) = plugin.lock() else {
                log::warn!("{}", qexed_language::t("qexed.plugins.instance.poisoned"));
                continue;
            };
            if !plugin.supports_event(event) {
                continue;
            }
            let plugin_name = plugin.name.clone();
            let response = match catch_unwind(AssertUnwindSafe(|| {
                plugin.call_query(event, &payload)
            })) {
                Err(_) => {
                    log::warn!(
                        "{}",
                        qexed_language::t("qexed.plugins.event.panicked")
                            .replace("%{plugin}", &plugin_name)
                    );
                    continue;
                }
                Ok(Ok(Some(response))) => response,
                Ok(Ok(None)) => continue,
                Ok(Err(err)) => {
                    log::warn!(
                        "{}",
                        qexed_language::t("qexed.plugins.event.failed")
                            .replace("%{plugin}", &plugin_name)
                            .replace("%{error}", &err.to_string())
                    );
                    continue;
                }
            };
            match serde_json::from_slice(&response) {
                Ok(response) => responses.push(response),
                Err(err) => log::warn!(
                    "{}",
                    qexed_language::t("qexed.plugins.response.decode.failed")
                        .replace("%{plugin}", &plugin_name)
                        .replace("%{error}", &err.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn online_player_converts_to_player_payload() {
        use qexed_packet::net_types::GameProfile;
        let player = qexed_player::OnlinePlayer {
            profile: GameProfile {
                uuid: uuid::Uuid::new_v4(),
                username: "Steve".to_string(),
                properties: Vec::new(),
            },
            entity_id: 7,
            game_mode: 0,
            position: qexed_protocol::types::EntityPosition {
                x: 0.0,
                y: 64.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                on_ground: true,
            },
            dimension: "minecraft:overworld".to_string(),
            equipment: Vec::new(),
            language: "en_us".to_string(),
            displayed_skin_parts: 0x7f,
        };

        let payload = crate::api::PlayerPayload::from(&player);
        assert_eq!(payload.uuid, player.profile.uuid.to_string());
        assert_eq!(payload.username, "Steve");
        assert_eq!(payload.entity_id, 7);
        assert_eq!(payload.language, "en_us");
        assert_eq!(payload.dimension, "minecraft:overworld");

        // 强类型入口在无插件时不 panic（空事件循环）。
        let manager = PluginManager::empty_for_tests();
        manager.emit_player_join_of(&player);
        manager.emit_player_leave_of(&player);
    }

    #[test]
    fn empty_plugin_manager_keeps_mining_speed_unchanged() {
        let manager = PluginManager::empty_for_tests();
        let speed = manager.apply_mining_speed(MiningSpeedQuery {
            block_state: 1,
            block_name: "minecraft:stone".to_string(),
            item_id: None,
            enchantments: Vec::new(),
            plugin_enchantments: Vec::new(),
            speed: 3.0,
        });

        assert_eq!(speed, 3.0);
    }

    #[test]
    fn missing_plugin_event_does_not_wait_for_plugin_lock() {
        let manager = PluginManager::empty_for_tests();
        let plugins = manager
            .plugins
            .get()
            .expect("empty plugin manager should be initialized");

        assert!(plugins.is_empty());
    }

    #[test]
    fn empty_plugin_manager_keeps_block_drops_default() {
        let manager = PluginManager::empty_for_tests();
        let drops = manager.apply_block_drops(BlockDropQuery {
            player: None,
            player_position: None,
            block_state: 1,
            block_name: "minecraft:stone".to_string(),
            position: BlockDropPosition { x: 0, y: 64, z: 0 },
            tool_item_id: None,
            enchantments: Vec::new(),
            plugin_enchantments: Vec::new(),
            default_item_id: Some(1),
        });

        assert!(drops.is_none());
    }

    #[test]
    fn plugin_files_only_keeps_libraries_in_stable_order() {
        let dir = std::env::temp_dir().join(format!(
            "qexed-plugins-files-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("b.dll"), []).unwrap();
        std::fs::write(dir.join("a.txt"), []).unwrap();
        std::fs::write(dir.join("a.dll"), []).unwrap();

        let names = plugin_files(&dir)
            .into_iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(names, ["a.dll", "b.dll"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

