use crate::AiCommand;
use qexed_entity::{EntityId, EntityStore};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EntityCommandReport {
    pub pose_updates: usize,
    pub missing_entities: usize,
}

pub fn apply_entity_commands(
    store: &mut EntityStore,
    commands: impl IntoIterator<Item = AiCommand<EntityId>>,
) -> EntityCommandReport {
    let mut report = EntityCommandReport::default();

    for command in commands {
        if let AiCommand::MoveBy { entity, dx, dy, dz } = command {
            let Some(existing) = store.get(entity) else {
                report.missing_entities += 1;
                continue;
            };
            let mut pose = existing.pose;
            pose.position.x += f64::from(dx);
            pose.position.y += f64::from(dy);
            pose.position.z += f64::from(dz);

            if store.update_pose(entity, pose) {
                report.pose_updates += 1;
            } else {
                report.missing_entities += 1;
            }
        }
    }

    report
}
