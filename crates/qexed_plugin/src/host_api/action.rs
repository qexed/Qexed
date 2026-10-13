//! ActionHub：v6 宿主服务中枢（v4 宿主服务的原生类型版）。
//!
//! v4 中插件经 C-ABI 字符串查询调用宿主（world_edit / entity_control /
//! structured_storage / economy / random_pool / placeholder / http /
//! plugin_api / player_action）；v6 将同一能力收敛为本模块的强类型
//! trait 实现，并经 PluginManager 的子 trait 委托暴露给插件。
//!
//! 会话模型：v6 没有全局会话表，每个连接循环独占自己的 PacketSink。
//! 因此跨玩家动作走每连接一条无界帧队列：连接循环调用
//! register_player_session 拿接收端，派生任务把帧写入 sink（见 qexed_play）
//! 服务端退出/断开时调用 unregister_player_session。
//!
//! 世界编辑是视觉广播：不触碰 qexed_world（并行开发中），方块变更以
//! BlockUpdate 包推给在线客户端，并留一份环形缓存供重登重放。

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use qexed_packet::net_types::{Position as BlockPosition, VarInt};
use qexed_plugin_api::payload::*;
use qexed_plugin_api::services::*;
use qexed_plugin_api::PlayerInfo;
use qexed_protocol::to_client::play::add_entity as add_entity_pk;
use qexed_protocol::to_client::play::block_update as block_update_pk;
use qexed_protocol::to_client::play::boss_event as boss_event_pk;
use qexed_protocol::to_client::play::hurt_animation as hurt_animation_pk;
use qexed_protocol::to_client::play::position as position_pk;
use qexed_protocol::to_client::play::set_entity_data as set_entity_data_pk;
use qexed_protocol::to_client::play::set_entity_motion as set_entity_motion_pk;
use qexed_protocol::to_client::play::set_player_inventory as set_player_inventory_pk;
use qexed_protocol::to_client::play::system_chat as system_chat_pk;
use qexed_protocol::to_client::play::transfer as transfer_pk;

/// 单条待发客户端帧（已含包 id 前缀、未压缩）。
pub type OutboundFrame = bytes::Bytes;

// ---------------------------------------------------------------------------
// 会话注册表
// ---------------------------------------------------------------------------

/// 每连接的出站帧队列。连接循环从配套接收端取帧写入 PacketSink。
pub struct SessionQueue {
    tx: tokio::sync::mpsc::UnboundedSender<OutboundFrame>,
    entity_id: i32,
}

impl SessionQueue {
    /// 入队一帧；接收端已关闭（连接断开）时返回 false。
    pub fn push(&self, frame: OutboundFrame) -> bool {
        self.tx.send(frame).is_ok()
    }

    /// 该玩家的服务端实体 id（Velocity/DamagePlayer 等自指动作需要）。
    pub fn entity_id(&self) -> i32 {
        self.entity_id
    }
}

#[derive(Default)]
pub(crate) struct SessionRegistry {
    by_name: Mutex<HashMap<String, Arc<SessionQueue>>>,
}

impl SessionRegistry {
    pub(crate) fn register(
        &self,
        username: &str,
        entity_id: i32,
    ) -> tokio::sync::mpsc::UnboundedReceiver<OutboundFrame> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.by_name.lock().unwrap().insert(
            username.to_string(),
            Arc::new(SessionQueue { tx, entity_id }),
        );
        rx
    }

    pub(crate) fn unregister(&self, username: &str) {
        self.by_name.lock().unwrap().remove(username);
    }

    fn lookup(&self, username: &str) -> Option<Arc<SessionQueue>> {
        self.by_name.lock().unwrap().get(username).cloned()
    }

    fn names(&self) -> Vec<String> {
        self.by_name.lock().unwrap().keys().cloned().collect()
    }
}

// ---------------------------------------------------------------------------
// 运行时实体注册表
// ---------------------------------------------------------------------------

/// 插件运行时实体（NPC/展示实体）的服务端登记项。
pub(crate) struct ManagedEntity {
    pub spec: RuntimeEntitySpec,
    pub entity_id: i32,
    pub uuid: uuid::Uuid,
    pub main_hand_event: String,
    pub off_hand_event: String,
    pub attack_event: String,
}

#[derive(Default)]
pub(crate) struct EntityRegistry {
    pub by_key: HashMap<String, ManagedEntity>,
}

// ---------------------------------------------------------------------------
// 注册表索引（Mojang reports 派生，懒加载一次）
// ---------------------------------------------------------------------------

/// blocks.json 派生索引：名字（含带属性全名）→ 状态 id。
pub(crate) struct BlockStateIndex {
    pub name_to_id: HashMap<String, i32>,
    pub air: i32,
}

fn build_block_index() -> Option<BlockStateIndex> {
    let report = qexed_registry::load_blocks_report().ok()?;
    let mut name_to_id = HashMap::new();
    let mut air = 0i32;
    for (name, entry) in report.as_object()? {
        let states = entry.get("states")?.as_array()?;
        let mut best: Option<(bool, bool, i64)> = None;
        let mut best_id = 0i32;
        for st in states {
            let id = st.get("id")?.as_i64()? as i32;
            let props = st.get("properties").and_then(|p| p.as_object());
            let has_props = props.map(|p| !p.is_empty()).unwrap_or(false);
            let default = st.get("default").and_then(|d| d.as_bool()).unwrap_or(false);
            if name == "minecraft:air" && !has_props {
                air = id;
            }
            if has_props {
                if let Some(p) = props {
                    let mut kv: Vec<(String, String)> = p
                        .iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
                        .collect();
                    kv.sort();
                    let key = format!(
                        "{name}[{}]",
                        kv.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(",")
                    );
                    name_to_id.insert(key, id);
                }
            }
            let score = (!has_props, default, -(id as i64));
            if best.map(|b| score > b).unwrap_or(true) {
                best = Some(score);
                best_id = id;
            }
        }
        name_to_id.insert(name.clone(), best_id);
        if let Some(stripped) = name.strip_prefix("minecraft:") {
            name_to_id.insert(stripped.to_string(), best_id);
        }
    }
    Some(BlockStateIndex { name_to_id, air })
}

/// registries.json 派生 id 表（item / entity_type / enchantment）。
pub(crate) struct IdMaps {
    pub item: HashMap<String, i32>,
    pub entity: HashMap<String, i32>,
    pub enchant: HashMap<String, i32>,
}

fn load_id_map(registry_id: &str) -> HashMap<String, i32> {
    let mut map = qexed_registry::load_registry_id_map(registry_id).unwrap_or_default();
    let aliases: Vec<(String, i32)> = map
        .iter()
        .filter_map(|(k, v)| k.strip_prefix("minecraft:").map(|s| (s.to_string(), *v)))
        .collect();
    for (k, v) in aliases {
        map.entry(k).or_insert(v);
    }
    map
}

fn build_id_maps() -> IdMaps {
    IdMaps {
        item: load_id_map("minecraft:item"),
        entity: load_id_map("minecraft:entity_type"),
        enchant: load_id_map("minecraft:enchantment"),
    }
}

#[derive(Default)]
pub(crate) struct RegistryCaches {
    block: Option<Arc<BlockStateIndex>>,
    ids: Option<Arc<IdMaps>>,
}

// ---------------------------------------------------------------------------
// 经济 / 随机池 / 占位符 / 跨插件服务 状态
// ---------------------------------------------------------------------------

#[derive(Default)]
pub(crate) struct EconomyState {
    pub currencies: HashMap<String, CurrencyInfo>,
    pub balances: HashMap<String, HashMap<String, i64>>,
    pub loaded: bool,
}

#[derive(Default)]
pub(crate) struct RandomPoolState {
    pools: HashMap<(String, String), VecDeque<String>>,
}

pub(crate) struct PlaceholderEntry {
    pub plugin_id: String,
    pub resolver: Arc<dyn PlaceholderResolver>,
}

