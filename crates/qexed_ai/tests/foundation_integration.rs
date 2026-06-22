use qexed_ai::{AiAgent, AiCommand, AiScheduler, NoopGoal, WanderBehavior};
use qexed_block::{BlockId, BlockState};
use qexed_entity::{EntityKind, EntityPose, EntityStore};
use qexed_item::{DEFAULT_MAX_STACK_SIZE, ItemDefinition, ItemId, ItemRegistry};

#[test]
fn block_item_entity_and_ai_compile_together() {
    let block = BlockState::new(BlockId::new("stone"));
    assert_eq!(block.block().as_str(), "minecraft:stone");

    let mut items = ItemRegistry::new();
    let item_id = ItemId::new("minecraft:stone").unwrap();
    items
        .register(ItemDefinition::new(item_id.clone(), DEFAULT_MAX_STACK_SIZE).unwrap());
    let stack = items.stack(&item_id, 16).unwrap();
    assert_eq!(stack.count, 16);

    let mut entities = EntityStore::new();
    let entity_id = entities.spawn(EntityKind::Player, EntityPose::default());
    assert!(entities.contains(entity_id));

    let mut scheduler = AiScheduler::new();
    let agent = AiAgent::new(entity_id, Box::new(WanderBehavior::new(0.25)))
        .with_goal(Box::new(NoopGoal::new("hold_stone", 1)));
    scheduler.add_agent(agent);

    let tick = scheduler.tick();

    assert_eq!(scheduler.tick_index(), 1);
    assert_eq!(tick.commands.len(), 2);
    assert!(matches!(tick.commands[0], AiCommand::SetGoal(_)));
    assert!(matches!(
        tick.commands[1],
        AiCommand::MoveBy {
            entity,
            dx: 0.25,
            dy: 0.0,
            dz: 0.0
        } if entity == entity_id
    ));
}
