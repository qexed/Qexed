//! 维度世界规则：v4 `world/rules.rs` 的 v6 迁移。
//!
//! 适配：
//! - 配置类型来自 `crate::config`（v4 的 qexed_config::app::qexed::* 在 v6 不存在）；
//! - v4 经 AppConfigTrait::load_or_create_default/save_to_config 按维度目录读写
//!   rules.toml；v6 的 qexed_config::Config 无目录参数，这里直接用 qexed_toml 按
//!   `<base_dir>/<dimension>/rules.toml` 读写（结构不变，缺省字段合并保持 v4 语义）；
//! - anyhow → `crate::error::WorldError`；加载失败告警走
//!   `qexed_language::t("qexed.world.rules.*")`。

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use crate::{
    config::{DimensionTimeRule, DimensionWorldRules, LightAlgorithm, LightMode, WorldConfig},
    error::{Result, WorldError},
};

const DEFAULT_RULES_DIR: &str = "config/worlds";
const RULES_FILE_NAME: &str = "rules.toml";

type DimensionRuleFile = DimensionWorldRules;

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
    default_world: WorldConfig,
    states: RwLock<HashMap<String, DimensionRuleState>>,
}

impl WorldRulesManager {
    pub fn from_world_config(world: &WorldConfig) -> Result<Self> {
        Self::from_world_config_with_base_dir(world, PathBuf::from(DEFAULT_RULES_DIR))
    }

    fn from_world_config_with_base_dir(world: &WorldConfig, base_dir: PathBuf) -> Result<Self> {
        let manager = Self {
            inner: Arc::new(WorldRulesInner {
                base_dir,
                default_world: world.clone(),
                states: RwLock::new(HashMap::new()),
            }),
        };
        std::fs::create_dir_all(&manager.inner.base_dir).map_err(|source| {
            WorldError::io_context(
                qexed_language::t("qexed.world.rules.create_dir_failed")
                    .replace("%{path}", &manager.inner.base_dir.display().to_string()),
                source,
            )
        })?;
        for dimension in world.configured_dimension_names() {
            manager.ensure_loaded(&dimension)?;
        }
        manager.ensure_loaded(&world.default_play_dimension())?;
        Ok(manager)
    }

    pub fn from_world_config_for_tests(
        world: &WorldConfig,
        base_dir: PathBuf,
    ) -> Result<Self> {
        Self::from_world_config_with_base_dir(world, base_dir)
    }

