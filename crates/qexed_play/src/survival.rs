//! 生存状态机（v4 play/survival.rs 迁移）。
//!
//! v6 适配：GameMode/Spawn 用 crate::config；EntityPosition 用
//! qexed_protocol::types（v4 在 add_entity 包内）；StoredSurvival 未从
//! qexed_player 导出，此处定义同构结构（字段与 serde 兼容：health/food/saturation）。

use std::time::Duration;

use qexed_packet::net_types::VarInt;
use qexed_protocol::{
    to_client::play::set_health::SetHealth,
    types::EntityPosition,
};

use crate::config::{GameMode, Spawn};

/// 持久化生存数据（qexed_player::player_data::StoredSurvival 的同构副本；
/// qexed_player 尚未导出该类型，字段语义与序列化格式保持一致以便互通）。
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StoredSurvival {
    #[serde(default = "default_survival_health")]
    pub health: f32,
    #[serde(default = "default_survival_food")]
    pub food: i32,
    #[serde(default = "default_survival_saturation")]
    pub saturation: f32,
}

impl Default for StoredSurvival {
    fn default() -> Self {
        Self {
            health: default_survival_health(),
            food: default_survival_food(),
            saturation: default_survival_saturation(),
        }
    }
}

fn default_survival_health() -> f32 {
    MAX_HEALTH
}

fn default_survival_food() -> i32 {
    MAX_FOOD
}

fn default_survival_saturation() -> f32 {
    DEFAULT_SATURATION
}

const MAX_HEALTH: f32 = 20.0;
const MAX_FOOD: i32 = 20;
const MAX_SATURATION: f32 = 20.0;
const DEFAULT_SATURATION: f32 = 5.0;
const SAFE_FALL_DISTANCE: f64 = 3.0;
const VOID_DAMAGE_OFFSET: f64 = 64.0;
/// 世界最小 Y（v4 crate::world::WORLD_MIN_Y = -64；v6 该常量为 qexed_world 内部）。
const WORLD_MIN_Y: i32 = -64;
const EXHAUSTION_FOOD_COST: f32 = 4.0;
const STARVATION_DAMAGE_INTERVAL: Duration = Duration::from_secs(4);
const NATURAL_REGEN_INTERVAL: Duration = Duration::from_secs(4);
const NATURAL_REGEN_FOOD_THRESHOLD: i32 = 18;
const NATURAL_REGEN_EXHAUSTION: f32 = 6.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeathMessage {
    Starve,
    OutOfWorld,
    Fall(FallLocation),
    Stalagmite,
    Drown,
    Generic,
    Explosion,
    PlayerAttack,
    Magic,
    Wither,
}

