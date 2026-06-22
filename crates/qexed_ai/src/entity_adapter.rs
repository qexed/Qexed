use crate::AiCommand;
use qexed_entity::{EntityId, EntityStore};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EntityCommandReport {
    pub movement_updates: usize,
    pub look_updates: usize,
    pub missing_entities: usize,
}

impl EntityCommandReport {
    pub const fn pose_updates(self) -> usize {
        self.movement_updates + self.look_updates
    }
}

pub fn apply_entity_commands(
    store: &mut EntityStore,
    commands: impl IntoIterator<Item = AiCommand<EntityId>>,
) -> EntityCommandReport {
    let mut report = EntityCommandReport::default();

    for command in commands {
        match command {
            AiCommand::MoveBy { entity, dx, dy, dz } => {
                if store.update_pose_with(entity, |pose| {
                    pose.position.x += f64::from(dx);
                    pose.position.y += f64::from(dy);
                    pose.position.z += f64::from(dz);
                    pose.velocity.x = f64::from(dx);
                    pose.velocity.y = f64::from(dy);
                    pose.velocity.z = f64::from(dz);
                }) {
                    report.movement_updates += 1;
                } else {
                    report.missing_entities += 1;
                }
            }
            AiCommand::LookAt { entity, x, y, z } => {
                let Some(origin) = store.get(entity).map(|existing| existing.pose.position) else {
                    report.missing_entities += 1;
                    continue;
                };

                if store.update_pose_with(entity, |pose| {
                    let dx = x - origin.x;
                    let dy = y - origin.y;
                    let dz = z - origin.z;
                    let horizontal = (dx * dx + dz * dz).sqrt();
                    pose.yaw = (-(dx.atan2(dz)).to_degrees()) as f32;
                    pose.pitch = (-(dy.atan2(horizontal)).to_degrees()) as f32;
                }) {
                    report.look_updates += 1;
                } else {
                    report.missing_entities += 1;
                }
            }
            AiCommand::Noop | AiCommand::SetGoal(_) => {}
        }
    }

    report
}