#[derive(Default)]
pub(crate) struct PlaceholderState {
    /// 归一化 key（无百分号）→ 条目。
    pub resolvers: HashMap<String, PlaceholderEntry>,
}

pub(crate) struct ServiceEntry {
    pub plugin_id: String,
    pub provider: Arc<dyn PluginServiceProvider>,
}

#[derive(Default)]
pub(crate) struct InterPluginState {
    pub providers: HashMap<String, ServiceEntry>,
}

// ---------------------------------------------------------------------------
// ActionHub
// ---------------------------------------------------------------------------

/// 宿主服务中枢。全局单例，经 crate::action_hub() 访问。
pub struct ActionHub {
    pub(crate) sessions: SessionRegistry,
    pub(crate) entities: Mutex<EntityRegistry>,
    pub(crate) edit_regions: RwLock<HashMap<String, WorldEditRegionSpec>>,
    /// 视觉方块变更环形缓存（dimension, pos, state），供重登重放。
    pub(crate) world_cache: Mutex<VecDeque<(String, (i32, i32, i32), i32)>>,
    pub(crate) registries: Mutex<RegistryCaches>,
    pub(crate) storage_root: RwLock<Option<PathBuf>>,
    pub(crate) economy: Mutex<EconomyState>,
    pub(crate) random_pools: Mutex<RandomPoolState>,
    pub(crate) placeholders: Mutex<PlaceholderState>,
    pub(crate) services: Mutex<InterPluginState>,
    next_entity_id: AtomicI32,
    next_teleport_id: AtomicI32,
    /// 插件命令注册表：command → plugin_id。
    pub(crate) commands: Mutex<HashMap<String, String>>,
}

impl ActionHub {
    pub(crate) fn new() -> Self {
        Self {
            sessions: SessionRegistry::default(),
            entities: Mutex::new(EntityRegistry::default()),
            edit_regions: RwLock::new(HashMap::new()),
            world_cache: Mutex::new(VecDeque::new()),
            registries: Mutex::new(RegistryCaches::default()),
            storage_root: RwLock::new(None),
            economy: Mutex::new(EconomyState::default()),
            random_pools: Mutex::new(RandomPoolState::default()),
            placeholders: Mutex::new(PlaceholderState::default()),
            services: Mutex::new(InterPluginState::default()),
            next_entity_id: AtomicI32::new(2_000_000_000),
            next_teleport_id: AtomicI32::new(1),
            commands: Mutex::new(HashMap::new()),
        }
    }

    // —— 会话生命周期（qexed_play 连接循环调用） ——

    /// 注册在线玩家会话，返回出站帧接收端。连接循环应派生任务持续把帧
    /// 写入该连接的 PacketSink（帧已是完整包，直接 send_encoded_frame）。
    pub fn register_player_session(
        &self,
        username: &str,
        entity_id: i32,
    ) -> tokio::sync::mpsc::UnboundedReceiver<OutboundFrame> {
        self.sessions.register(username, entity_id)
    }

    /// 注销会话（断开/切换世界时调用）。
    pub fn unregister_player_session(&self, username: &str) {
        self.sessions.unregister(username);
    }

    /// 会话队列（qexed_play 世界编辑重放等直接投递场景用）。
    pub fn session(&self, username: &str) -> Option<Arc<SessionQueue>> {
        self.sessions.lookup(username)
    }

    // —— NPC 交互路由（qexed_play 交互/攻击解码后调用） ——

    /// 按服务端实体 id 查运行时实体及其交互事件名。
    // —— 插件命令注册表（CommandApi） ——

    /// 注册插件顶层命令；同一命令已注册时保留首个（先加载者优先）。
    pub fn command_register(&self, plugin_id: &str, command: &str) {
        let mut st = self.commands.lock().unwrap();
        if st.contains_key(command) {
            log::warn!("插件命令 {command} 已被 {} 注册，忽略 {}", st[command], plugin_id);
            return;
        }
        st.insert(command.to_string(), plugin_id.to_string());
    }

    /// 全部插件顶层命令（快照）。
    pub fn plugin_command_names(&self) -> Vec<String> {
        self.commands.lock().unwrap().keys().cloned().collect()
    }

    /// 注销某插件的全部命令（禁用/卸载时调用）。
    pub fn command_unregister_plugin(&self, plugin_id: &str) {
        self.commands.lock().unwrap().retain(|_, owner| owner != plugin_id);
    }

    pub fn npc_event_by_entity_id(
        &self,
        entity_id: i32,
        interaction: NpcInteraction,
    ) -> Option<(String, RuntimeEntitySpec, String)> {
        let reg = self.entities.lock().unwrap();
        for (key, e) in reg.by_key.iter() {
            if e.entity_id == entity_id {
                let event = match interaction {
                    NpcInteraction::MainHand => e.main_hand_event.clone(),
                    NpcInteraction::OffHand => e.off_hand_event.clone(),
                    NpcInteraction::Attack => e.attack_event.clone(),
                };
                return Some((key.clone(), e.spec.clone(), event));
            }
        }
        None
    }

    /// 运行时实体当前 (x, y, z, yaw, pitch)。
    pub fn entity_position(&self, key: &str) -> Option<(f64, f64, f64, f32, f32)> {
        let reg = self.entities.lock().unwrap();
        let e = reg.by_key.get(key)?;
        Some((e.spec.x, e.spec.y, e.spec.z, e.spec.yaw, e.spec.pitch))
    }

    // —— 编码与分发原语 ——

    pub(crate) fn encode<T: qexed_packet::Packet>(packet: T) -> Option<OutboundFrame> {
        qexed_tcp_connect::PacketSink::<tokio::io::Sink>::build_send_packet(packet).ok()
    }

    /// 单帧直发（会话排空走 UnboundedReceiver；此接口供精确单帧场景）。
    #[allow(dead_code)]
    pub(crate) fn deliver(&self, username: &str, frame: OutboundFrame) -> bool {
        self.session(username).map(|q| q.push(frame)).unwrap_or(false)
    }

    pub(crate) fn broadcast(&self, frame: &OutboundFrame) {
        for name in self.sessions.names() {
            if let Some(q) = self.session(&name) {
                let _ = q.push(frame.clone());
            }
        }
    }

    // —— 注册表索引（懒加载） ——

    fn block_index(&self) -> Option<Arc<BlockStateIndex>> {
        let mut caches = self.registries.lock().unwrap();
        if caches.block.is_none() {
            match build_block_index() {
                Some(index) => caches.block = Some(Arc::new(index)),
                None => {
                    log::warn!("blocks.json 不可用，世界编辑/物品解析将失败");
                    return None;
                }
            }
        }
        caches.block.clone()
    }

    fn id_maps(&self) -> Option<Arc<IdMaps>> {
        let mut caches = self.registries.lock().unwrap();
        if caches.ids.is_none() {
            caches.ids = Some(Arc::new(build_id_maps()));
        }
        caches.ids.clone()
    }

    /// 方块参数解析：接受十进制状态 id、minecraft:name、name、
    /// name[k=v,...]（属性按字母序）。未知返回 None（绝不猜 id）。
    pub(crate) fn resolve_block_state(&self, block: &str) -> Option<i32> {
        let block = block.trim();
        if let Ok(id) = block.parse::<i32>() {
            return Some(id);
        }
        let index = self.block_index()?;
        let full = if block.contains(':') {
            block.to_string()
        } else {
            format!("minecraft:{block}")
        };
        index.name_to_id.get(block).copied().or_else(|| index.name_to_id.get(&full).copied())
    }

    pub(crate) fn air_state(&self) -> i32 {
        self.block_index().map(|i| i.air).unwrap_or(0)
    }

    pub(crate) fn item_id(&self, item: &str) -> Option<i32> {
        let item = item.trim();
        if let Ok(id) = item.parse::<i32>() {
            return Some(id);
        }
        let maps = self.id_maps()?;
        let full = if item.contains(':') {
            item.to_string()
        } else {
            format!("minecraft:{item}")
        };
        maps.item.get(item).copied().or_else(|| maps.item.get(&full).copied())
    }

