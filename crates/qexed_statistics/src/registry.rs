//! 26.3 统计注册表数据（custom_stat 全量 78 键 + stat_type 映射）。
//!
//! 数据源：官方 datagen registries.json（26.3）。

/// 统计类型 id（AwardStats 的 category 编码）。
pub mod stat_type {
    pub const MINED: i32 = 0;
    pub const CRAFTED: i32 = 1;
    pub const USED: i32 = 2;
    pub const BROKEN: i32 = 3;
    pub const DROPPED: i32 = 4;
    pub const PICKED_UP: i32 = 5;
    pub const KILLED: i32 = 6;
    pub const KILLED_BY: i32 = 7;
    pub const CUSTOM: i32 = 8;
}

pub use stat_type as StatTypes;

/// custom_stat 注册表全量键（按 protocol_id 排序）。
pub const ALL_CUSTOM: &[&str] = &[
    "minecraft:leave_game", "minecraft:play_time", "minecraft:total_world_time", "minecraft:time_since_death", "minecraft:time_since_rest", "minecraft:sneak_time", "minecraft:walk_one_cm", "minecraft:crouch_one_cm", "minecraft:sprint_one_cm", "minecraft:walk_on_water_one_cm", "minecraft:fall_one_cm", "minecraft:climb_one_cm", "minecraft:fly_one_cm", "minecraft:walk_under_water_one_cm", "minecraft:minecart_one_cm", "minecraft:boat_one_cm", "minecraft:pig_one_cm", "minecraft:happy_ghast_one_cm", "minecraft:horse_one_cm", "minecraft:aviate_one_cm", "minecraft:swim_one_cm", "minecraft:strider_one_cm", "minecraft:nautilus_one_cm", "minecraft:jump", "minecraft:drop", "minecraft:damage_dealt", "minecraft:damage_dealt_absorbed", "minecraft:damage_dealt_resisted", "minecraft:damage_taken", "minecraft:damage_blocked_by_shield", "minecraft:damage_absorbed", "minecraft:damage_resisted", "minecraft:deaths", "minecraft:mob_kills", "minecraft:animals_bred", "minecraft:player_kills", "minecraft:fish_caught", "minecraft:talked_to_villager", "minecraft:traded_with_villager", "minecraft:eat_cake_slice", "minecraft:fill_cauldron", "minecraft:use_cauldron", "minecraft:clean_armor", "minecraft:clean_banner", "minecraft:clean_shulker_box", "minecraft:interact_with_brewingstand", "minecraft:interact_with_beacon", "minecraft:inspect_dropper", "minecraft:inspect_hopper", "minecraft:inspect_dispenser", "minecraft:play_noteblock", "minecraft:tune_noteblock", "minecraft:pot_flower", "minecraft:trigger_trapped_chest", "minecraft:open_enderchest", "minecraft:enchant_item", "minecraft:play_record", "minecraft:interact_with_furnace", "minecraft:interact_with_crafting_table", "minecraft:open_chest", "minecraft:sleep_in_bed", "minecraft:sleep_in_straw_bed", "minecraft:open_shulker_box", "minecraft:open_barrel", "minecraft:interact_with_blast_furnace", "minecraft:interact_with_smoker", "minecraft:interact_with_lectern", "minecraft:interact_with_campfire", "minecraft:interact_with_cartography_table", "minecraft:interact_with_loom", "minecraft:interact_with_stonecutter", "minecraft:bell_ring", "minecraft:raid_trigger", "minecraft:raid_win", "minecraft:interact_with_anvil", "minecraft:interact_with_grindstone", "minecraft:target_hit", "minecraft:interact_with_smithing_table"
];

