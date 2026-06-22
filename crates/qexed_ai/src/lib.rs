use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

pub mod entity_adapter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BehaviorStatus {
    Running,
    Success,
    Failure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalStatus {
    Active,
    Complete,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GoalId(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalState {
    id: GoalId,
    priority: i32,
    complete: bool,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Blackboard {
    values: HashMap<String, BlackboardValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BlackboardValue {
    Bool(bool),
    I64(i64),
    F64(f64),
    Text(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Memory {
    entries: VecDeque<MemoryEntry>,
    capacity: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryEntry {
    pub tick: u64,
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AiCommand<E> {
    Noop,
    SetGoal(GoalId),
    MoveBy {
        entity: E,
        dx: f32,
        dy: f32,
        dz: f32,
    },
    LookAt {
        entity: E,
        x: f64,
        y: f64,
        z: f64,
    },
}

pub struct AiTickContext<'a, E> {
    pub entity: E,
    pub tick: u64,
    pub blackboard: &'a mut Blackboard,
    pub memory: &'a mut Memory,
    pub commands: &'a mut Vec<AiCommand<E>>,
}

pub trait Behavior<E> {
    fn tick(&mut self, context: &mut AiTickContext<'_, E>) -> BehaviorStatus;
}

pub trait Goal<E> {
    fn id(&self) -> &GoalId;
    fn priority(&self) -> i32;
    fn is_complete(&self) -> bool;
    fn tick_goal(&mut self, context: &mut AiTickContext<'_, E>) -> GoalStatus;
}

pub trait AiTick<E> {
    fn tick_ai(&mut self, context: &mut AiTickContext<'_, E>) -> BehaviorStatus;
}

pub struct NoopBehavior;

pub struct NoopGoal {
    state: GoalState,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AiVec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TargetSnapshot<E> {
    pub entity: E,
    pub position: AiVec3,
}

pub struct MoveToNearestTargetBehavior<E> {
    candidates: Vec<TargetSnapshot<E>>,
    speed: f32,
    stop_distance: f64,
    eye_height: f64,
}

pub struct Sequence<E> {
    children: Vec<Box<dyn Behavior<E>>>,
    current: usize,
}

pub struct Selector<E> {
    children: Vec<Box<dyn Behavior<E>>>,
    current: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WanderBehavior {
    step: f32,
}

pub struct AiAgent<E> {
    entity: E,
    behavior: Box<dyn Behavior<E>>,
    goals: Vec<Box<dyn Goal<E>>>,
    blackboard: Blackboard,
    memory: Memory,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct SchedulerTick<E> {
    pub commands: Vec<AiCommand<E>>,
}

#[derive(Default)]
pub struct AiScheduler<E> {
    agents: Vec<AiAgent<E>>,
    tick: u64,
}

impl GoalId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl GoalState {
    pub fn new(id: impl Into<String>, priority: i32) -> Self {
        Self {
            id: GoalId::new(id),
            priority,
            complete: false,
        }
    }

    pub fn id(&self) -> &GoalId {
        &self.id
    }

    pub fn priority(&self) -> i32 {
        self.priority
    }

    pub fn is_complete(&self) -> bool {
        self.complete
    }

    pub fn mark_complete(&mut self) {
        self.complete = true;
    }
}

impl NoopGoal {
    pub fn new(id: impl Into<String>, priority: i32) -> Self {
        Self {
            state: GoalState::new(id, priority),
        }
    }
}

impl<E> Goal<E> for NoopGoal {
    fn id(&self) -> &GoalId {
        self.state.id()
    }

    fn priority(&self) -> i32 {
        self.state.priority()
    }

    fn is_complete(&self) -> bool {
        self.state.is_complete()
    }

    fn tick_goal(&mut self, context: &mut AiTickContext<'_, E>) -> GoalStatus {
        context
            .commands
            .push(AiCommand::SetGoal(self.state.id().clone()));
        self.state.mark_complete();
        GoalStatus::Complete
    }
}

impl AiVec3 {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn horizontal_distance_squared(self, other: Self) -> f64 {
        let dx = other.x - self.x;
        let dz = other.z - self.z;
        dx.mul_add(dx, dz * dz)
    }
}

impl<E> TargetSnapshot<E> {
    pub const fn new(entity: E, position: AiVec3) -> Self {
        Self { entity, position }
    }
}

impl<E: Copy + Eq> MoveToNearestTargetBehavior<E> {
    pub fn new(speed: f32) -> Self {
        Self {
            candidates: Vec::new(),
            speed,
            stop_distance: 0.25,
            eye_height: 1.6,
        }
    }

    pub fn with_stop_distance(mut self, stop_distance: f64) -> Self {
        self.stop_distance = stop_distance.max(0.0);
        self
    }

    pub fn with_eye_height(mut self, eye_height: f64) -> Self {
        self.eye_height = eye_height;
        self
    }

    pub fn set_candidates(&mut self, candidates: impl IntoIterator<Item = TargetSnapshot<E>>) {
        self.candidates.clear();
        self.candidates.extend(candidates);
    }

    pub fn nearest_target(&self, origin: AiVec3, self_entity: E) -> Option<&TargetSnapshot<E>> {
        self.candidates
            .iter()
            .filter(|candidate| candidate.entity != self_entity)
            .min_by(|left, right| {
                origin
                    .horizontal_distance_squared(left.position)
                    .partial_cmp(&origin.horizontal_distance_squared(right.position))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }
}

impl<E: Copy + Eq + ToString> Behavior<E> for MoveToNearestTargetBehavior<E> {
    fn tick(&mut self, context: &mut AiTickContext<'_, E>) -> BehaviorStatus {
        let Some(BlackboardValue::F64(x)) = context.blackboard.get("position.x") else {
            return BehaviorStatus::Failure;
        };
        let Some(BlackboardValue::F64(y)) = context.blackboard.get("position.y") else {
            return BehaviorStatus::Failure;
        };
        let Some(BlackboardValue::F64(z)) = context.blackboard.get("position.z") else {
            return BehaviorStatus::Failure;
        };

        let origin = AiVec3::new(*x, *y, *z);
        let Some(target) = self.nearest_target(origin, context.entity) else {
            return BehaviorStatus::Failure;
        };

        let dx = target.position.x - origin.x;
        let dz = target.position.z - origin.z;
        let distance = (dx * dx + dz * dz).sqrt();
        if distance <= self.stop_distance {
            return BehaviorStatus::Success;
        }

        let scale = f64::from(self.speed) / distance.max(f64::EPSILON);
        context.commands.push(AiCommand::MoveBy {
            entity: context.entity,
            dx: (dx * scale) as f32,
            dy: 0.0,
            dz: (dz * scale) as f32,
        });
        context.commands.push(AiCommand::LookAt {
            entity: context.entity,
            x: target.position.x,
            y: target.position.y + self.eye_height,
            z: target.position.z,
        });
        context
            .memory
            .remember(context.tick, "last_target", target.entity.to_string());
        BehaviorStatus::Running
    }
}

impl Blackboard {
    pub fn insert(&mut self, key: impl Into<String>, value: BlackboardValue) {
        self.values.insert(key.into(), value);
    }

    pub fn get(&self, key: &str) -> Option<&BlackboardValue> {
        self.values.get(key)
    }

    pub fn remove(&mut self, key: &str) -> Option<BlackboardValue> {
        self.values.remove(key)
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::with_capacity(32)
    }
}

impl Memory {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn remember(&mut self, tick: u64, key: impl Into<String>, value: impl Into<String>) {
        if self.capacity == 0 {
            return;
        }

        while self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }

        self.entries.push_back(MemoryEntry {
            tick,
            key: key.into(),
            value: value.into(),
        });
    }

    pub fn latest(&self, key: &str) -> Option<&MemoryEntry> {
        self.entries.iter().rev().find(|entry| entry.key == key)
    }

    pub fn entries(&self) -> impl Iterator<Item = &MemoryEntry> {
        self.entries.iter()
    }
}

impl<E> Behavior<E> for NoopBehavior {
    fn tick(&mut self, context: &mut AiTickContext<'_, E>) -> BehaviorStatus {
        context.commands.push(AiCommand::Noop);
        BehaviorStatus::Success
    }
}

impl<E> Sequence<E> {
    pub fn new(children: Vec<Box<dyn Behavior<E>>>) -> Self {
        Self {
            children,
            current: 0,
        }
    }
}

impl<E> Behavior<E> for Sequence<E> {
    fn tick(&mut self, context: &mut AiTickContext<'_, E>) -> BehaviorStatus {
        while self.current < self.children.len() {
            match self.children[self.current].tick(context) {
                BehaviorStatus::Success => self.current += 1,
                BehaviorStatus::Failure => {
                    self.current = 0;
                    return BehaviorStatus::Failure;
                }
                BehaviorStatus::Running => return BehaviorStatus::Running,
            }
        }

        self.current = 0;
        BehaviorStatus::Success
    }
}

impl<E> Selector<E> {
    pub fn new(children: Vec<Box<dyn Behavior<E>>>) -> Self {
        Self {
            children,
            current: 0,
        }
    }
}

impl<E> Behavior<E> for Selector<E> {
    fn tick(&mut self, context: &mut AiTickContext<'_, E>) -> BehaviorStatus {
        while self.current < self.children.len() {
            match self.children[self.current].tick(context) {
                BehaviorStatus::Success => {
                    self.current = 0;
                    return BehaviorStatus::Success;
                }
                BehaviorStatus::Failure => self.current += 1,
                BehaviorStatus::Running => return BehaviorStatus::Running,
            }
        }

        self.current = 0;
        BehaviorStatus::Failure
    }
}

impl WanderBehavior {
    pub fn new(step: f32) -> Self {
        Self { step }
    }
}

impl<E: Copy> Behavior<E> for WanderBehavior {
    fn tick(&mut self, context: &mut AiTickContext<'_, E>) -> BehaviorStatus {
        context.commands.push(AiCommand::MoveBy {
            entity: context.entity,
            dx: self.step,
            dy: 0.0,
            dz: 0.0,
        });
        context
            .memory
            .remember(context.tick, "last_behavior", "wander");
        BehaviorStatus::Running
    }
}

impl<E> AiAgent<E> {
    pub fn new(entity: E, behavior: Box<dyn Behavior<E>>) -> Self {
        Self {
            entity,
            behavior,
            goals: Vec::new(),
            blackboard: Blackboard::default(),
            memory: Memory::default(),
        }
    }

    pub fn with_goal(mut self, goal: Box<dyn Goal<E>>) -> Self {
        self.add_goal(goal);
        self
    }

    pub fn add_goal(&mut self, goal: Box<dyn Goal<E>>) {
        self.goals.push(goal);
    }

    pub fn entity(&self) -> &E {
        &self.entity
    }

    pub fn goals(&self) -> &[Box<dyn Goal<E>>] {
        &self.goals
    }

    pub fn blackboard(&self) -> &Blackboard {
        &self.blackboard
    }

    pub fn blackboard_mut(&mut self) -> &mut Blackboard {
        &mut self.blackboard
    }

    pub fn memory(&self) -> &Memory {
        &self.memory
    }

    pub fn memory_mut(&mut self) -> &mut Memory {
        &mut self.memory
    }
}

impl<E: Copy + Eq + Hash> AiScheduler<E> {
    pub fn new() -> Self {
        Self {
            agents: Vec::new(),
            tick: 0,
        }
    }

    pub fn add_agent(&mut self, agent: AiAgent<E>) {
        if self
            .agents
            .iter()
            .any(|existing| existing.entity == agent.entity)
        {
            return;
        }

        self.agents.push(agent);
    }

    pub fn remove_agent(&mut self, entity: E) -> Option<AiAgent<E>> {
        let index = self
            .agents
            .iter()
            .position(|existing| existing.entity == entity)?;
        Some(self.agents.remove(index))
    }

    pub fn tick(&mut self) -> SchedulerTick<E> {
        self.tick = self.tick.saturating_add(1);
        let mut output = SchedulerTick {
            commands: Vec::new(),
        };

        for agent in &mut self.agents {
            if let Some(goal_index) = agent
                .goals
                .iter()
                .enumerate()
                .filter(|(_, goal)| !goal.is_complete())
                .max_by_key(|(_, goal)| goal.priority())
                .map(|(index, _)| index)
            {
                let mut context = AiTickContext {
                    entity: agent.entity,
                    tick: self.tick,
                    blackboard: &mut agent.blackboard,
                    memory: &mut agent.memory,
                    commands: &mut output.commands,
                };
                agent.goals[goal_index].tick_goal(&mut context);
            }

            {
                let mut context = AiTickContext {
                    entity: agent.entity,
                    tick: self.tick,
                    blackboard: &mut agent.blackboard,
                    memory: &mut agent.memory,
                    commands: &mut output.commands,
                };
                agent.behavior.tick(&mut context);
            }
        }

        output
    }

    pub fn tick_index(&self) -> u64 {
        self.tick
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AiAgent, AiCommand, AiScheduler, Behavior, BehaviorStatus, Blackboard, BlackboardValue,
        GoalStatus, Memory, NoopBehavior, NoopGoal, Selector, Sequence, WanderBehavior,
    };

    struct FixedBehavior(BehaviorStatus);

    impl<E> Behavior<E> for FixedBehavior {
        fn tick(&mut self, _context: &mut super::AiTickContext<'_, E>) -> BehaviorStatus {
            self.0
        }
    }

    #[test]
    fn blackboard_stores_values_by_key() {
        let mut blackboard = Blackboard::default();

        blackboard.insert("target_visible", BlackboardValue::Bool(true));

        assert_eq!(
            blackboard.get("target_visible"),
            Some(&BlackboardValue::Bool(true))
        );
    }

    #[test]
    fn memory_keeps_latest_entry_with_capacity() {
        let mut memory = Memory::with_capacity(1);

        memory.remember(1, "state", "idle");
        memory.remember(2, "state", "alert");

        assert_eq!(memory.entries().count(), 1);
        assert_eq!(memory.latest("state").unwrap().value, "alert");
    }

    #[test]
    fn sequence_fails_when_child_fails() {
        let mut sequence = Sequence::new(vec![
            Box::new(FixedBehavior(BehaviorStatus::Success)),
            Box::new(FixedBehavior(BehaviorStatus::Failure)),
        ]);
        let mut blackboard = Blackboard::default();
        let mut memory = Memory::default();
        let mut commands = Vec::new();
        let mut context = super::AiTickContext {
            entity: 1_u64,
            tick: 1,
            blackboard: &mut blackboard,
            memory: &mut memory,
            commands: &mut commands,
        };

        assert_eq!(sequence.tick(&mut context), BehaviorStatus::Failure);
    }

    #[test]
    fn selector_succeeds_when_child_succeeds() {
        let mut selector = Selector::new(vec![
            Box::new(FixedBehavior(BehaviorStatus::Failure)),
            Box::new(FixedBehavior(BehaviorStatus::Success)),
        ]);
        let mut blackboard = Blackboard::default();
        let mut memory = Memory::default();
        let mut commands = Vec::new();
        let mut context = super::AiTickContext {
            entity: 1_u64,
            tick: 1,
            blackboard: &mut blackboard,
            memory: &mut memory,
            commands: &mut commands,
        };

        assert_eq!(selector.tick(&mut context), BehaviorStatus::Success);
    }

    #[test]
    fn scheduler_ticks_agents_and_collects_commands() {
        let mut scheduler = AiScheduler::new();
        scheduler.add_agent(AiAgent::new(7_u64, Box::new(WanderBehavior::new(0.25))));

        let tick = scheduler.tick();

        assert_eq!(scheduler.tick_index(), 1);
        assert_eq!(
            tick.commands,
            vec![AiCommand::MoveBy {
                entity: 7,
                dx: 0.25,
                dy: 0.0,
                dz: 0.0,
            }]
        );
    }

    #[test]
    fn noop_behavior_emits_noop_command() {
        let mut scheduler = AiScheduler::new();
        scheduler.add_agent(AiAgent::new(9_u64, Box::new(NoopBehavior)));

        let tick = scheduler.tick();

        assert_eq!(tick.commands, vec![AiCommand::Noop]);
    }

    #[test]
    fn scheduler_ticks_highest_priority_goal_before_behavior() {
        let mut scheduler = AiScheduler::new();
        let agent = AiAgent::new(11_u64, Box::new(NoopBehavior))
            .with_goal(Box::new(NoopGoal::new("idle", 1)))
            .with_goal(Box::new(NoopGoal::new("urgent", 10)));
        scheduler.add_agent(agent);

        let tick = scheduler.tick();

        assert_eq!(
            tick.commands,
            vec![
                AiCommand::SetGoal(super::GoalId::new("urgent")),
                AiCommand::Noop
            ]
        );
    }

    #[test]
    fn noop_goal_completes_after_one_tick() {
        let mut goal = NoopGoal::new("idle", 1);
        let mut blackboard = Blackboard::default();
        let mut memory = Memory::default();
        let mut commands = Vec::new();
        let mut context = super::AiTickContext {
            entity: 1_u64,
            tick: 1,
            blackboard: &mut blackboard,
            memory: &mut memory,
            commands: &mut commands,
        };

        assert_eq!(
            super::Goal::tick_goal(&mut goal, &mut context),
            GoalStatus::Complete
        );
        assert!(super::Goal::<u64>::is_complete(&goal));
    }
}