    pub(crate) fn entity_type_id(&self, entity_type: &str) -> Option<i32> {
        let et = entity_type.trim();
        if let Ok(id) = et.parse::<i32>() {
            return Some(id);
        }
        let maps = self.id_maps()?;
        let full = if et.contains(':') {
            et.to_string()
        } else {
            format!("minecraft:{et}")
        };
        maps.entity.get(et).copied().or_else(|| maps.entity.get(&full).copied())
    }

    pub(crate) fn enchantment_id(&self, enchant: &str) -> Option<i32> {
        let e = enchant.trim();
        if let Ok(id) = e.parse::<i32>() {
            return Some(id);
        }
        let maps = self.id_maps()?;
        let full = if e.contains(':') {
            e.to_string()
        } else {
            format!("minecraft:{e}")
        };
        maps.enchant.get(e).copied().or_else(|| maps.enchant.get(&full).copied())
    }

    // —— 世界编辑应用（视觉广播 + 环形缓存） ——

    pub(crate) fn apply_block_changes(
        &self,
        dimension: &str,
        changes: &[((i32, i32, i32), i32)],
    ) -> usize {
        if changes.is_empty() {
            return 0;
        }
        let frames: Vec<OutboundFrame> = changes
            .iter()
            .filter_map(|(pos, state)| {
                Self::encode(block_update_pk::BlockUpdate {
                    location: BlockPosition { x: pos.0, y: pos.1, z: pos.2 },
                    block_state: VarInt(*state),
                })
            })
            .collect();
        {
            let mut cache = self.world_cache.lock().unwrap();
            for (pos, state) in changes {
                cache.push_back((dimension.to_string(), *pos, *state));
            }
            while cache.len() > 200_000 {
                cache.pop_front();
            }
        }
        let names = self.sessions.names();
        for name in &names {
            if let Some(q) = self.session(name) {
                for frame in &frames {
                    let _ = q.push(frame.clone());
                }
            }
        }
        log::debug!(
            "world edit: {dim} x{n} 方块广播至 {p} 个会话",
            dim = dimension,
            n = frames.len(),
            p = names.len()
        );
        frames.len()
    }

    /// 向会话重放全部视觉方块变更（重登/换世界后调用）。
    pub fn replay_world_edits(&self, username: &str) {
        let Some(queue) = self.session(username) else {
            return;
        };
        let cache = self.world_cache.lock().unwrap();
        for (_dimension, pos, state) in cache.iter() {
            if let Some(frame) = Self::encode(block_update_pk::BlockUpdate {
                location: BlockPosition { x: pos.0, y: pos.1, z: pos.2 },
                block_state: VarInt(*state),
            }) {
                let _ = queue.push(frame);
            }
        }
    }

    // —— 结构化存储路径 ——

    fn storage_path(&self, plugin_id: &str, key: &str) -> Option<PathBuf> {
        fn valid_component(s: &str) -> bool {
            !s.is_empty()
                && s != "."
                && s != ".."
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ' '))
        }
        if !valid_component(plugin_id) {
            return None;
        }
        let mut rel = PathBuf::from(plugin_id);
        for part in key.split('/') {
            if !valid_component(part) {
                return None;
            }
            rel.push(part);
        }
        {
            let mut root = self.storage_root.write().unwrap();
            if root.is_none() {
                let dir = qexed_config::config_path()
                    .ok()
                    .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                    .unwrap_or_else(|| PathBuf::from("."));
                *root = Some(dir.join("storage"));
            }
        }
        self.storage_root.read().unwrap().as_ref().map(|r| r.join(rel))
    }

    // —— 实体 upsert 内核 ——

    pub(crate) fn entity_upsert_inner(
        &self,
        entity: &RuntimeEntitySpec,
        main_hand_event: &str,
        off_hand_event: &str,
        attack_event: &str,
    ) -> bool {
        let Some(type_id) = self.entity_type_id(&entity.entity_type) else {
            log::warn!("entity upsert 失败: 未知实体类型 {t}", t = entity.entity_type);
            return false;
        };
        let (entity_id, uuid) = {
            let reg = self.entities.lock().unwrap();
            match reg.by_key.get(&entity.key) {
                Some(e) => (e.entity_id, e.uuid),
                None => {
                    let id = self.next_entity_id.fetch_add(1, Ordering::Relaxed);
                    (id, uuid::Uuid::new_v4())
                }
            }
        };
        {
            let mut reg = self.entities.lock().unwrap();
            reg.by_key.insert(
                entity.key.clone(),
                ManagedEntity {
                    spec: RuntimeEntitySpec {
                        key: entity.key.clone(),
                        dimension: entity.dimension.clone(),
                        entity_type: entity.entity_type.clone(),
                        x: entity.x,
                        y: entity.y,
                        z: entity.z,
                        yaw: entity.yaw,
                        pitch: entity.pitch,
                        display_name: entity.display_name.clone(),
                        ai: entity.ai.clone(),
                        ai_params_json: entity.ai_params_json.clone(),
                        auto_jump: entity.auto_jump,
                    },
                    entity_id,
                    uuid,
                    main_hand_event: main_hand_event.to_string(),
                    off_hand_event: off_hand_event.to_string(),
                    attack_event: attack_event.to_string(),
                },
            );
        }

        let position = add_entity_pk::EntityPosition {
            x: entity.x,
            y: entity.y,
            z: entity.z,
            yaw: entity.yaw,
            pitch: entity.pitch,
            on_ground: true,
        };
        if let Some(frame) = Self::encode(add_entity_pk::AddEntity::new(
            entity_id, uuid, type_id, position, 0,
        )) {
            self.broadcast(&frame);
        }
        if !entity.display_name.trim().is_empty() {
            let metadata = qexed_protocol::types::EntityMetadata {
                data: vec![
                    qexed_protocol::types::EntityMetadataSub {
                        index: 2,
                        data: Some(qexed_protocol::types::EntityMetadataEnum::OptionTextComponent(Some(
                            qexed_chat::text_component(entity.display_name.as_str()),
                        ))),
                    },
                    qexed_protocol::types::EntityMetadataSub {
                        index: 3,
                        data: Some(qexed_protocol::types::EntityMetadataEnum::Boolean(true)),
                    },
                    qexed_protocol::types::EntityMetadataSub {
                        index: 0xff,
                        data: None,
                    },
                ],
            };
            if let Some(frame) = Self::encode(set_entity_data_pk::SetEntityData {
                entity_id: VarInt(entity_id),
                metadata,
            }) {
                self.broadcast(&frame);
            }
        }
        true
    }

    // —— BossBar 稳定 id ——

    fn bossbar_uuid(id: &str) -> uuid::Uuid {
        uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_DNS, id.as_bytes())
    }
}

/// NPC 交互种类（qexed_play 解码侧使用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcInteraction {
    MainHand,
    OffHand,
    Attack,
}

impl ActionHub {
    // —— 玩家动作 ——

    /// 对目标玩家执行动作序列（编码并入队；不在线则整体静默跳过），
    /// 返回成功递交的帧数。
    pub fn apply_player_actions(&self, username: &str, actions: &[PlayerAction]) -> usize {
        let Some(queue) = self.session(username) else {
            return 0;
        };
        let entity_id = queue.entity_id();
        let mut applied = 0usize;
        for action in actions {
            for frame in self.encode_action(action, entity_id) {
                if queue.push(frame) {
                    applied += 1;
                }
            }
        }
        applied
    }