// 便捷常量（高频使用的统计键）
pub const CUSTOM_LEAVE_GAME: &str = "minecraft:leave_game";
pub const CUSTOM_PLAY_TIME: &str = "minecraft:play_time";
pub const CUSTOM_TOTAL_WORLD_TIME: &str = "minecraft:total_world_time";
pub const CUSTOM_TIME_SINCE_DEATH: &str = "minecraft:time_since_death";
pub const CUSTOM_TIME_SINCE_REST: &str = "minecraft:time_since_rest";
pub const CUSTOM_SNEAK_TIME: &str = "minecraft:sneak_time";
pub const CUSTOM_WALK_ONE_CM: &str = "minecraft:walk_one_cm";
pub const CUSTOM_CROUCH_ONE_CM: &str = "minecraft:crouch_one_cm";
pub const CUSTOM_SPRINT_ONE_CM: &str = "minecraft:sprint_one_cm";
pub const CUSTOM_WALK_ON_WATER_ONE_CM: &str = "minecraft:walk_on_water_one_cm";
pub const CUSTOM_FALL_ONE_CM: &str = "minecraft:fall_one_cm";
pub const CUSTOM_CLIMB_ONE_CM: &str = "minecraft:climb_one_cm";
pub const CUSTOM_FLY_ONE_CM: &str = "minecraft:fly_one_cm";
pub const CUSTOM_WALK_UNDER_WATER_ONE_CM: &str = "minecraft:walk_under_water_one_cm";
pub const CUSTOM_MINECART_ONE_CM: &str = "minecraft:minecart_one_cm";
pub const CUSTOM_BOAT_ONE_CM: &str = "minecraft:boat_one_cm";
pub const CUSTOM_PIG_ONE_CM: &str = "minecraft:pig_one_cm";
pub const CUSTOM_HAPPY_GHAST_ONE_CM: &str = "minecraft:happy_ghast_one_cm";
pub const CUSTOM_HORSE_ONE_CM: &str = "minecraft:horse_one_cm";
pub const CUSTOM_AVIATE_ONE_CM: &str = "minecraft:aviate_one_cm";
pub const CUSTOM_SWIM_ONE_CM: &str = "minecraft:swim_one_cm";
pub const CUSTOM_STRIDER_ONE_CM: &str = "minecraft:strider_one_cm";
pub const CUSTOM_NAUTILUS_ONE_CM: &str = "minecraft:nautilus_one_cm";
pub const CUSTOM_JUMP: &str = "minecraft:jump";
pub const CUSTOM_DROP: &str = "minecraft:drop";
pub const CUSTOM_DAMAGE_DEALT: &str = "minecraft:damage_dealt";
pub const CUSTOM_DAMAGE_DEALT_ABSORBED: &str = "minecraft:damage_dealt_absorbed";
pub const CUSTOM_DAMAGE_DEALT_RESISTED: &str = "minecraft:damage_dealt_resisted";
pub const CUSTOM_DAMAGE_TAKEN: &str = "minecraft:damage_taken";
pub const CUSTOM_DAMAGE_BLOCKED_BY_SHIELD: &str = "minecraft:damage_blocked_by_shield";
pub const CUSTOM_DAMAGE_ABSORBED: &str = "minecraft:damage_absorbed";
pub const CUSTOM_DAMAGE_RESISTED: &str = "minecraft:damage_resisted";
pub const CUSTOM_DEATHS: &str = "minecraft:deaths";
pub const CUSTOM_MOB_KILLS: &str = "minecraft:mob_kills";
pub const CUSTOM_ANIMALS_BRED: &str = "minecraft:animals_bred";
pub const CUSTOM_PLAYER_KILLS: &str = "minecraft:player_kills";
pub const CUSTOM_FISH_CAUGHT: &str = "minecraft:fish_caught";
pub const CUSTOM_TALKED_TO_VILLAGER: &str = "minecraft:talked_to_villager";
pub const CUSTOM_TRADED_WITH_VILLAGER: &str = "minecraft:traded_with_villager";
pub const CUSTOM_EAT_CAKE_SLICE: &str = "minecraft:eat_cake_slice";
pub const CUSTOM_FILL_CAULDRON: &str = "minecraft:fill_cauldron";
pub const CUSTOM_USE_CAULDRON: &str = "minecraft:use_cauldron";
pub const CUSTOM_CLEAN_ARMOR: &str = "minecraft:clean_armor";
pub const CUSTOM_CLEAN_BANNER: &str = "minecraft:clean_banner";
pub const CUSTOM_CLEAN_SHULKER_BOX: &str = "minecraft:clean_shulker_box";
pub const CUSTOM_INTERACT_WITH_BREWINGSTAND: &str = "minecraft:interact_with_brewingstand";
pub const CUSTOM_INTERACT_WITH_BEACON: &str = "minecraft:interact_with_beacon";
pub const CUSTOM_INSPECT_DROPPER: &str = "minecraft:inspect_dropper";
pub const CUSTOM_INSPECT_HOPPER: &str = "minecraft:inspect_hopper";
pub const CUSTOM_INSPECT_DISPENSER: &str = "minecraft:inspect_dispenser";
pub const CUSTOM_PLAY_NOTEBLOCK: &str = "minecraft:play_noteblock";
pub const CUSTOM_TUNE_NOTEBLOCK: &str = "minecraft:tune_noteblock";
pub const CUSTOM_POT_FLOWER: &str = "minecraft:pot_flower";
pub const CUSTOM_TRIGGER_TRAPPED_CHEST: &str = "minecraft:trigger_trapped_chest";
pub const CUSTOM_OPEN_ENDERCHEST: &str = "minecraft:open_enderchest";
pub const CUSTOM_ENCHANT_ITEM: &str = "minecraft:enchant_item";
pub const CUSTOM_PLAY_RECORD: &str = "minecraft:play_record";
pub const CUSTOM_INTERACT_WITH_FURNACE: &str = "minecraft:interact_with_furnace";
pub const CUSTOM_INTERACT_WITH_CRAFTING_TABLE: &str = "minecraft:interact_with_crafting_table";
pub const CUSTOM_OPEN_CHEST: &str = "minecraft:open_chest";
pub const CUSTOM_SLEEP_IN_BED: &str = "minecraft:sleep_in_bed";
pub const CUSTOM_SLEEP_IN_STRAW_BED: &str = "minecraft:sleep_in_straw_bed";
pub const CUSTOM_OPEN_SHULKER_BOX: &str = "minecraft:open_shulker_box";
pub const CUSTOM_OPEN_BARREL: &str = "minecraft:open_barrel";
pub const CUSTOM_INTERACT_WITH_BLAST_FURNACE: &str = "minecraft:interact_with_blast_furnace";
pub const CUSTOM_INTERACT_WITH_SMOKER: &str = "minecraft:interact_with_smoker";
pub const CUSTOM_INTERACT_WITH_LECTERN: &str = "minecraft:interact_with_lectern";
pub const CUSTOM_INTERACT_WITH_CAMPFIRE: &str = "minecraft:interact_with_campfire";
pub const CUSTOM_INTERACT_WITH_CARTOGRAPHY_TABLE: &str = "minecraft:interact_with_cartography_table";
pub const CUSTOM_INTERACT_WITH_LOOM: &str = "minecraft:interact_with_loom";
pub const CUSTOM_INTERACT_WITH_STONECUTTER: &str = "minecraft:interact_with_stonecutter";
pub const CUSTOM_BELL_RING: &str = "minecraft:bell_ring";
pub const CUSTOM_RAID_TRIGGER: &str = "minecraft:raid_trigger";
pub const CUSTOM_RAID_WIN: &str = "minecraft:raid_win";
pub const CUSTOM_INTERACT_WITH_ANVIL: &str = "minecraft:interact_with_anvil";
pub const CUSTOM_INTERACT_WITH_GRINDSTONE: &str = "minecraft:interact_with_grindstone";
pub const CUSTOM_TARGET_HIT: &str = "minecraft:target_hit";
pub const CUSTOM_INTERACT_WITH_SMITHING_TABLE: &str = "minecraft:interact_with_smithing_table";