    pub fn snapshot(&self, dimension: &str) -> DimensionRuleSnapshot {
        if let Err(err) = self.ensure_loaded(dimension) {
            log::warn!(
                "{}",
                qexed_language::t("qexed.world.rules.load_failed")
                    .replace("%{dimension}", dimension)
                    .replace("%{error}", &format!("{err:#}"))
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
        let mut rule = load_rules_file(&rule_dir)?;
        if rule.dimension.trim().is_empty() || rule.dimension == default_world_dimension() {
            rule.dimension = dimension.to_string();
        }
        if rule.dimension_type.trim().is_empty() {
            rule.dimension_type = self
                .inner
                .default_world
                .dimension_type_for(dimension)
                .unwrap_or_else(|| default_dimension_type(dimension));
        } else if rule.dimension == dimension && rule.dimension_type == default_world_dimension_type()
        {
            if let Some(dimension_type) = self.inner.default_world.dimension_type_for(dimension) {
                rule.dimension_type = dimension_type;
            }
        }
        if rule.time.tick_step < 0 {
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
        save_rules_file(&rule_dir, rule)
    }

    fn dimension_rules_dir_path(&self, dimension: &str) -> Result<PathBuf> {
        let folder = sanitize_dimension_folder(dimension)?;
        Ok(self.inner.base_dir.join(folder))
    }
}

/// 读取（缺省合并）或创建 `<dir>/rules.toml`。
fn load_rules_file(dir: &Path) -> Result<DimensionRuleFile> {
    let path = dir.join(RULES_FILE_NAME);
    if !qexed_toml::has_file(&path)? {
        return Ok(DimensionRuleFile::default());
    }
    let loaded = qexed_toml::load_file(&path)?;
    let defaults = qexed_toml::to_document(&DimensionRuleFile::default())?;
    // 与默认值合并补齐新增字段（v4 merge_missing_default_items 语义）。
    let merged = qexed_toml::merge(&loaded, &defaults)?;
    Ok(qexed_toml::from_document(merged)?)
}

/// 写 `<dir>/rules.toml`（与旧文件合并保留未知字段）。
fn save_rules_file(dir: &Path, rule: &DimensionRuleFile) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(RULES_FILE_NAME);
    let new_doc = qexed_toml::to_document(rule)?;
    let merged = if qexed_toml::has_file(&path)? {
        let old_doc = qexed_toml::load_file(&path)?;
        qexed_toml::merge(&old_doc, &new_doc)?
    } else {
        new_doc
    };
    qexed_toml::save_file(&path, &merged)?;
    Ok(())
}

fn default_dimension_type(dimension: &str) -> String {
    match dimension {
        "minecraft:the_nether" => "minecraft:the_nether".to_string(),
        "minecraft:the_end" => "minecraft:the_end".to_string(),
        _ => "minecraft:overworld".to_string(),
    }
}

fn default_world_dimension() -> String {
    "minecraft:overworld".to_string()
}

fn default_world_dimension_type() -> String {
    "minecraft:overworld".to_string()
}

trait DimensionRuleExt {
    fn from_world_defaults(world: &WorldConfig, dimension: &str) -> Self;
}

impl DimensionRuleExt for DimensionRuleFile {
    fn from_world_defaults(world: &WorldConfig, dimension: &str) -> Self {
        Self {
            dimension: dimension.to_string(),
            dimension_type: world
                .dimension_type_for(dimension)
                .unwrap_or_else(|| default_dimension_type(dimension)),
            read_only: world.read_only,
            block_updates: true,
            light: world.light.clone(),
            light_algorithm: world.light_algorithm,
            time: DimensionTimeRule::default(),
        }
    }
}

fn sanitize_dimension_folder(dimension: &str) -> Result<String> {
    let dimension = dimension.trim();
    if dimension.is_empty() {
        return Err(WorldError::msg(
            qexed_language::t("qexed.world.rules.dimension_empty"),
        ));
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
        return Err(WorldError::msg(
            qexed_language::t("qexed.world.rules.dimension_no_valid_chars")
                .replace("%{dimension}", dimension),
        ));
    }
    if output == "." || output == ".." {
        return Err(WorldError::msg(
            qexed_language::t("qexed.world.rules.dimension_invalid_path")
                .replace("%{dimension}", dimension),
        ));
    }
    if Path::new(&output).components().count() != 1 {
        return Err(WorldError::msg(
            qexed_language::t("qexed.world.rules.dimension_not_single_segment")
                .replace("%{dimension}", dimension),
        ));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorldStorage;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "qexed_world_rules_test_{name}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn sanitize_dimension_folder_normalizes_resource_key() {
        let folder = sanitize_dimension_folder("minecraft:overworld").unwrap();
        assert_eq!(folder, "minecraft_overworld");
    }

    #[test]
    fn tick_dimension_time_respects_fixed_and_cycle() {
        let temp = temp_dir("tick");
        let mut world = WorldConfig::default();
        world.dimension = "minecraft:overworld".to_string();
        let manager = WorldRulesManager {
            inner: Arc::new(WorldRulesInner {
                base_dir: temp.join("worlds"),
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
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn world_rules_load_default_dimension_without_overworld_world() {
        let temp = temp_dir("multi");
        let mut world = WorldConfig::default();
        world.default_dimension = "qexed:mine_a".to_string();
        world.worlds = vec![WorldStorage {
            id: "mine_a".to_string(),
            dimension: "qexed:mine_a".to_string(),
            dimension_type: "minecraft:overworld".to_string(),
            path: temp.join("mine_a").to_string_lossy().into_owned(),
        }];
        world.instances.clear();
        let manager = WorldRulesManager {
            inner: Arc::new(WorldRulesInner {
                base_dir: temp.join("worlds"),
                default_world: world,
                states: RwLock::new(HashMap::new()),
            }),
        };

        manager.ensure_loaded("qexed:mine_a").unwrap();
        let snapshot = manager.snapshot("qexed:mine_a");

        assert_eq!(snapshot.dimension, "qexed:mine_a");
        assert_eq!(snapshot.dimension_type, "minecraft:overworld");
        let _ = std::fs::remove_dir_all(&temp);
    }
}