    /// 单动作 → 0..n 条客户端帧。entity_id 为目标玩家自身服务端实体 id。
    pub(crate) fn encode_action(&self, action: &PlayerAction, entity_id: i32) -> Vec<OutboundFrame> {
        match action {
            PlayerAction::SystemMessage { text, translate, with, overlay } => {
                let content = if !translate.trim().is_empty() {
                    qexed_chat::translatable_component(
                        translate.as_str(),
                        with.iter().map(|w| qexed_chat::text_component(w)).collect(),
                    )
                } else {
                    qexed_chat::text_component(text.as_str())
                };
                self.encode_one(system_chat_pk::SystemChat { content, overlay: *overlay })
            }
            PlayerAction::Teleport { dimension: _, x, y, z, yaw, pitch } => {
                // v6 单世界：dimension 忽略，位置强制重定位。
                self.encode_one(position_pk::Position {
                    teleport_id: VarInt(self.next_teleport_id.fetch_add(1, Ordering::Relaxed)),
                    x: *x,
                    y: *y,
                    z: *z,
                    dx: 0.0,
                    dy: 0.0,
                    dz: 0.0,
                    yaw: yaw.unwrap_or(0.0),
                    pitch: pitch.unwrap_or(0.0),
                    flags: 0,
                })
            }
            PlayerAction::Transfer { host, port, message } => {
                let mut frames = Vec::new();
                if !message.trim().is_empty() {
                    if let Some(f) = self.encode_packet(system_chat_pk::SystemChat {
                        content: qexed_chat::text_component(message.as_str()),
                        overlay: false,
                    }) {
                        frames.push(f);
                    }
                }
                if let Some(f) = self.encode_packet(transfer_pk::Transfer::new(host, *port)) {
                    frames.push(f);
                }
                frames
            }
            PlayerAction::ProxyConnect { server, message } => {
                // v6 无代理集成：降级为提示消息。
                log::info!("ProxyConnect 降级（无代理集成）: target={server}");
                if message.trim().is_empty() {
                    Vec::new()
                } else {
                    self.encode_one(system_chat_pk::SystemChat {
                        content: qexed_chat::text_component(message.as_str()),
                        overlay: false,
                    })
                }
            }
            PlayerAction::OpenMenu { menu } => {
                log::warn!("OpenMenu 降级（v6 暂无箱子菜单基础设施）: menu={menu}");
                Vec::new()
            }
            PlayerAction::GiveItem { item, count, name, lore, enchantments, plugin_enchantments } => {
                let mut frames = Vec::new();
                if *count <= 0 {
                    return frames;
                }
                let Some(item_id) = self.item_id(item) else {
                    log::warn!("GiveItem 失败: 未知物品 {item}");
                    return frames;
                };
                let mut components: Vec<qexed_protocol::types::ComponentsToAdd> = Vec::new();
                if !name.trim().is_empty() {
                    components.push(qexed_protocol::types::ComponentsToAdd::MinecraftCustomName(
                        qexed_protocol::types::minecraft::CustomName {
                            name: qexed_chat::text_component(name.as_str()),
                        },
                    ));
                }
                if !lore.is_empty() {
                    components.push(qexed_protocol::types::ComponentsToAdd::MinecraftLore(
                        qexed_protocol::types::minecraft::Lore {
                            lines: lore.iter().map(|l| qexed_chat::text_component(l)).collect(),
                        },
                    ));
                }
                if !enchantments.is_empty() {
                    let list: Vec<qexed_protocol::types::minecraft::Enchantment> = enchantments
                        .iter()
                        .filter_map(|e| {
                            let id = self.enchantment_id(&e.id)?;
                            Some(qexed_protocol::types::minecraft::Enchantment {
                                enchantment: VarInt(id),
                                level: VarInt(e.level.max(0)),
                            })
                        })
                        .collect();
                    if !list.is_empty() {
                        components.push(qexed_protocol::types::ComponentsToAdd::MinecraftEnchantments(
                            qexed_protocol::types::minecraft::Enchantments { enchantments: list },
                        ));
                    }
                }
                if !plugin_enchantments.is_empty() {
                    log::debug!(
                        "GiveItem: {n} 个插件附魔无原版组件映射，已跳过",
                        n = plugin_enchantments.len()
                    );
                }
                let contents = qexed_protocol::types::Slot {
                    item_count: VarInt(*count as i32),
                    item_id: Some(VarInt(item_id)),
                    number_of_components_to_add: Some(VarInt(components.len() as i32)),
                    number_of_components_to_remove: Some(VarInt(0)),
                    components_to_add: Some(components),
                    components_to_remove: Some(Vec::new()),
                };
                if let Some(f) = self.encode_packet(set_player_inventory_pk::SetPlayerInventory {
                    slot: VarInt(36),
                    contents,
                }) {
                    frames.push(f);
                }
                frames
            }
            PlayerAction::ResetInventory { restore_menu_items } => {
                log::warn!(
                    "ResetInventory 降级（v6 无服务端背包副本）: restore_menu_items={r}",
                    r = restore_menu_items
                );
                Vec::new()
            }
            PlayerAction::SetPlayersVisible { visible } => {
                log::warn!("SetPlayersVisible 降级（v6 无玩家实体可见性开关）: visible={v}", v = visible);
                Vec::new()
            }
            PlayerAction::SpawnProjectile { kind, .. } => {
                log::warn!("SpawnProjectile 降级（v6 无服务端弹射物运行时）: kind={kind}");
                Vec::new()
            }
            PlayerAction::Velocity { x, y, z, additive: _ } => {
                // additive 语义无法忠实还原（无速度查询）——按绝对速度处理。
                self.encode_one(set_entity_motion_pk::SetEntityMotion::from_velocity(
                    entity_id, *x, *y, *z,
                ))
            }
            PlayerAction::DamagePlayer { amount, .. } => {
                // 视觉受伤动画；真实血量扣减需 v6 生存运行时（暂缺）。
                log::debug!("DamagePlayer {amount} → HurtAnimation（视觉）");
                self.encode_one(hurt_animation_pk::HurtAnimation {
                    entity_id: VarInt(entity_id),
                    yaw: 0.0,
                })
            }
            PlayerAction::BossBar { id, title, progress, color, overlay } => {
                let uuid = Self::bossbar_uuid(id);
                let progress = progress.clamp(0.0, 1.0);
                if title.trim().is_empty() {
                    // 仅 id+进度：视为进度更新（沿用首次注册时的样式）。
                    self.encode_one(boss_event_pk::BossEvent::update_progress(uuid, progress))
                } else {
                    self.encode_one(boss_event_pk::BossEvent::add_with_options(
                        uuid,
                        qexed_chat::text_component(title.as_str()),
                        progress,
                        parse_boss_color(color),
                        parse_boss_overlay(overlay),
                        boss_event_pk::BossBarProperties::default(),
                    ))
                }
            }
            PlayerAction::RemoveBossBar { id } => {
                self.encode_one(boss_event_pk::BossEvent::remove(Self::bossbar_uuid(id)))
            }
            PlayerAction::SendBedrockForm { username: target, .. } => {
                log::warn!("SendBedrockForm 降级（v6 无 Geyser 集成）: target={target}");
                Vec::new()
            }
        }
    }

    fn encode_packet<T: qexed_packet::Packet>(&self, packet: T) -> Option<OutboundFrame> {
        Self::encode(packet)
    }

    /// 单包编码为 Vec（encode_action 各分支统一返回形态）。
    fn encode_one<T: qexed_packet::Packet>(&self, packet: T) -> Vec<OutboundFrame> {
        Self::encode(packet).into_iter().collect()
    }
}

fn parse_boss_color(color: &str) -> boss_event_pk::BossBarColor {
    match color.trim().to_ascii_lowercase().as_str() {
        "pink" => boss_event_pk::BossBarColor::Pink,
        "blue" => boss_event_pk::BossBarColor::Blue,
        "red" => boss_event_pk::BossBarColor::Red,
        "yellow" => boss_event_pk::BossBarColor::Yellow,
        "purple" => boss_event_pk::BossBarColor::Purple,
        "white" => boss_event_pk::BossBarColor::White,
        _ => boss_event_pk::BossBarColor::Green,
    }
}