impl DeathMessage {
    pub(crate) fn translation_key(self) -> &'static str {
        match self {
            Self::Starve => "death.attack.starve",
            Self::OutOfWorld => "death.attack.outOfWorld",
            Self::Fall(location) => location.translation_key(),
            Self::Stalagmite => "death.attack.stalagmite",
            Self::Drown => "death.attack.drown",
            Self::Generic => "death.attack.generic",
            Self::Explosion => "death.attack.explosion",
            Self::PlayerAttack => "death.attack.player",
            Self::Magic => "death.attack.magic",
            Self::Wither => "death.attack.wither",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FallLocation {
    Generic,
    Ladder,
    Vines,
    WeepingVines,
    TwistingVines,
    Scaffolding,
    OtherClimbable,
}

impl FallLocation {
    fn translation_key(self) -> &'static str {
        match self {
            Self::Generic => "death.fell.accident.generic",
            Self::Ladder => "death.fell.accident.ladder",
            Self::Vines => "death.fell.accident.vines",
            Self::WeepingVines => "death.fell.accident.weeping_vines",
            Self::TwistingVines => "death.fell.accident.twisting_vines",
            Self::Scaffolding => "death.fell.accident.scaffolding",
            Self::OtherClimbable => "death.fell.accident.other_climbable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FallContext {
    pub in_lava: bool,
    pub landing: FallLanding,
    pub climbable: Option<FallLocation>,
}

impl Default for FallContext {
    fn default() -> Self {
        Self {
            in_lava: false,
            landing: FallLanding::Generic,
            climbable: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum FallLanding {
    Generic,
    Bed,
    Hay,
    Honey,
    Slime,
    PowderSnow,
    PointedDripstoneTip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SurvivalDamage {
    None,
    Damaged,
    Died(DeathMessage),
}

impl SurvivalDamage {
    pub(crate) fn changed(self) -> bool {
        !matches!(self, Self::None)
    }

    pub(crate) fn death_message(self) -> Option<DeathMessage> {
        match self {
            Self::Died(message) => Some(message),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SurvivalState {
    health: f32,
    food: i32,
    saturation: f32,
    exhaustion: f32,
    dead: bool,
    death_message: Option<DeathMessage>,
    fall_start_y: Option<f64>,
    starvation_timer: Duration,
    natural_regen_timer: Duration,
}

impl SurvivalState {
    pub(crate) fn from_stored(stored: StoredSurvival, game_mode: GameMode) -> Self {
        let mut health = finite_or_default(stored.health, MAX_HEALTH).clamp(0.0, MAX_HEALTH);
        let mut food = stored.food.clamp(0, MAX_FOOD);
        let mut saturation =
            finite_or_default(stored.saturation, DEFAULT_SATURATION).clamp(0.0, MAX_SATURATION);

        if health <= 0.0 || game_mode != GameMode::Survival {
            health = MAX_HEALTH;
            food = MAX_FOOD;
            saturation = DEFAULT_SATURATION;
        }

        Self {
            health,
            food,
            saturation,
            exhaustion: 0.0,
            dead: false,
            death_message: None,
            fall_start_y: None,
            starvation_timer: Duration::ZERO,
            natural_regen_timer: Duration::ZERO,
        }
    }

    pub(crate) fn to_stored(self) -> StoredSurvival {
        StoredSurvival {
            health: self.health,
            food: self.food,
            saturation: self.saturation,
        }
    }

    pub(crate) fn health_packet(self) -> SetHealth {
        SetHealth {
            health: self.health,
            food: VarInt(self.food),
            saturation: self.saturation,
        }
    }

    pub(crate) fn is_dead(self) -> bool {
        self.dead
    }

    #[cfg(test)]
    pub(crate) fn death_message(self) -> DeathMessage {
        self.death_message
            .unwrap_or(DeathMessage::Fall(FallLocation::Generic))
    }

    pub(crate) fn respawn(&mut self) {
        self.health = MAX_HEALTH;
        self.food = MAX_FOOD;
        self.saturation = DEFAULT_SATURATION;
        self.exhaustion = 0.0;
        self.dead = false;
        self.death_message = None;
        self.fall_start_y = None;
        self.starvation_timer = Duration::ZERO;
        self.natural_regen_timer = Duration::ZERO;
    }

    pub(crate) fn apply_exhaustion(&mut self, amount: f32) {
        if amount > 0.0 && amount.is_finite() && !self.dead {
            self.exhaustion = (self.exhaustion + amount).min(40.0);
        }
    }

    pub(crate) fn can_eat(self, always: bool) -> bool {
        !self.dead && (always || self.food < MAX_FOOD)
    }

    pub(crate) fn eat(&mut self, nutrition: i32, saturation_modifier: f32) -> bool {
        if self.dead || nutrition <= 0 {
            return false;
        }
        let previous_food = self.food;
        let previous_saturation = self.saturation;
        self.food = (self.food + nutrition).clamp(0, MAX_FOOD);
        let saturation_gain = nutrition as f32 * saturation_modifier.max(0.0) * 2.0;
        self.saturation = (self.saturation + saturation_gain).clamp(0.0, self.food as f32);
        self.food != previous_food || (self.saturation - previous_saturation).abs() > f32::EPSILON
    }

    pub(crate) fn tick(&mut self, game_mode: GameMode, elapsed: Duration) -> SurvivalDamage {
        if game_mode != GameMode::Survival || self.dead {
            self.starvation_timer = Duration::ZERO;
            self.natural_regen_timer = Duration::ZERO;
            return SurvivalDamage::None;
        }

        let mut changed = self.apply_exhaustion_decay();
        let mut outcome = SurvivalDamage::None;

        if self.food <= 0 {
            self.starvation_timer += elapsed;
            while self.starvation_timer >= STARVATION_DAMAGE_INTERVAL {
                self.starvation_timer -= STARVATION_DAMAGE_INTERVAL;
                let damage = self.apply_damage(1.0, DeathMessage::Starve);
                changed |= damage.changed();
                outcome = strongest_outcome(outcome, damage);
                if self.dead {
                    self.natural_regen_timer = Duration::ZERO;
                    return outcome;
                }
            }
        } else {
            self.starvation_timer = Duration::ZERO;
        }

        if self.food >= NATURAL_REGEN_FOOD_THRESHOLD && self.health < MAX_HEALTH {
            self.natural_regen_timer += elapsed;
            while self.natural_regen_timer >= NATURAL_REGEN_INTERVAL {
                self.natural_regen_timer -= NATURAL_REGEN_INTERVAL;
                if self.heal(1.0) {
                    changed = true;
                    outcome = SurvivalDamage::Damaged;
                }
                self.apply_exhaustion(NATURAL_REGEN_EXHAUSTION);
                changed |= self.apply_exhaustion_decay();
                if self.health >= MAX_HEALTH || self.food < NATURAL_REGEN_FOOD_THRESHOLD {
                    break;
                }
            }
        } else {
            self.natural_regen_timer = Duration::ZERO;
        }

        if changed {
            strongest_outcome(outcome, SurvivalDamage::Damaged)
        } else {
            SurvivalDamage::None
        }
    }

    pub(crate) fn apply_movement(
        &mut self,
        game_mode: GameMode,
        previous: EntityPosition,
        current: EntityPosition,
        fall_context: FallContext,
    ) -> SurvivalDamage {
        self.apply_movement_with_flight(game_mode, false, previous, current, fall_context)
    }

    pub(crate) fn apply_movement_with_flight(
        &mut self,
        game_mode: GameMode,
        allow_flight: bool,
        previous: EntityPosition,
        current: EntityPosition,
        fall_context: FallContext,
    ) -> SurvivalDamage {
        if game_mode != GameMode::Survival || self.dead {
            self.fall_start_y = None;
            return SurvivalDamage::None;
        }

        if current.y < WORLD_MIN_Y as f64 - VOID_DAMAGE_OFFSET {
            return self.apply_damage(MAX_HEALTH, DeathMessage::OutOfWorld);
        }

        if allow_flight {
            self.fall_start_y = None;
            return SurvivalDamage::None;
        }

        let Some(fall_distance) = self.track_fall_distance(previous, current, fall_context) else {
            return SurvivalDamage::None;
        };

        let fall_location = fall_context.climbable.unwrap_or(FallLocation::Generic);
        let (effective_distance, damage_modifier, death_message) =
            fall_damage_profile(fall_distance, fall_context.landing, fall_location);
        let damage = ((effective_distance + 1.0e-6 - SAFE_FALL_DISTANCE) * damage_modifier).floor();
        if damage <= 0.0 {
            return SurvivalDamage::None;
        }
        self.apply_damage(damage as f32, death_message)
    }

    pub(crate) fn apply_damage(
        &mut self,
        amount: f32,
        death_message: DeathMessage,
    ) -> SurvivalDamage {
        if amount <= 0.0 || self.dead {
            return SurvivalDamage::None;
        }

        let previous = self.health;
        self.health = (self.health - amount).max(0.0);
        if self.health <= 0.0 {
            self.dead = true;
            self.death_message = Some(death_message);
            return SurvivalDamage::Died(death_message);
        }
        if (self.health - previous).abs() > f32::EPSILON {
            SurvivalDamage::Damaged
        } else {
            SurvivalDamage::None
        }
    }

    pub(crate) fn heal(&mut self, amount: f32) -> bool {
        if amount <= 0.0 || self.dead || self.health >= MAX_HEALTH {
            return false;
        }

        let previous = self.health;
        self.health = (self.health + amount).min(MAX_HEALTH);
        (self.health - previous).abs() > f32::EPSILON
    }

    fn apply_exhaustion_decay(&mut self) -> bool {
        let mut changed = false;
        while self.exhaustion >= EXHAUSTION_FOOD_COST {
            self.exhaustion -= EXHAUSTION_FOOD_COST;
            if self.saturation > 0.0 {
                self.saturation = (self.saturation - 1.0).max(0.0);
            } else if self.food > 0 {
                self.food -= 1;
            }
            changed = true;
        }
        changed
    }

    fn track_fall_distance(
        &mut self,
        previous: EntityPosition,
        current: EntityPosition,
        fall_context: FallContext,
    ) -> Option<f64> {
        if fall_context.in_lava {
            self.fall_start_y = None;
            return None;
        }

        match (previous.on_ground, current.on_ground) {
            (true, false) => {
                self.fall_start_y = Some(previous.y.max(current.y));
                None
            }
            (false, false) => {
                let start_y = self.fall_start_y.unwrap_or(previous.y.max(current.y));
                self.fall_start_y = Some(start_y.max(current.y));
                None
            }
            (false, true) => {
                let start_y = self
                    .fall_start_y
                    .take()
                    .unwrap_or(previous.y.max(current.y));
                Some((start_y - current.y).max(0.0))
            }
            (true, true) => {
                self.fall_start_y = None;
                None
            }
        }
    }
}

fn fall_damage_profile(
    fall_distance: f64,
    landing: FallLanding,
    fall_location: FallLocation,
) -> (f64, f64, DeathMessage) {
    match landing {
        FallLanding::Generic | FallLanding::PowderSnow => {
            (fall_distance, 1.0, DeathMessage::Fall(fall_location))
        }
        FallLanding::Bed => (fall_distance * 0.5, 1.0, DeathMessage::Fall(fall_location)),
        FallLanding::Hay => (fall_distance, 0.2, DeathMessage::Fall(fall_location)),
        FallLanding::Honey => (fall_distance, 0.2, DeathMessage::Fall(fall_location)),
        FallLanding::Slime => (fall_distance, 0.0, DeathMessage::Fall(fall_location)),
        FallLanding::PointedDripstoneTip => (fall_distance + 2.5, 2.0, DeathMessage::Stalagmite),
    }
}

fn strongest_outcome(current: SurvivalDamage, next: SurvivalDamage) -> SurvivalDamage {
    match (current, next) {
        (SurvivalDamage::Died(_), _) => current,
        (_, SurvivalDamage::Died(_)) => next,
        (SurvivalDamage::Damaged, _) => SurvivalDamage::Damaged,
        (_, SurvivalDamage::Damaged) => SurvivalDamage::Damaged,
        _ => SurvivalDamage::None,
    }
}

pub(crate) fn spawn_position(spawn: &Spawn) -> EntityPosition {
    EntityPosition {
        x: spawn.x,
        y: spawn.y,
        z: spawn.z,
        yaw: spawn.yaw,
        pitch: spawn.pitch,
        on_ground: true,
    }
}

fn finite_or_default(value: f32, default: f32) -> f32 {
    if value.is_finite() { value } else { default }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn position(y: f64, on_ground: bool) -> EntityPosition {
        EntityPosition {
            x: 0.0,
            y,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground,
        }
    }

    fn survival() -> SurvivalState {
        SurvivalState::from_stored(StoredSurvival::default(), GameMode::Survival)
    }

    fn normal_fall() -> FallContext {
        FallContext::default()
    }

    #[test]
    fn stored_survival_is_clamped_for_login() {
        let state = SurvivalState::from_stored(
            StoredSurvival {
                health: f32::NAN,
                food: 99,
                saturation: f32::INFINITY,
            },
            GameMode::Survival,
        );

        assert_eq!(state.health, MAX_HEALTH);
        assert_eq!(state.food, MAX_FOOD);
        assert_eq!(state.saturation, DEFAULT_SATURATION);
        assert!(!state.is_dead());
    }

    #[test]
    fn safe_fall_distance_does_not_damage_player() {
        let mut state = survival();

        assert_eq!(
            state.apply_movement(
                GameMode::Survival,
                position(70.0, true),
                position(70.0, false),
                normal_fall()
            ),
            SurvivalDamage::None
        );
        assert_eq!(
            state.apply_movement(
                GameMode::Survival,
                position(70.0, false),
                position(67.0, true),
                normal_fall()
            ),
            SurvivalDamage::None
        );

        assert_eq!(state.health, MAX_HEALTH);
        assert!(!state.is_dead());
    }

    #[test]
    fn hay_and_slime_change_fall_damage_like_vanilla_blocks() {
        let mut hay = survival();
        hay.apply_movement(
            GameMode::Survival,
            position(90.0, true),
            position(90.0, false),
            normal_fall(),
        );
        assert_eq!(
            hay.apply_movement(
                GameMode::Survival,
                position(90.0, false),
                position(40.0, true),
                FallContext {
                    landing: FallLanding::Hay,
                    ..FallContext::default()
                }
            ),
            SurvivalDamage::Damaged
        );
        assert_eq!(hay.health, 11.0);

        let mut slime = survival();
        slime.apply_movement(
            GameMode::Survival,
            position(90.0, true),
            position(90.0, false),
            normal_fall(),
        );
        assert_eq!(
            slime.apply_movement(
                GameMode::Survival,
                position(90.0, false),
                position(40.0, true),
                FallContext {
                    landing: FallLanding::Slime,
                    ..FallContext::default()
                }
            ),
            SurvivalDamage::None
        );
        assert_eq!(slime.health, MAX_HEALTH);
    }

    #[test]
    fn pointed_dripstone_uses_stalagmite_death_message() {
        let mut state = survival();

        state.apply_movement(
            GameMode::Survival,
            position(80.0, true),
            position(80.0, false),
            normal_fall(),
        );
        let damage = state.apply_movement(
            GameMode::Survival,
            position(80.0, false),
            position(68.0, true),
            FallContext {
                landing: FallLanding::PointedDripstoneTip,
                ..FallContext::default()
            },
        );

        assert_eq!(damage, SurvivalDamage::Died(DeathMessage::Stalagmite));
        assert_eq!(
            state.death_message().translation_key(),
            "death.attack.stalagmite"
        );
    }

    #[test]
    fn fall_damage_starts_after_three_blocks() {
        let mut state = survival();

        state.apply_movement(
            GameMode::Survival,
            position(70.0, true),
            position(70.0, false),
            normal_fall(),
        );
        assert_eq!(
            state.apply_movement(
                GameMode::Survival,
                position(70.0, false),
                position(66.0, true),
                normal_fall()
            ),
            SurvivalDamage::Damaged
        );

        assert_eq!(state.health, 19.0);
        assert!(!state.is_dead());
    }

    #[test]
    fn large_fall_kills_player() {
        let mut state = survival();

        state.apply_movement(
            GameMode::Survival,
            position(90.0, true),
            position(90.0, false),
            normal_fall(),
        );
        assert_eq!(
            state.apply_movement(
                GameMode::Survival,
                position(90.0, false),
                position(40.0, true),
                normal_fall()
            ),
            SurvivalDamage::Died(DeathMessage::Fall(FallLocation::Generic))
        );

        assert_eq!(state.health, 0.0);
        assert!(state.is_dead());
        assert_eq!(
            state.death_message().translation_key(),
            "death.fell.accident.generic"
        );
    }

    #[test]
    fn allow_flight_suppresses_fall_damage_but_not_void_damage() {
        let mut state = survival();

        state.apply_movement_with_flight(
            GameMode::Survival,
            true,
            position(90.0, true),
            position(90.0, false),
            normal_fall(),
        );
        assert_eq!(
            state.apply_movement_with_flight(
                GameMode::Survival,
                true,
                position(90.0, false),
                position(40.0, true),
                normal_fall()
            ),
            SurvivalDamage::None
        );
        assert_eq!(state.health, MAX_HEALTH);

        let mut void = survival();
        assert_eq!(
            void.apply_movement_with_flight(
                GameMode::Survival,
                true,
                position(0.0, false),
                position(
                    WORLD_MIN_Y as f64 - VOID_DAMAGE_OFFSET - 1.0,
                    false
                ),
                normal_fall(),
            ),
            SurvivalDamage::Died(DeathMessage::OutOfWorld)
        );
    }

    #[test]
    fn void_below_world_kills_player() {
        let mut state = survival();

        assert_eq!(
            state.apply_movement(
                GameMode::Survival,
                position(0.0, false),
                position(
                    WORLD_MIN_Y as f64 - VOID_DAMAGE_OFFSET - 1.0,
                    false
                ),
                normal_fall(),
            ),
            SurvivalDamage::Died(DeathMessage::OutOfWorld)
        );

        assert_eq!(state.health, 0.0);
        assert!(state.is_dead());
    }

    #[test]
    fn respawn_restores_default_survival_values() {
        let mut state = survival();
        state.apply_movement(
            GameMode::Survival,
            position(90.0, true),
            position(90.0, false),
            normal_fall(),
        );
        state.apply_movement(
            GameMode::Survival,
            position(90.0, false),
            position(40.0, true),
            normal_fall(),
        );

        state.respawn();

        assert_eq!(state.to_stored(), StoredSurvival::default());
        assert!(!state.is_dead());
    }

    #[test]
    fn exhaustion_consumes_saturation_before_food() {
        let mut state = survival();

        state.apply_exhaustion(4.0);
        assert_eq!(
            state.tick(GameMode::Survival, Duration::from_secs(1)),
            SurvivalDamage::Damaged
        );

        assert_eq!(state.saturation, DEFAULT_SATURATION - 1.0);
        assert_eq!(state.food, MAX_FOOD);
    }

    #[test]
    fn starvation_damages_player_over_time() {
        let mut state = survival();
        state.food = 0;
        state.saturation = 0.0;

        assert_eq!(
            state.tick(GameMode::Survival, STARVATION_DAMAGE_INTERVAL),
            SurvivalDamage::Damaged
        );

        assert_eq!(state.health, MAX_HEALTH - 1.0);
        assert!(!state.is_dead());
    }

    #[test]
    fn natural_regen_heals_and_spends_hunger_budget() {
        let mut state = survival();
        state.health = 18.0;
        state.saturation = 0.0;

        assert_eq!(
            state.tick(GameMode::Survival, NATURAL_REGEN_INTERVAL),
            SurvivalDamage::Damaged
        );

        assert_eq!(state.health, 19.0);
        assert_eq!(state.food, MAX_FOOD - 1);
    }

    #[test]
    fn creative_mode_does_not_tick_hunger() {
        let mut state = survival();
        state.food = 0;

        assert_eq!(
            state.tick(GameMode::Creative, STARVATION_DAMAGE_INTERVAL),
            SurvivalDamage::None
        );

        assert_eq!(state.health, MAX_HEALTH);
    }
}
