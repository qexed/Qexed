use qexed_ai::{
    AiAgent, AiCommand, AiScheduler, AiVec3, BlackboardValue, MoveToNearestTargetBehavior,
    NoopGoal, TargetSnapshot, WanderBehavior, entity_adapter::apply_entity_commands,
};
use qexed_block::{BlockId, BlockState};
use qexed_entity::{
    EntityKind, EntityPose, EntityPosition, EntityStore, EntityTypeId, EntityUpdateSnapshot,
};
use qexed_item::{DEFAULT_MAX_STACK_SIZE, ItemDefinition, ItemId, ItemRegistry};

#[test]
fn block_item_entity_and_ai_compile_together() {
    let block = BlockState::new(BlockId::new("stone"));
    assert_eq!(block.block().as_str(), "minecraft:stone");

    let mut items = ItemRegistry::new();
    let item_id = ItemId::new("minecraft:stone").unwrap();
    items.register(ItemDefinition::new(item_id.clone(), DEFAULT_MAX_STACK_SIZE).unwrap());
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

#[test]
fn ai_commands_update_entity_store_pose() {
    let mut entities = EntityStore::new();
    let entity_id = entities.spawn(EntityKind::Player, EntityPose::default());
    entities.drain_updates();

    let mut scheduler = AiScheduler::new();
    scheduler.add_agent(AiAgent::new(entity_id, Box::new(WanderBehavior::new(0.25))));

    let tick = scheduler.tick();
    let report = apply_entity_commands(&mut entities, tick.commands);

    assert_eq!(report.pose_updates(), 1);
    assert_eq!(report.movement_updates, 1);
    assert_eq!(report.missing_entities, 0);
    assert_eq!(entities.get(entity_id).unwrap().pose.position.x, 0.25);
    assert_eq!(entities.get(entity_id).unwrap().pose.velocity.x, 0.25);
    assert!(matches!(
        entities.updates(),
        [EntityUpdateSnapshot::Pose(snapshot)] if snapshot.id == entity_id
    ));
}

#[test]
fn move_to_nearest_target_updates_entity_motion_and_look() {
    let mut entities = EntityStore::new();
    let actor = entities.spawn(EntityKind::Player, EntityPose::default());
    let far = entities.spawn(
        EntityKind::Mob {
            type_id: EntityTypeId::new(1),
        },
        EntityPose {
            position: EntityPosition {
                x: 10.0,
                y: 64.0,
                z: 0.0,
            },
            ..EntityPose::default()
        },
    );
    let near = entities.spawn(
        EntityKind::Mob {
            type_id: EntityTypeId::new(2),
        },
        EntityPose {
            position: EntityPosition {
                x: 0.0,
                y: 64.0,
                z: 3.0,
            },
            ..EntityPose::default()
        },
    );
    entities.drain_updates();

    let mut behavior = MoveToNearestTargetBehavior::new(0.5);
    behavior.set_candidates([
        TargetSnapshot::new(far, AiVec3::new(10.0, 64.0, 0.0)),
        TargetSnapshot::new(near, AiVec3::new(0.0, 64.0, 3.0)),
    ]);

    let mut agent = AiAgent::new(actor, Box::new(behavior));
    agent
        .blackboard_mut()
        .insert("position.x", BlackboardValue::F64(0.0));
    agent
        .blackboard_mut()
        .insert("position.y", BlackboardValue::F64(64.0));
    agent
        .blackboard_mut()
        .insert("position.z", BlackboardValue::F64(0.0));

    let mut scheduler = AiScheduler::new();
    scheduler.add_agent(agent);

    let tick = scheduler.tick();
    assert!(matches!(
        tick.commands.as_slice(),
        [
            AiCommand::MoveBy {
                entity,
                dx: 0.0,
                dy: 0.0,
                dz: 0.5
            },
            AiCommand::LookAt {
                entity: look_entity,
                ..
            }
        ] if *entity == actor && *look_entity == actor
    ));

    let report = apply_entity_commands(&mut entities, tick.commands);

    assert_eq!(report.movement_updates, 1);
    assert_eq!(report.look_updates, 1);
    assert_eq!(report.missing_entities, 0);
    let actor_pose = entities.get(actor).unwrap().pose;
    assert_eq!(actor_pose.position.z, 0.5);
    assert_eq!(actor_pose.velocity.z, 0.5);
    assert_eq!(actor_pose.yaw, 0.0);
}