fn parse_boss_overlay(overlay: &str) -> boss_event_pk::BossBarOverlay {
    match overlay.trim().to_ascii_lowercase().as_str() {
        "notched_6" | "notched6" | "6" => boss_event_pk::BossBarOverlay::Notched6,
        "notched_10" | "notched10" | "10" => boss_event_pk::BossBarOverlay::Notched10,
        "notched_12" | "notched12" | "12" => boss_event_pk::BossBarOverlay::Notched12,
        "notched_20" | "notched20" | "20" => boss_event_pk::BossBarOverlay::Notched20,
        _ => boss_event_pk::BossBarOverlay::Progress,
    }
}

// ===========================================================================
// 服务内核（ActionHub inherent 方法）
// ===========================================================================

impl ActionHub {
    // —— 玩家名快照 ——

    pub fn online_player_names(&self) -> Vec<String> {
        self.sessions.names()
    }

    // —— 世界编辑 ——

    pub fn world_set_block(&self, dimension: &str, pos: (i32, i32, i32), block: &str) -> bool {
        let Some(state) = self.resolve_block_state(block) else {
            log::warn!("world_set_block: 未知方块 {block}");
            return false;
        };
        let change = (pos, state);
        self.apply_block_changes(dimension, std::slice::from_ref(&change)) == 1
    }

    pub fn world_set_blocks(&self, blocks: &[(&str, (i32, i32, i32), &str)]) -> usize {
        let mut by_dim: HashMap<&str, Vec<((i32, i32, i32), i32)>> = HashMap::new();
        for (dim, pos, block) in blocks {
            let Some(state) = self.resolve_block_state(block) else {
                log::warn!("world_set_blocks: 未知方块 {block}");
                continue;
            };
            by_dim.entry(*dim).or_default().push((*pos, state));
        }
        let mut total = 0usize;
        for (dim, changes) in by_dim {
            total += self.apply_block_changes(dim, &changes);
        }
        total
    }

    pub fn world_break_block(&self, dimension: &str, pos: (i32, i32, i32)) -> bool {
        let air = self.air_state();
        let change = (pos, air);
        self.apply_block_changes(dimension, std::slice::from_ref(&change)) == 1
    }

    pub fn world_register_edit_region(&self, region: &WorldEditRegionSpec) -> bool {
        let cloned = WorldEditRegionSpec {
            id: region.id.clone(),
            dimension: region.dimension.clone(),
            min: region.min,
            max: region.max,
            allow_player_break: region.allow_player_break,
            allow_player_place: region.allow_player_place,
            allow_plugin_write: region.allow_plugin_write,
            runtime_only: region.runtime_only,
        };
        self.edit_regions.write().unwrap().insert(cloned.id.clone(), cloned);
        true
    }

    // —— 运行时实体移动 / 移除 ——

    pub(crate) fn entity_move_inner(&self, key: &str, position: NpcPosition) -> bool {
        let (entity_id, yaw, pitch) = {
            let mut reg = self.entities.lock().unwrap();
            let Some(e) = reg.by_key.get_mut(key) else {
                log::warn!("entity_move: 未知实体 key={key}");
                return false;
            };
            e.spec.x = position.x;
            e.spec.y = position.y;
            e.spec.z = position.z;
            e.spec.yaw = position.yaw;
            e.spec.pitch = position.pitch;
            (e.entity_id, position.yaw, position.pitch)
        };
        let position = add_entity_pk::EntityPosition {
            x: position.x,
            y: position.y,
            z: position.z,
            yaw,
            pitch,
            on_ground: true,
        };
        if let Some(frame) = Self::encode(add_entity_pk::TeleportEntity::from_position(entity_id, position)) {
            self.broadcast(&frame);
        }
        if let Some(frame) = Self::encode(add_entity_pk::RotateHead::new(entity_id, yaw)) {
            self.broadcast(&frame);
        }
        true
    }

    pub(crate) fn entity_remove_inner(&self, key: &str) -> bool {
        let entity_id = {
            let mut reg = self.entities.lock().unwrap();
            match reg.by_key.remove(key) {
                Some(e) => e.entity_id,
                None => return false,
            }
        };
        if let Some(frame) = Self::encode(add_entity_pk::RemoveEntities::one(entity_id)) {
            self.broadcast(&frame);
        }
        true
    }

    // —— 结构化存储 ——

    pub fn storage_exists(&self, plugin_id: &str, key: &str) -> bool {
        self.storage_path(plugin_id, key).is_some_and(|p| p.exists())
    }

    pub fn storage_get(&self, plugin_id: &str, key: &str) -> Option<Vec<u8>> {
        let p = self.storage_path(plugin_id, key)?;
        std::fs::read(p).ok()
    }

    pub fn storage_set(&self, plugin_id: &str, key: &str, value: &[u8]) -> bool {
        let Some(p) = self.storage_path(plugin_id, key) else {
            log::warn!("structured_storage: 非法 plugin_id/key（仅字母数字 _-. 空格）");
            return false;
        };
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&p, value).is_ok()
    }

    pub fn storage_delete(&self, plugin_id: &str, key: &str) -> bool {
        let Some(p) = self.storage_path(plugin_id, key) else {
            return false;
        };
        p.exists() && std::fs::remove_file(&p).is_ok()
    }
}

// ===========================================================================
// 经济内核
// ===========================================================================

/// 经济账本落盘结构（storage/_economy/economy.json）。
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct EconomyDump {
    currencies: HashMap<String, CurrencyInfo>,
    balances: HashMap<String, HashMap<String, i64>>,
}

impl ActionHub {
    fn ensure_economy_loaded(&self) {
        let mut econ = self.economy.lock().unwrap();
        if econ.loaded {
            return;
        }
        econ.loaded = true;
        let Some(path) = self.storage_path("_economy", "economy.json") else {
            return;
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return;
        };
        match serde_json::from_str::<EconomyDump>(&text) {
            Ok(dump) => {
                econ.currencies = dump.currencies;
                econ.balances = dump.balances;
            }
            Err(e) => log::warn!("经济账本解析失败（忽略）: {e}"),
        }
    }

