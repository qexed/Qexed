//! 注册表驱动的物品/背包实现（v4 inventory 的 v6 组装层直连）。

use bytes::Bytes;
use qexed_play::ItemRegistry;
use qexed_player::player_data::{StoredEquipment, StoredInventory, StoredSlot};
use qexed_protocol::to_client::play::set_equipment::EquipmentEntry;
use qexed_protocol::types::Slot;

/// 注册表物品源（启动时一次性加载）。
pub struct RealItems {
    item_ids: std::collections::HashMap<String, i32>,
    air_states: std::collections::HashSet<i32>,
    /// 方块状态 id -> 掉落物 id（loot_table/blocks 最小解析）。
    block_drop_items: std::collections::HashMap<i32, i32>,
}

impl RealItems {
    /// 从 workspace 的 assets/reports + mojang 数据缓存加载（缺失时回退空表）。
    pub fn load() -> Self {
        let item_ids = load_item_ids();
        let air_states = load_air_states();
        let block_drop_items = load_block_drop_items(&item_ids);
        if item_ids.is_empty() {
            log::warn!("item registry empty; item lookups will miss");
        }
        if block_drop_items.is_empty() {
            log::warn!("block loot table mapping empty; block drops will miss");
        } else {
            log::info!(
                "block loot table mapping loaded: {} block states",
                block_drop_items.len()
            );
        }
        Self { item_ids, air_states, block_drop_items }
    }
}

fn workspace_reports() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_default()
        .join("assets/reports")
}

/// Mojang 数据缓存根（run/cache/mojang/<version>/data/minecraft）；
/// 与 qexed_mojang_data::registry_sync 的 data_roots 保持一致（cwd 相对）。
fn mojang_data_root() -> std::path::PathBuf {
    std::path::Path::new(".")
        .join("cache/mojang")
        .join(qexed_config::MC_VERSION)
        .join("data/minecraft")
}

fn load_item_ids() -> std::collections::HashMap<String, i32> {
    let path = workspace_reports().join("registries.json");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return std::collections::HashMap::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return std::collections::HashMap::new();
    };
    let mut map = std::collections::HashMap::new();
    if let Some(entries) = value.get("minecraft:item").and_then(|v| v.get("entries")).and_then(|v| v.as_object()) {
        for (name, entry) in entries {
            if let Some(id) = entry.get("protocol_id").and_then(|v| v.as_i64()) {
                map.insert(name.clone(), id as i32);
            }
        }
    }
    map
}

fn load_air_states() -> std::collections::HashSet<i32> {
    let path = workspace_reports().join("blocks.json");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return [0].into_iter().collect();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return [0].into_iter().collect();
    };
    let mut set = std::collections::HashSet::new();
    set.insert(0);
    for block in ["minecraft:air", "minecraft:cave_air", "minecraft:void_air"] {
        if let Some(states) = value.get(block).and_then(|v| v.get("states")).and_then(|v| v.as_array()) {
            for state in states {
                if let Some(id) = state.get("id").and_then(|v| v.as_i64()) {
                    set.insert(id as i32);
                }
            }
        }
    }
    set
}

/// 方块名（minecraft:stone）-> 该方块全部状态 id（blocks.json 的 states[].id）。
fn load_block_states_by_name() -> std::collections::HashMap<String, Vec<i32>> {
    let path = workspace_reports().join("blocks.json");
    let mut map = std::collections::HashMap::new();
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return map;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return map;
    };
    let Some(blocks) = value.as_object() else {
        return map;
    };
    for (name, block) in blocks {
        let Some(states) = block.get("states").and_then(|v| v.as_array()) else {
            continue;
        };
        let ids = states
            .iter()
            .filter_map(|state| {
                state
                    .get("id")
                    .and_then(serde_json::Value::as_i64)
                    .and_then(|id| i32::try_from(id).ok())
            })
            .collect::<Vec<_>>();
        if !ids.is_empty() {
            map.insert(name.clone(), ids);
        }
    }
    map
}

