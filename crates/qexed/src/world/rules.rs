use anyhow::{Context, Result};
use qexed_config::app::qexed::server::{LightAlgorithm, LightMode, World};
use qexed_config::tool::AppConfigTrait;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

const DEFAULT_RULES_DIR: &str = "config/qexed.d/worlds";

type DimensionRuleFile = qexed_config::app::qexed::world_rules::DimensionWorldRules;

#[derive(Debug, Clone)]
pub struct DimensionRuleSnapshot {
    pub dimension: String,
    pub dimension_type: String,
    pub read_only: bool,
    pub block_updates: bool,
    pub light: LightMode,
    pub light_algorithm: LightAlgorithm,
    pub time_value: i64,
    pub fixed_time: Option<i64>,
    pub daylight_cycle: bool,
    pub tick_step: i64,
}

#[derive(Debug, Clone)]
struct DimensionRuleState {
    rule: DimensionRuleFile,
    current_time: i64,
}

#[derive(Clone, Debug)]
pub struct WorldRulesManager {
    inner: Arc<WorldRulesInner>,
}

#[derive(Debug)]
struct WorldRulesInner {
    base_dir: PathBuf,
    default_world: World,
    states: RwLock<HashMap<String, DimensionRuleState>>,
}

impl WorldRulesManager {
    pub fn from_world_config(world: &World) -> Result<Self> {
        let manager = Self {
            inner: Arc::new(WorldRulesInner {
                base_dir: PathBuf::from(DEFAULT_RULES_DIR),
                default_world: world.clone(),
                states: RwLock::new(HashMap::new()),
            }),
        };
        std::fs::create_dir_all(&manager.inner.base_dir).with_context(|| {
            format!(
                "create world rules directory {}",
                manager.inner.base_dir.display()
            )
        })?;
        manager.ensure_loaded(&world.dimension)?;
        Ok(manager)
    }

    pub fn snapshot(&self, dimension: &str) -> DimensionRuleSnapshot {
        if let Err(err) = self.ensure_loaded(dimension) {
            log::warn!(
                "failed to load world rule for dimension {dimension}, using defaults: {err:#}"
            );
            let default =
                DimensionRuleFile::from_world_defaults(&self.inner.default_world, dimension);
            return DimensionRuleSnapshot {
                dimension: default.dimension,
                dimension_type: default.dimension_type,
                read_only: default.read_only,
                block_updates: default.block_updates,
                light: default.light,
                light_algorithm: default.light_algorithm,
                time_value: default.time.value,
                fixed_time: default.time.fixed,
                daylight_cycle: default.time.daylight_cycle,
                tick_step: default.time.tick_step,
            };
        }
        let states = self.inner.states.read().expect("world rules lock poisoned");
        let state = states.get(dimension).expect("dimension rule state missing");
        DimensionRuleSnapshot {
            dimension: state.rule.dimension.clone(),
            dimension_type: state.rule.dimension_type.clone(),
            read_only: state.rule.read_only,
            block_updates: state.rule.block_updates,
            light: state.rule.light.clone(),
            light_algorithm: state.rule.light_algorithm,
            time_value: state.current_time,
            fixed_time: state.rule.time.fixed,
            daylight_cycle: state.rule.time.daylight_cycle,
            tick_step: state.rule.time.tick_step,
        }
    }

    pub fn current_time(&self, dimension: &str) -> i64 {
        self.snapshot(dimension).time_value
    }

    pub fn tick_dimension_time(&self, dimension: &str) -> i64 {
        let _ = self.ensure_loaded(dimension);
        let mut states = self
            .inner
            .states
            .write()
            .expect("world rules lock poisoned");
        let Some(state) = states.get_mut(dimension) else {
            return 0;
        };

        if let Some(fixed) = state.rule.time.fixed {
            state.current_time = fixed;
            return state.current_time;
        }

        if state.rule.time.daylight_cycle {
            state.current_time = state
                .current_time
                .saturating_add(state.rule.time.tick_step.max(0));
        }
        state.current_time
    }