    fn save_economy(&self, econ: &EconomyState) {
        let Some(path) = self.storage_path("_economy", "economy.json") else {
            return;
        };
        let dump = EconomyDump {
            currencies: econ.currencies.clone(),
            balances: econ.balances.clone(),
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match serde_json::to_string_pretty(&dump) {
            Ok(text) => {
                if let Err(e) = std::fs::write(&path, text) {
                    log::warn!("经济账本写入失败: {e}");
                }
            }
            Err(e) => log::warn!("经济账本序列化失败: {e}"),
        }
    }

    pub fn economy_register_currency(
        &self,
        _plugin_id: &str,
        id: &str,
        name: &str,
        symbol: &str,
        fractional_digits: i32,
    ) -> bool {
        self.ensure_economy_loaded();
        let mut econ = self.economy.lock().unwrap();
        let key = id.trim().to_ascii_lowercase();
        if key.is_empty() || econ.currencies.contains_key(&key) {
            return false;
        }
        econ.currencies.insert(
            key.clone(),
            CurrencyInfo {
                id: key,
                name: name.to_string(),
                symbol: symbol.to_string(),
                fractional_digits,
            },
        );
        self.save_economy(&econ);
        true
    }

    pub fn economy_currency_info(&self, currency: &str) -> Option<CurrencyInfo> {
        self.ensure_economy_loaded();
        let econ = self.economy.lock().unwrap();
        econ.currencies.get(&currency.trim().to_ascii_lowercase()).cloned()
    }

    /// 全部账本导出（v4 economy_storage 语义：JSON 字符串）。
    pub fn economy_storage(&self, currency: &str) -> Option<String> {
        self.ensure_economy_loaded();
        let econ = self.economy.lock().unwrap();
        let key = currency.trim().to_ascii_lowercase();
        if !econ.currencies.contains_key(&key) {
            return None;
        }
        let balances = econ.balances.get(&key).cloned().unwrap_or_default();
        serde_json::to_string(&balances).ok()
    }

    pub fn economy_balance(&self, player: &str, currency: &str) -> Option<i64> {
        self.ensure_economy_loaded();
        let econ = self.economy.lock().unwrap();
        let cur = currency.trim().to_ascii_lowercase();
        if !econ.currencies.contains_key(&cur) {
            return None;
        }
        econ.balances.get(&cur).and_then(|m| m.get(player)).copied()
    }

    pub fn economy_set_balance(&self, player: &str, currency: &str, amount: i64) -> Option<i64> {
        self.ensure_economy_loaded();
        let mut econ = self.economy.lock().unwrap();
        let cur = currency.trim().to_ascii_lowercase();
        if !econ.currencies.contains_key(&cur) {
            return None;
        }
        let entry = econ.balances.entry(cur).or_default().entry(player.to_string()).or_insert(0);
        *entry = amount;
        let result = *entry;
        drop(econ);
        self.save_economy(&self.economy.lock().unwrap());
        Some(result)
    }

    pub fn economy_deposit(&self, player: &str, currency: &str, amount: i64) -> Option<i64> {
        self.ensure_economy_loaded();
        let mut econ = self.economy.lock().unwrap();
        let cur = currency.trim().to_ascii_lowercase();
        if !econ.currencies.contains_key(&cur) {
            return None;
        }
        let entry = econ.balances.entry(cur).or_default().entry(player.to_string()).or_insert(0);
        *entry += amount;
        let result = *entry;
        drop(econ);
        self.save_economy(&self.economy.lock().unwrap());
        Some(result)
    }

    pub fn economy_withdraw(&self, player: &str, currency: &str, amount: i64) -> Option<i64> {
        self.ensure_economy_loaded();
        let mut econ = self.economy.lock().unwrap();
        let cur = currency.trim().to_ascii_lowercase();
        if !econ.currencies.contains_key(&cur) {
            return None;
        }
        let balances = econ.balances.get_mut(&cur)?;
        let entry = balances.get_mut(player)?;
        if *entry < amount {
            return None;
        }
        *entry -= amount;
        let result = *entry;
        drop(econ);
        self.save_economy(&self.economy.lock().unwrap());
        Some(result)
    }
}

// ===========================================================================
// 随机池内核
// ===========================================================================

impl ActionHub {
    /// 加权抽取。pool_id 相同且 precompute_count 更大时重建缓存（v4 语义）。
    pub fn random_pool_roll(
        &self,
        kind: &str,
        pool_id: &str,
        entries: &[(&str, u64)],
        precompute_count: usize,
    ) -> Option<String> {
        if entries.is_empty() {
            return None;
        }
        let total: u64 = entries.iter().map(|(_, w)| *w).sum();
        if total == 0 {
            return None;
        }
        let rebuild = {
            let pools = self.random_pools.lock().unwrap();
            match pools.pools.get(&(kind.to_string(), pool_id.to_string())) {
                Some(cached) => cached.len() < precompute_count,
                None => true,
            }
        };
        if rebuild {
            let mut fresh = VecDeque::new();
            let mut rng = rand::thread_rng();
            for _ in 0..precompute_count.max(1) {
                let mut pick = rand::Rng::gen_range(&mut rng, 0..total);
                for (value, weight) in entries {
                    if pick < *weight {
                        fresh.push_back(value.to_string());
                        break;
                    }
                    pick -= *weight;
                }
            }
            self.random_pools.lock().unwrap().pools.insert(
                (kind.to_string(), pool_id.to_string()),
                fresh,
            );
        }
        let mut pools = self.random_pools.lock().unwrap();
        let pool = pools.pools.get_mut(&(kind.to_string(), pool_id.to_string()))?;
        pool.pop_front()
    }
}

// ===========================================================================
// 占位符内核
// ===========================================================================

impl ActionHub {
    pub fn placeholder_register(
        &self,
        plugin_id: &str,
        key: &str,
        resolver: Arc<dyn PlaceholderResolver>,
    ) -> bool {
        let norm = normalize_placeholder_key(key);
        if norm.is_empty() {
            return false;
        }
        let mut st = self.placeholders.lock().unwrap();
        st.resolvers.insert(
            norm,
            PlaceholderEntry { plugin_id: plugin_id.to_string(), resolver },
        );
        true
    }

    pub fn placeholder_unregister_plugin(&self, plugin_id: &str) {
        let mut st = self.placeholders.lock().unwrap();
        st.resolvers.retain(|_, entry| entry.plugin_id != plugin_id);
    }

    /// 展开文本里的全部已注册占位符。未知占位符原样保留。
    pub fn placeholders_apply(&self, text: &str, player: Option<&PlayerInfo>) -> String {
        let mut out = text.to_string();
        if !out.contains('%') {
            return out;
        }
        loop {
            let Some(start) = out.find('%') else { break; };
            let Some(rest) = out.get(start + 1..) else { break; };
            let Some(rel) = rest.find('%') else { break; };
            let raw = &rest[..rel];
            let (key, arg) = match raw.split_once(' ') {
                Some((k, a)) => (k.trim(), a.trim()),
                None => (raw.trim(), ""),
            };
            let norm = normalize_placeholder_key(key);
            let resolved = {
                let st = self.placeholders.lock().unwrap();
                st.resolvers.get(&norm).and_then(|entry| {
                    entry.resolver.resolve(
                        player,
                        &PlaceholderContext { key: norm.clone(), value: arg.to_string() },
                    )
                })
            };
            let replacement = resolved.unwrap_or_else(|| "%".to_string() + raw + "%");
            out = format!("{}{}{}", &out[..start], replacement, &rest[rel + 1..]);
        }
        out
    }
}

fn normalize_placeholder_key(key: &str) -> String {
    key.trim().trim_matches('%').to_ascii_lowercase()
}

// ===========================================================================
// HTTP 客户端内核（reqwest；v4 语义：同步阻塞单请求）
// ===========================================================================

impl ActionHub {
    pub fn http_request(&self, request: &HttpRequest) -> Option<HttpResponse> {
        let timeout = std::time::Duration::from_millis(if request.timeout_ms == 0 {
            2000
        } else {
            request.timeout_ms.min(5000)
        });
        let method = match request.method.to_ascii_uppercase().as_str() {
            "POST" => reqwest::Method::POST,
            "PUT" => reqwest::Method::PUT,
            "DELETE" => reqwest::Method::DELETE,
            "PATCH" => reqwest::Method::PATCH,
            "HEAD" => reqwest::Method::HEAD,
            _ => reqwest::Method::GET,
        };
        let client = reqwest::Client::builder().timeout(timeout).build().ok()?;
        let mut wait = client.request(method, request.url.as_str());
        for h in &request.headers {
            wait = wait.header(h.name.as_str(), h.value.as_str());
        }
        if !request.body.is_empty() {
            wait = wait.body(request.body.clone());
        }
        let fut = async move {
            let resp = wait.send().await?;
            let status = resp.status().as_u16();
            let headers: Vec<HttpHeader> = resp
                .headers()
                .iter()
                .map(|(k, v)| HttpHeader {
                    name: k.as_str().to_string(),
                    value: String::from_utf8_lossy(v.as_bytes()).to_string(),
                })
                .collect();
            let body = resp.bytes().await.map(|b| b.to_vec())?;
            Ok(HttpResponse { status, headers, body, error: String::new() })
        };
        // 同步 API：事件处理器跑在 tokio worker 上时用 block_in_place 把当前
        // 任务挪开再阻塞等待；非 tokio 线程则走兜底后台 runtime。
        let result: Result<HttpResponse, reqwest::Error> = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| handle.block_on(fut))
        } else {
            fallback_runtime().block_on(fut)
        };
        Some(match result {
            Ok(resp) => resp,
            Err(e) => {
                log::warn!("http_request 失败: {e}");
                HttpResponse {
                    status: 0,
                    headers: Vec::new(),
                    body: Vec::new(),
                    error: e.to_string(),
                }
            }
        })
    }
}

