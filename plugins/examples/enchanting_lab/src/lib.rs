use qexed_plugin_sdk::{
    EnchantingOption, EnchantingQuery, EnchantingResponse, WorldEditRegion,
    world_register_edit_region, world_set_blocks,
};

qexed_plugin_sdk::qexed_plugin_memory!();

const DIMENSION: &str = "minecraft:overworld";

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_priority() -> i32 {
    250
}

#[unsafe(no_mangle)]
pub extern "C" fn qexed_plugin_init() {
    let _ = world_register_edit_region(&WorldEditRegion {
        id: "enchanting_lab",
        dimension: DIMENSION,
        min: (-6, -64, -6),
        max: (6, -58, 6),
        allow_player_break: true,
        allow_player_place: true,
        allow_plugin_write: true,
        runtime_only: true,
    });
    build_lab();
    qexed_plugin_sdk::log("enchanting_lab initialized");
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn qexed_plugin_enchanting_options(ptr: i32, len: i32) -> i64 {
    let Some(query) = (unsafe { qexed_plugin_sdk::decode_payload::<EnchantingQuery>(ptr, len) })
    else {
        return qexed_plugin_sdk::response_ptr_len(&EnchantingResponse::default());
    };
    let item_name = query.item.item_name.as_str();
    let tool = item_name.ends_with("_pickaxe")
        || item_name.ends_with("_axe")
        || item_name.ends_with("_shovel")
        || item_name.ends_with("_hoe");
    if !tool {
        return qexed_plugin_sdk::response_ptr_len(&EnchantingResponse::default());
    }
    qexed_plugin_sdk::response_ptr_len(&EnchantingResponse {
        replace: false,
        options: vec![EnchantingOption {
            id: "qexed:vein_miner".to_string(),
            display_name: "Vein Miner".to_string(),
            level: 1,
            weight: 20,
            required_level: 1,
            lapis_cost: 0,
            item_damage_cost: 0,
            hidden: false,
        }],
    })
}

fn build_lab() {
    let mut blocks = Vec::new();
    for x in -3..=3 {
        for z in -3..=3 {
            blocks.push((DIMENSION, (x, -61, z), "minecraft:smooth_stone"));
        }
    }
    blocks.push((DIMENSION, (0, -60, 0), "minecraft:enchanting_table"));
    blocks.push((DIMENSION, (0, -59, -3), "minecraft:sea_lantern"));
    let _ = world_set_blocks(blocks);
}