/// 方块状态 -> 掉落物品 id 映射（loot_table/blocks 的最小实现）。
///
/// 语义（vanilla 无工具挖掘的最简近似）：
/// - 方块名 -> loot_table/blocks/<name>.json；
/// - 逐 pool（按声明顺序）DFS 找第一个"无条件"的 minecraft:item 条目：
///   带 silk_touch/shears（tool/*）条件的条目与整 pool 为 tool 条件的池
///   视为工具分支跳过；其余条件（survives_explosion/table_bonus/match_block）
///   视为可通过，其条目按声明顺序进回退位；
/// - 无无条件条目时取回退位（如 stone -> cobblestone、grass_block -> dirt）；
/// - 全池皆 tool 条件（glass/ice/blue_ice 等 silk 专属掉落）则不掉落；
/// - 无任何 item 条目（spawner/vault 等）则不掉落；
/// - 输出物品名经 registries.json 转协议 id；
/// - 已知精度限制：snow 等按方块状态分层的表映射为单一物品（取 DFS 首个
///   无条件条目 snow_block），gravel 等概率表取多数分支（gravel 而非 10% flint）。
fn load_block_drop_items(item_ids: &std::collections::HashMap<String, i32>) -> std::collections::HashMap<i32, i32> {
    let mut drops = std::collections::HashMap::new();
    let blocks = load_block_states_by_name();
    let loot_dir = mojang_data_root().join("loot_table/blocks");
    let Ok(entries) = std::fs::read_dir(&loot_dir) else {
        log::warn!("block loot table directory missing: {}", loot_dir.display());
        return drops;
    };

    let mut parsed = 0usize;
    let mut resolved = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let block_name = format!("minecraft:{stem}");
        let Some(states) = blocks.get(&block_name) else {
            continue;
        };
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
            log::debug!("block loot table parse failed: {}", path.display());
            continue;
        };
        parsed += 1;
        let Some(item_name) = first_drop_item(&value) else {
            continue;
        };
        let Some(&item_id) = item_ids.get(&item_name) else {
            log::debug!("block loot drop item not in registry: {block_name} -> {item_name}");
            continue;
        };
        resolved += 1;
        for state in states {
            drops.insert(*state, item_id);
        }
    }
    log::debug!("block loot tables: parsed={parsed} resolved={resolved} states={}", drops.len());
    drops
}

/// 掉落表 JSON -> 第一个掉落物品名（最小语义见 load_block_drop_items 文档）。
fn first_drop_item(table: &serde_json::Value) -> Option<String> {
    let pools = table.get("pools")?.as_array()?;
    let mut fallback: Option<String> = None;
    for pool in pools {
        // 整池 tool 条件（silk 专属掉落，如 glass/ice）贡献为空。
        if pool.get("condition").is_some_and(is_tool_condition) {
            continue;
        }
        let mut pool_fallback: Option<String> = None;
        let entries = pool.get("entries").and_then(|v| v.as_array());
        for entry in entries.into_iter().flatten() {
            if let Some(name) = pick_item(entry, &mut pool_fallback) {
                return Some(name);
            }
        }
        if fallback.is_none() {
            fallback = pool_fallback;
        }
    }
    fallback
}

/// 条目（或其 children，按声明顺序）里的第一个无工具条件掉落。
/// 无条件命中返回 Some；非工具条件条目按序写入 fallback。
fn pick_item(entry: &serde_json::Value, fallback: &mut Option<String>) -> Option<String> {
    match entry.get("type").and_then(|t| t.as_str()) {
        Some("minecraft:item") => {
            let name = entry.get("name").and_then(|n| n.as_str());
            match entry.get("condition") {
                None => name.map(|n| n.to_string()),
                Some(condition) => {
                    // 工具分支（silk/shears）不参与普通掉落；
                    // 其余条件（survives_explosion/table_bonus/match_block）
                    // 视为可通过，按声明顺序占回退位。
                    if !is_tool_condition(condition) && fallback.is_none() {
                        *fallback = name.map(|n| n.to_string());
                    }
                    None
                }
            }
        }
        // alternatives/group 等组合条目：按声明顺序深入 children。
        _ => {
            let children = entry.get("children").and_then(|v| v.as_array());
            for child in children.into_iter().flatten() {
                if let Some(name) = pick_item(child, fallback) {
                    return Some(name);
                }
            }
            None
        }
    }
}

/// 条件是否为工具条件（minecraft:tool/*，含 any_of/all_of/inverted 嵌套）。
/// 工具条件分支是 silk_touch/shears 专属掉落，普通挖掘不可达。
fn is_tool_condition(condition: &serde_json::Value) -> bool {
    match condition {
        serde_json::Value::String(value) => value.starts_with("minecraft:tool/"),
        serde_json::Value::Object(_) => {
            let kind = condition.get("type").and_then(|t| t.as_str()).unwrap_or("");
            match kind {
                "minecraft:any_of" | "minecraft:all_of" => condition
                    .get("terms")
                    .and_then(|v| v.as_array())
                    .is_some_and(|terms| terms.iter().any(is_tool_condition)),
                "minecraft:inverted" => condition
                    .get("term")
                    .is_some_and(is_tool_condition),
                _ => false,
            }
        }
        _ => false,
    }
}

impl ItemRegistry for RealItems {
    fn is_air_block_state(&self, block_state: i32) -> bool {
        self.air_states.contains(&block_state)
    }

    fn air_block_state(&self) -> i32 {
        0
    }

    fn picked_item_for_block_state(&self, block_state: i32) -> Option<i32> {
        self.block_drop_items.get(&block_state).copied()
    }

    fn item_id_for_name(&self, name: &str) -> Option<i32> {
        let key = if name.contains(':') { name.to_string() } else { format!("minecraft:{name}") };
        self.item_ids.get(&key).copied()
    }

    fn simple_item(&self, item_id: i32, count: i32) -> Slot {
        Slot {
            item_count: qexed_packet::net_types::VarInt(count),
            item_id: Some(qexed_packet::net_types::VarInt(item_id)),
            number_of_components_to_add: None,
            number_of_components_to_remove: None,
            components_to_add: None,
            components_to_remove: None,
        }
    }