/// 非 tokio 线程调用同步 HTTP 时的兜底 runtime（惰性单例）。
fn fallback_runtime() -> &'static tokio::runtime::Runtime {
    static RT: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("兜底 tokio runtime 创建失败")
    })
}

// ===========================================================================
// 路径查找内核（方块坐标 BFS，只走空气态；视觉/脚本寻路用）
// ===========================================================================

impl ActionHub {
    pub fn pathfinding_find(&self, query: &PathfindingQuery) -> Option<PathfindingResponse> {
        let air = self.air_state();
        let index = self.block_index()?;
        let max_nodes = if query.max_nodes == 0 { 4000 } else { query.max_nodes.min(50_000) };
        let start = (query.start.x, query.start.y, query.start.z);
        let goal = (query.goal.x, query.goal.y, query.goal.z);
        let _ = air;
        // 无真实方块数据源（qexed_world 并行开发中），当前只做几何直线
        // BFS 语义占位：起终点一致直接成功，否则报告未找到。
        if start == goal {
            return Some(PathfindingResponse {
                found: true,
                path: vec![query.start.clone(), query.goal.clone()],
            });
        }
        let _ = index;
        log::debug!(
            "pathfinding: {s} -> {g}（无方块数据源，返回未找到；max_nodes={m}）",
            s = format!("{:?}", start),
            g = format!("{:?}", goal),
            m = max_nodes
        );
        Some(PathfindingResponse { found: false, path: Vec::new() })
    }
}

// ===========================================================================
// 基岩版探测内核（v6 无 Geyser，Java 会话存在即 online，bedrock 恒 false）
// ===========================================================================

impl ActionHub {
    pub fn geyser_player_info(&self, query: &GeyserPlayerInfoQuery) -> GeyserPlayerInfoResponse {
        let mut resp = GeyserPlayerInfoResponse::default();
        let online = if query.username.is_empty() {
            false
        } else {
            self.online_player_names()
                .iter()
                .any(|n| n.eq_ignore_ascii_case(query.username.as_str()))
        };
        if online {
            resp.online = true;
            resp.username = query.username.clone();
        }
        resp
    }
}

// ===========================================================================
// 跨插件服务内核
// ===========================================================================

impl ActionHub {
    pub fn service_exists(&self, service: &str) -> bool {
        self.services
            .lock()
            .unwrap()
            .providers
            .contains_key(service)
    }

    pub fn service_call(&self, service: &str, method: &str, payload: &[u8]) -> Option<Vec<u8>> {
        let provider = {
            let st = self.services.lock().unwrap();
            st.providers.get(service).map(|e| e.provider.clone())?
        };
        // v6 in-process 调用没有稳定的 caller 身份可传，传空串；provider 自行
        // 需要来源时应改用带 plugin_id 的私有扩展方法。
        provider.call("", method, payload)
    }

    pub fn service_register(
        &self,
        plugin_id: &str,
        service: &str,
        provider: Arc<dyn PluginServiceProvider>,
    ) -> bool {
        if service.trim().is_empty() {
            return false;
        }
        let mut st = self.services.lock().unwrap();
        st.providers.insert(
            service.to_string(),
            ServiceEntry { plugin_id: plugin_id.to_string(), provider },
        );
        true
    }

    pub fn service_unregister_plugin(&self, plugin_id: &str) {
        let mut st = self.services.lock().unwrap();
        st.providers.retain(|_, entry| entry.plugin_id != plugin_id);
    }
}

// ===========================================================================
// PluginManager 委托实现（trait 逐字对齐 services.rs）
// ===========================================================================

use crate::manager::PluginManager;

impl PlayerActionApi for PluginManager {
    fn apply_actions(&self, username: &str, actions: &[PlayerAction]) -> usize {
        crate::action_hub().apply_player_actions(username, actions)
    }

    fn online_players(&self) -> Vec<String> {
        crate::action_hub().online_player_names()
    }
}

impl CommandApi for PluginManager {
    fn register_command(&self, plugin_id: &str, command: &str) {
        crate::action_hub().command_register(plugin_id, command);
    }

    fn plugin_commands(&self) -> Vec<String> {
        crate::action_hub().plugin_command_names()
    }
}

impl WorldEditApi for PluginManager {
    fn world_set_block(&self, dimension: &str, pos: (i32, i32, i32), block: &str) -> bool {
        crate::action_hub().world_set_block(dimension, pos, block)
    }

    fn world_set_blocks(&self, blocks: &[(&str, (i32, i32, i32), &str)]) -> usize {
        crate::action_hub().world_set_blocks(blocks)
    }

    fn world_break_block(&self, dimension: &str, pos: (i32, i32, i32)) -> bool {
        crate::action_hub().world_break_block(dimension, pos)
    }

    fn world_register_edit_region(&self, region: &WorldEditRegionSpec) -> bool {
        crate::action_hub().world_register_edit_region(region)
    }
}

impl EntityControlApi for PluginManager {
    fn entity_upsert(&self, entity: &RuntimeEntitySpec) -> bool {
        crate::action_hub().entity_upsert_inner(entity, "", "", "")
    }

    fn entity_upsert_with_events(
        &self,
        entity: &RuntimeEntitySpec,
        main_hand_event: &str,
        off_hand_event: &str,
        attack_event: &str,
    ) -> bool {
        crate::action_hub().entity_upsert_inner(entity, main_hand_event, off_hand_event, attack_event)
    }

    fn entity_move(&self, key: &str, _dimension: &str, position: NpcPosition) -> bool {
        // v6 单世界：dimension 仅记录不路由。
        crate::action_hub().entity_move_inner(key, position)
    }

    fn npc_patrol_step(
        &self,
        key: &str,
        _dimension: &str,
        current: NpcPosition,
        target: NpcPosition,
        speed: f64,
    ) -> bool {
        // 一步推进：朝 target 移动 min(speed, 距离)，到达后停在 target。
        let dx = target.x - current.x;
        let dy = target.y - current.y;
        let dz = target.z - current.z;
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
        let step = if dist <= speed || dist == 0.0 {
            dist
        } else {
            speed
        };
        let yaw = if dist == 0.0 { current.yaw } else { (dz.atan2(dx) as f32).to_degrees() - 90.0 };
        let next = NpcPosition {
            x: current.x + if dist == 0.0 { 0.0 } else { dx / dist * step },
            y: current.y + if dist == 0.0 { 0.0 } else { dy / dist * step },
            z: current.z + if dist == 0.0 { 0.0 } else { dz / dist * step },
            yaw,
            pitch: current.pitch,
        };
        crate::action_hub().entity_move_inner(key, next)
    }

    fn entity_remove(&self, key: &str) -> bool {
        crate::action_hub().entity_remove_inner(key)
    }
}

impl StructuredStorageApi for PluginManager {
    fn structured_storage_exists(&self, plugin_id: &str, key: &str) -> bool {
        crate::action_hub().storage_exists(plugin_id, key)
    }

    fn structured_storage_get(&self, plugin_id: &str, key: &str) -> Option<Vec<u8>> {
        crate::action_hub().storage_get(plugin_id, key)
    }

    fn structured_storage_set(&self, plugin_id: &str, key: &str, value: &[u8]) -> bool {
        crate::action_hub().storage_set(plugin_id, key, value)
    }

    fn structured_storage_delete(&self, plugin_id: &str, key: &str) -> bool {
        crate::action_hub().storage_delete(plugin_id, key)
    }
}

impl EconomyApi for PluginManager {
    fn economy_register_currency(
        &self,
        plugin_id: &str,
        id: &str,
        name: &str,
        symbol: &str,
        fractional_digits: i32,
    ) -> bool {
        crate::action_hub().economy_register_currency(plugin_id, id, name, symbol, fractional_digits)
    }

    fn economy_currency_info(&self, currency: &str) -> Option<CurrencyInfo> {
        crate::action_hub().economy_currency_info(currency)
    }

    fn economy_storage(&self, currency: &str) -> Option<String> {
        crate::action_hub().economy_storage(currency)
    }