    pub fn set_time_value(&self, dimension: &str, value: i64) -> Result<DimensionRuleSnapshot> {
        self.update_rule(dimension, |state| {
            state.current_time = value;
            state.rule.time.value = value;
        })
    }

    pub fn add_time_value(&self, dimension: &str, delta: i64) -> Result<DimensionRuleSnapshot> {
        self.update_rule(dimension, |state| {
            state.current_time = state.current_time.saturating_add(delta);
            state.rule.time.value = state.current_time;
        })
    }

    pub fn set_daylight_cycle(
        &self,
        dimension: &str,
        enabled: bool,
    ) -> Result<DimensionRuleSnapshot> {
        self.update_rule(dimension, |state| {
            state.rule.time.daylight_cycle = enabled;
        })
    }

    pub fn set_block_updates(
        &self,
        dimension: &str,
        enabled: bool,
    ) -> Result<DimensionRuleSnapshot> {
        self.update_rule(dimension, |state| {
            state.rule.block_updates = enabled;
        })
    }

    pub fn set_fixed_time(
        &self,
        dimension: &str,
        fixed: Option<i64>,
    ) -> Result<DimensionRuleSnapshot> {
        self.update_rule(dimension, |state| {
            state.rule.time.fixed = fixed;
            if let Some(value) = fixed {
                state.current_time = value;
                state.rule.time.value = value;
            }
        })
    }

    pub fn set_tick_step(&self, dimension: &str, tick_step: i64) -> Result<DimensionRuleSnapshot> {
        self.update_rule(dimension, |state| {
            state.rule.time.tick_step = tick_step.max(0);
        })
    }

    pub fn set_light(&self, dimension: &str, light: LightMode) -> Result<DimensionRuleSnapshot> {
        self.update_rule(dimension, |state| {
            state.rule.light = light;
        })
    }

    pub fn set_read_only(&self, dimension: &str, read_only: bool) -> Result<DimensionRuleSnapshot> {
        self.update_rule(dimension, |state| {
            state.rule.read_only = read_only;
        })
    }

    pub fn ensure_loaded(&self, dimension: &str) -> Result<()> {
        if self
            .inner
            .states
            .read()
            .expect("world rules lock poisoned")
            .contains_key(dimension)
        {
            return Ok(());
        }
        let state = self.load_or_create_state(dimension)?;
        self.inner
            .states
            .write()
            .expect("world rules lock poisoned")
            .insert(dimension.to_string(), state);
        Ok(())
    }

    fn update_rule(
        &self,
        dimension: &str,
        mutate: impl FnOnce(&mut DimensionRuleState),
    ) -> Result<DimensionRuleSnapshot> {
        self.ensure_loaded(dimension)?;
        let (snapshot, rule) = {
            let mut states = self
                .inner
                .states
                .write()
                .expect("world rules lock poisoned");
            let state = states
                .get_mut(dimension)
                .expect("dimension rule state missing");
            mutate(state);
            let snapshot = DimensionRuleSnapshot {
                dimension: state.rule.dimension.clone(),
                dimension_type: state.rule.dimension_type.clone(),
                read_only: state.rule.read_only,
                block_updates: state.rule.block_updates,
                light: state.rule.light.clone(),
                light_algorithm: state.rule.light_algorithm,
                time_value: state.current_time,
                fixed_time: state.rule.time.fixed,
                daylight_cycle: state.rule.time.daylight_cycle,
                tick_step: state.rule.time.tick_step,
            };
            (snapshot, state.rule.clone())
        };
        self.write_rule_file(dimension, &rule)?;
        Ok(snapshot)
    }

    fn load_or_create_state(&self, dimension: &str) -> Result<DimensionRuleState> {
        let rule_dir = self.dimension_rules_dir_path(dimension)?;
        let mut rule =
            DimensionRuleFile::load_or_create_default(None, Some(false), Some(rule_dir))?;
        if rule.dimension.trim().is_empty() {
            rule.dimension = dimension.to_string();
        }
        if rule.dimension_type.trim().is_empty() {
            rule.dimension_type = default_dimension_type(dimension);
        }
        if rule.tick_step_invalid() {
            rule.time.tick_step = 1;
        }
        if rule.time.daylight_cycle && rule.time.tick_step < 0 {
            rule.time.tick_step = 0;
        }
        self.write_rule_file(dimension, &rule)?;
        Ok(DimensionRuleState {
            current_time: rule.time.value,
            rule,
        })
    }