    fn empty_slot(&self) -> Slot {
        Slot {
            item_count: qexed_packet::net_types::VarInt(0),
            item_id: None,
            number_of_components_to_add: None,
            number_of_components_to_remove: None,
            components_to_add: None,
            components_to_remove: None,
        }
    }
}

/// 存档背包驱动。
pub struct RealInventory {
    pub stored: StoredInventory,
}

impl RealInventory {
    pub fn new(stored: StoredInventory) -> Self {
        Self { stored }
    }
}

/// 快捷栏槽位数（与 qexed_play::inventory 的 HOTBAR_SIZE 同值）。
const HOTBAR_SIZE: usize = 9;

/// 装备槽位协议编号（SetEquipment 的 slot 字节；与
/// qexed_play::inventory::Equipment 常量一致：0 主手/1 副手/2 脚/3 腿/4 胸/5 头/6 body）。
fn equipment_slot(entry: &StoredEquipment) -> Option<u8> {
    match entry.slot {
        0 => Some(0),
        1 => Some(1),
        2 => Some(2),
        3 => Some(3),
        4 => Some(4),
        5 => Some(5),
        _ => None,
    }
}

impl qexed_play::SessionInventory for RealInventory {
    fn selected_slot(&self) -> usize {
        self.stored.selected
    }

    fn set_player_inventory_packets(&self) -> Vec<Bytes> {
        // SetPlayerInventory（0x6e）：slot VarInt + Slot 内容。
        // 槽位布局与 v4 一致：0..8 快捷栏，9..35 主背包。
        let mut packets = Vec::with_capacity(self.stored.hotbar.len() + self.stored.main.len());
        let entries = self
            .stored
            .hotbar
            .iter()
            .enumerate()
            .map(|(slot, item)| (slot, item))
            .chain(
                self.stored
                    .main
                    .iter()
                    .enumerate()
                    .map(|(slot, item)| (HOTBAR_SIZE + slot, item)),
            );
        for (slot, item) in entries {
            let packet = qexed_protocol::to_client::play::set_player_inventory::SetPlayerInventory {
                slot: qexed_packet::net_types::VarInt(slot as i32),
                contents: decode_stored_slot(item),
            };
            match qexed_player::packet_bytes(packet) {
                Ok(bytes) => packets.push(bytes),
                Err(err) => log::warn!("encode SetPlayerInventory(slot={slot}) failed: {err}"),
            }
        }
        packets
    }

    fn visible_equipment(&self) -> Vec<EquipmentEntry> {
        self.stored
            .equipment
            .iter()
            .filter_map(|entry| {
                let slot = equipment_slot(entry)?;
                let item = decode_stored_slot(&entry.item);
                (item.item_count.0 > 0).then_some(EquipmentEntry {
                    slot: i32::from(slot),
                    item,
                })
            })
            .collect()
    }

    fn equipment_packet(
        &self,
        entity_id: i32,
        equipment: Vec<EquipmentEntry>,
    ) -> qexed_play::Result<Option<qexed_protocol::to_client::play::set_equipment::SetEquipment>> {
        if equipment.is_empty() {
            return Ok(None);
        }
        Ok(Some(qexed_protocol::to_client::play::set_equipment::SetEquipment {
            entity: qexed_packet::net_types::VarInt(entity_id),
            slots: equipment,
        }))
    }
}

/// StoredSlot 解码为协议槽（packet_data 是 base64 槽字节）。
pub fn decode_stored_slot(slot: &StoredSlot) -> Slot {
    use base64::Engine as _;
    if slot.packet_data.is_empty() {
        // 旧存档（无 packet_data）：按 item_count/item_id 字段构造。
        return legacy_stored_slot(slot);
    }
    let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&slot.packet_data) else {
        return Slot::default();
    };
    let mut buf = bytes::BytesMut::from(bytes.as_slice());
    let mut reader = qexed_packet::PacketReader::new(&mut buf);
    let mut parsed = Slot::default();
    use qexed_packet::PacketCodec as _;
    let _ = parsed.deserialize(&mut reader);
    parsed
}

/// 旧格式存档槽（qexed_player::player_data::model::legacy_slot 同语义）。
fn legacy_stored_slot(slot: &StoredSlot) -> Slot {
    let count = slot.item_count.unwrap_or_default();
    let Some(item_id) = slot.item_id else {
        return empty_protocol_slot();
    };
    Slot {
        item_count: qexed_packet::net_types::VarInt(count),
        item_id: Some(qexed_packet::net_types::VarInt(item_id)),
        number_of_components_to_add: None,
        number_of_components_to_remove: None,
        components_to_add: None,
        components_to_remove: None,
    }
}

fn empty_protocol_slot() -> Slot {
    Slot {
        item_count: qexed_packet::net_types::VarInt(0),
        item_id: None,
        number_of_components_to_add: None,
        number_of_components_to_remove: None,
        components_to_add: None,
        components_to_remove: None,
    }
}