    fn economy_balance(&self, player: &str, currency: &str) -> Option<i64> {
        crate::action_hub().economy_balance(player, currency)
    }

    fn economy_set_balance(&self, player: &str, currency: &str, amount: i64) -> Option<i64> {
        crate::action_hub().economy_set_balance(player, currency, amount)
    }

    fn economy_deposit(&self, player: &str, currency: &str, amount: i64) -> Option<i64> {
        crate::action_hub().economy_deposit(player, currency, amount)
    }

    fn economy_withdraw(&self, player: &str, currency: &str, amount: i64) -> Option<i64> {
        crate::action_hub().economy_withdraw(player, currency, amount)
    }
}

impl RandomPoolApi for PluginManager {
    fn random_pool_roll(
        &self,
        kind: &str,
        pool_id: &str,
        entries: &[(&str, u64)],
        precompute_count: usize,
    ) -> Option<String> {
        crate::action_hub().random_pool_roll(kind, pool_id, entries, precompute_count)
    }
}

impl PlaceholderApi for PluginManager {
    fn placeholder_register(
        &self,
        plugin_id: &str,
        key: &str,
        resolver: std::sync::Arc<dyn PlaceholderResolver>,
    ) -> bool {
        crate::action_hub().placeholder_register(plugin_id, key, resolver)
    }

    fn placeholder_unregister_plugin(&self, plugin_id: &str) {
        crate::action_hub().placeholder_unregister_plugin(plugin_id)
    }

    fn placeholders_apply(&self, text: &str, player: Option<&PlayerInfo>) -> String {
        crate::action_hub().placeholders_apply(text, player)
    }
}

impl HttpClientApi for PluginManager {
    fn http_request(&self, request: &HttpRequest) -> Option<HttpResponse> {
        crate::action_hub().http_request(request)
    }
}

impl PathfindingApi for PluginManager {
    fn pathfinding_find(&self, query: &PathfindingQuery) -> Option<PathfindingResponse> {
        crate::action_hub().pathfinding_find(query)
    }
}

impl GeyserApi for PluginManager {
    fn geyser_player_info(&self, query: &GeyserPlayerInfoQuery) -> GeyserPlayerInfoResponse {
        crate::action_hub().geyser_player_info(query)
    }
}

impl InterPluginApi for PluginManager {
    fn plugin_service_exists(&self, service: &str) -> bool {
        crate::action_hub().service_exists(service)
    }

    fn plugin_call(&self, service: &str, method: &str, payload: &[u8]) -> Option<Vec<u8>> {
        crate::action_hub().service_call(service, method, payload)
    }

    fn plugin_register_service(
        &self,
        plugin_id: &str,
        service: &str,
        provider: std::sync::Arc<dyn PluginServiceProvider>,
    ) -> bool {
        crate::action_hub().service_register(plugin_id, service, provider)
    }

    fn plugin_unregister_plugin(&self, plugin_id: &str) {
        crate::action_hub().service_unregister_plugin(plugin_id);
        crate::action_hub().placeholder_unregister_plugin(plugin_id);
    }
}

// ===========================================================================
// 实体 AI tick 查询与应用（qexed_play 按节奏驱动）
// ===========================================================================

impl ActionHub {
    /// 配置了 AI 的运行时实体 key 列表。
    pub fn ai_entity_keys(&self) -> Vec<String> {
        let reg = self.entities.lock().unwrap();
        reg.by_key
            .iter()
            .filter(|(_, e)| !e.spec.ai.is_empty())
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// 组装单个实体的 AI tick 查询。实体不存在或无 AI 时返回 None。
    pub fn ai_tick_query(
        &self,
        key: &str,
        tick_ms: u64,
        nearby: &[EntityAiPlayerPayload],
    ) -> Option<EntityAiTickQuery> {
        let reg = self.entities.lock().unwrap();
        let e = reg.by_key.get(key)?;
        if e.spec.ai.is_empty() {
            return None;
        }
        let ai_params: std::collections::BTreeMap<String, serde_json::Value> = if e.spec.ai_params_json.is_empty() {
            Default::default()
        } else {
            serde_json::from_str(&e.spec.ai_params_json).unwrap_or_default()
        };
        Some(EntityAiTickQuery {
            entity: EntityAiEntityPayload {
                key: e.spec.key.clone(),
                entity_id: e.entity_id,
                entity_type: e.spec.entity_type.clone(),
                custom_type: String::new(),
                ai: e.spec.ai.clone(),
                spawn_rule: String::new(),
                ai_params,
                dimension: e.spec.dimension.clone(),
                position: PlayerPositionPayload::from_parts(
                    e.spec.x, e.spec.y, e.spec.z, e.spec.yaw, e.spec.pitch, true
                ),
            },
            nearby_players: nearby.to_vec(),
            tick_ms,
        })
    }

    /// 应用插件返回的实体 AI 操作，返回下发的帧数。
    pub fn apply_ai_operations(&self, key: &str, ops: &[EntityAiOperation]) -> usize {
        let mut frames = 0usize;
        for op in ops {
            match op {
                EntityAiOperation::MoveDelta { x, y, z, yaw, pitch } => {
                    let Some(next) = (|| {
                        let mut reg = self.entities.lock().unwrap();
                        let e = reg.by_key.get_mut(key)?;
                        e.spec.x += x;
                        e.spec.y += y;
                        e.spec.z += z;
                        if let Some(yaw) = yaw {
                            e.spec.yaw = *yaw;
                        }
                        if let Some(pitch) = pitch {
                            e.spec.pitch = *pitch;
                        }
                        Some((e.entity_id, e.spec.x, e.spec.y, e.spec.z, e.spec.yaw, e.spec.pitch))
                    })() else { continue; };
                    let (entity_id, nx, ny, nz, nyaw, npitch) = next;
                    let position = add_entity_pk::EntityPosition {
                        x: nx,
                        y: ny,
                        z: nz,
                        yaw: nyaw,
                        pitch: npitch,
                        on_ground: true,
                    };
                    if let Some(f) = Self::encode(add_entity_pk::TeleportEntity::from_position(entity_id, position)) {
                        self.broadcast(&f);
                        frames += 1;
                    }
                    if let Some(f) = Self::encode(add_entity_pk::RotateHead::new(entity_id, nyaw)) {
                        self.broadcast(&f);
                        frames += 1;
                    }
                }
                EntityAiOperation::LookAt { x, y, z } => {
                    let Some(next) = (|| {
                        let mut reg = self.entities.lock().unwrap();
                        let e = reg.by_key.get_mut(key)?;
                        let dx = x - e.spec.x;
                        let dy = y - (e.spec.y + 1.62); // 眼高近似
                        let dz = z - e.spec.z;
                        let h = (dx * dx + dz * dz).sqrt();
                        if h > 1e-6 {
                            e.spec.yaw = (dz.atan2(dx) as f32).to_degrees() - 90.0;
                            e.spec.pitch = ((-dy).atan2(h) as f32).to_degrees();
                        }
                        Some((e.entity_id, e.spec.x, e.spec.y, e.spec.z, e.spec.yaw, e.spec.pitch))
                    })() else { continue; };
                    let (entity_id, nx, ny, nz, nyaw, npitch) = next;
                    let position = add_entity_pk::EntityPosition {
                        x: nx,
                        y: ny,
                        z: nz,
                        yaw: nyaw,
                        pitch: npitch,
                        on_ground: true,
                    };
                    if let Some(f) = Self::encode(add_entity_pk::TeleportEntity::from_position(entity_id, position)) {
                        self.broadcast(&f);
                        frames += 1;
                    }
                    if let Some(f) = Self::encode(add_entity_pk::RotateHead::new(entity_id, nyaw)) {
                        self.broadcast(&f);
                        frames += 1;
                    }
                }
                EntityAiOperation::Remove => {
                    if self.entity_remove_inner(key) {
                        frames += 1;
                    }
                }
            }
        }
        frames
    }
}
