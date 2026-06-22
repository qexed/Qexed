use qexed_block::{BlockRegistry, BlockState, BlockStateId};
use qexed_item::{DEFAULT_MAX_STACK_SIZE, ItemDefinition, ItemId, ItemRegistry};
use serde_json::json;

#[test]
fn block_state_and_item_stack_resolve_consistent_registry_identity() {
    let blocks = BlockRegistry::from_blocks_report(&json!({
        "minecraft:stone": {
            "states": [
                {"id": 1, "default": true}
            ]
        },
        "minecraft:oak_log": {
            "properties": {
                "axis": ["x", "y", "z"]
            },
            "states": [
                {"id": 10, "properties": {"axis": "x"}},
                {"id": 11, "properties": {"axis": "y"}, "default": true},
                {"id": 12, "properties": {"axis": "z"}}
            ]
        }
    }))
    .unwrap();

    let default_stone = blocks.default_state_id("stone").unwrap();
    assert_eq!(default_stone, BlockStateId::new(1));
    assert_eq!(
        blocks.state_id(&BlockState::new("minecraft:stone")).unwrap(),
        default_stone
    );
    assert_eq!(
        blocks.state_by_id(default_stone).unwrap(),
        &BlockState::new("minecraft:stone")
    );

    let z_axis_log = BlockState::with_properties("minecraft:oak_log", [("axis", "z")]);
    let z_axis_log_id = blocks.state_id(&z_axis_log).unwrap();
    assert_eq!(
        blocks.state_id_by_name("oak_log", [("axis", "z")]).unwrap(),
        z_axis_log_id
    );
    assert_eq!(blocks.state_by_id(z_axis_log_id).unwrap(), &z_axis_log);

    let mut items = ItemRegistry::new();
    let stone_item = ItemId::new("minecraft:stone").unwrap();
    items
        .register(ItemDefinition::new(stone_item.clone(), DEFAULT_MAX_STACK_SIZE).unwrap());

    let stack_from_id = items.stack(&stone_item, 32).unwrap();
    let stack_from_name = items.stack_by_str("minecraft:stone", 32).unwrap();

    assert_eq!(items.get_by_str("minecraft:stone").unwrap().id, stone_item);
    assert_eq!(stack_from_name, stack_from_id);
    assert_eq!(stack_from_name.item.as_str(), BlockState::new("stone").block().as_str());
}