/// 条目 id（方块/物品/实体）→ 注册表 protocol id。
/// 从 assets/reports/registries.json 的对应注册表查询；
/// 未找到返回 None（调用方决定回退策略）。
pub fn entry_protocol_id(registry: &str, entry: &str) -> Option<i32> {
    // 运行时从磁盘注册表懒加载（静态缓存）。
    use std::sync::OnceLock;
    static CACHE: OnceLock<std::collections::HashMap<String, std::collections::HashMap<String, i32>>> =
        OnceLock::new();
    let cache = CACHE.get_or_init(load_all);
    cache
        .get(registry)
        .and_then(|entries| entries.get(entry).copied())
}

/// custom 统计键 → custom_stat 注册表 id。
pub fn custom_stat_id(key: &str) -> Option<i32> {
    entry_protocol_id("minecraft:custom_stat", key)
}

fn load_all() -> std::collections::HashMap<String, std::collections::HashMap<String, i32>> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_default()
        .join("assets/reports/registries.json");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        log::warn!("statistics registry report missing: {}", path.display());
        return std::collections::HashMap::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return std::collections::HashMap::new();
    };
    let mut all = std::collections::HashMap::new();
    if let Some(registries) = value.as_object() {
        for (registry, body) in registries {
            let Some(entries) = body.get("entries").and_then(|v| v.as_object()) else {
                continue;
            };
            let map: std::collections::HashMap<String, i32> = entries
                .iter()
                .filter_map(|(name, entry)| {
                    entry
                        .get("protocol_id")
                        .and_then(|v| v.as_i64())
                        .and_then(|id| i32::try_from(id).ok())
                        .map(|id| (name.clone(), id))
                })
                .collect();
            all.insert(registry.clone(), map);
        }
    }
    all
}