    fn write_rule_file(&self, dimension: &str, rule: &DimensionRuleFile) -> Result<()> {
        let rule_dir = self.dimension_rules_dir_path(dimension)?;
        rule.save_to_config(None, Some(false), Some(rule_dir))
    }

    fn dimension_rules_dir_path(&self, dimension: &str) -> Result<PathBuf> {
        let folder = sanitize_dimension_folder(dimension)?;
        Ok(self.inner.base_dir.join(folder))
    }
}

fn default_dimension_type(dimension: &str) -> String {
    match dimension {
        "minecraft:the_nether" => "minecraft:the_nether".to_string(),
        "minecraft:the_end" => "minecraft:the_end".to_string(),
        _ => "minecraft:overworld".to_string(),
    }
}

trait DimensionRuleExt {
    fn from_world_defaults(world: &World, dimension: &str) -> Self;
    fn tick_step_invalid(&self) -> bool;
}

impl DimensionRuleExt for DimensionRuleFile {
    fn from_world_defaults(world: &World, dimension: &str) -> Self {
        Self {
            dimension: dimension.to_string(),
            dimension_type: default_dimension_type(dimension),
            read_only: world.read_only,
            block_updates: true,
            light: world.light.clone(),
            light_algorithm: world.light_algorithm,
            time: qexed_config::app::qexed::world_rules::DimensionTimeRule::default(),
        }
    }

    fn tick_step_invalid(&self) -> bool {
        self.time.tick_step < 0
    }
}

fn sanitize_dimension_folder(dimension: &str) -> Result<String> {
    let dimension = dimension.trim();
    if dimension.is_empty() {
        anyhow::bail!("dimension must not be empty");
    }
    let mut output = String::with_capacity(dimension.len());
    for ch in dimension.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
            output.push(ch);
        } else {
            output.push('_');
        }
    }
    while output.contains("__") {
        output = output.replace("__", "_");
    }
    let output = output.trim_matches('_').to_string();
    if output.is_empty() {
        anyhow::bail!("dimension contains no valid folder characters: {dimension}");
    }
    if output == "." || output == ".." {
        anyhow::bail!("dimension resolves to invalid folder path: {dimension}");
    }
    if Path::new(&output).components().count() != 1 {
        anyhow::bail!("dimension folder is not a single path segment: {dimension}");
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_world() -> World {
        World::default()
    }

    #[test]
    fn sanitize_dimension_folder_normalizes_resource_key() {
        let folder = sanitize_dimension_folder("minecraft:overworld").unwrap();
        assert_eq!(folder, "minecraft_overworld");
    }

    #[test]
    fn tick_dimension_time_respects_fixed_and_cycle() {
        let temp = tempfile::tempdir().unwrap();
        let mut world = default_world();
        world.dimension = "minecraft:overworld".to_string();
        let manager = WorldRulesManager {
            inner: Arc::new(WorldRulesInner {
                base_dir: temp.path().join("worlds"),
                default_world: world,
                states: RwLock::new(HashMap::new()),
            }),
        };
        manager.ensure_loaded("minecraft:overworld").unwrap();

        let start = manager.current_time("minecraft:overworld");
        assert_eq!(start, 0);
        let next = manager.tick_dimension_time("minecraft:overworld");
        assert_eq!(next, 1);

        manager
            .set_fixed_time("minecraft:overworld", Some(4000))
            .unwrap();
        assert_eq!(manager.tick_dimension_time("minecraft:overworld"), 4000);

        manager.set_fixed_time("minecraft:overworld", None).unwrap();
        manager
            .set_daylight_cycle("minecraft:overworld", false)
            .unwrap();
        assert_eq!(manager.tick_dimension_time("minecraft:overworld"), 4000);
    }
}
