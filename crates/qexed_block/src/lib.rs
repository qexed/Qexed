use std::{
    collections::{BTreeMap, HashMap},
    sync::OnceLock,
};

use anyhow::{Context as _, Result};
use qexed_packet::net_types::VarInt;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockId(String);

impl BlockId {
    pub fn new(id: impl AsRef<str>) -> Self {
        Self(normalize_identifier(id.as_ref()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for BlockId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for BlockId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl std::fmt::Display for BlockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockStateId(i32);

impl BlockStateId {
    pub fn new(id: i32) -> Self {
        Self(id)
    }

    pub fn get(self) -> i32 {
        self.0
    }
}

impl From<BlockStateId> for i32 {
    fn from(value: BlockStateId) -> Self {
        value.0
    }
}

impl From<i32> for BlockStateId {
    fn from(value: i32) -> Self {
        Self(value)
    }
}

impl From<BlockStateId> for VarInt {
    fn from(value: BlockStateId) -> Self {
        Self(value.0)
    }
}

impl From<VarInt> for BlockStateId {
    fn from(value: VarInt) -> Self {
        Self(value.0)
    }
}

pub type BlockProperties = BTreeMap<String, String>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockState {
    block: BlockId,
    properties: BlockProperties,
}

impl BlockState {
    pub fn new(block: impl Into<BlockId>) -> Self {
        Self {
            block: block.into(),
            properties: BTreeMap::new(),
        }
    }

    pub fn with_properties(
        block: impl Into<BlockId>,
        properties: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        Self {
            block: block.into(),
            properties: properties
                .into_iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
        }
    }

    pub fn block(&self) -> &BlockId {
        &self.block
    }

    pub fn properties(&self) -> &BlockProperties {
        &self.properties
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockDefinition {
    id: BlockId,
    properties: BTreeMap<String, Vec<String>>,
    default_state: BlockStateId,
}

impl BlockDefinition {
    pub fn id(&self) -> &BlockId {
        &self.id
    }

    pub fn properties(&self) -> &BTreeMap<String, Vec<String>> {
        &self.properties
    }

    pub fn default_state(&self) -> BlockStateId {
        self.default_state
    }
}

#[derive(Debug, Clone, Default)]
pub struct BlockRegistry {
    blocks: HashMap<BlockId, BlockDefinition>,
    states_by_id: HashMap<BlockStateId, BlockState>,
    state_ids: HashMap<BlockState, BlockStateId>,
}

impl BlockRegistry {
    pub fn from_qexed_registry() -> Result<Self> {
        Self::from_blocks_report(&qexed_registry::load_blocks_report()?)
    }

    pub fn cached() -> Result<&'static Self> {
        static REGISTRY: OnceLock<Result<BlockRegistry, String>> = OnceLock::new();
        REGISTRY
            .get_or_init(|| Self::from_qexed_registry().map_err(|err| format!("{err:#}")))
            .as_ref()
            .map_err(|err| anyhow::anyhow!("{err}"))
    }

    pub fn from_blocks_report(report: &Value) -> Result<Self> {
        let blocks = report
            .as_object()
            .with_context(|| "blocks report root is not an object")?;
        let mut registry = Self::default();

        for (id, value) in blocks {
            let id = BlockId::new(id);
            let report_block: ReportBlock =
                serde_json::from_value(value.clone()).with_context(|| {
                    format!(
                        "invalid block entry in Mojang blocks report: {}",
                        id.as_str()
                    )
                })?;
            let default_state = default_state_id_from_report(&report_block.states)
                .with_context(|| format!("block has no states: {}", id.as_str()))?;

            for state in report_block.states {
                let state_id = BlockStateId::new(state.id);
                let block_state = BlockState {
                    block: id.clone(),
                    properties: state.properties,
                };
                registry.states_by_id.insert(state_id, block_state.clone());
                registry.state_ids.insert(block_state, state_id);
            }

            registry.blocks.insert(
                id.clone(),
                BlockDefinition {
                    id,
                    properties: report_block.properties,
                    default_state,
                },
            );
        }

        Ok(registry)
    }

    pub fn block(&self, id: impl Into<BlockId>) -> Option<&BlockDefinition> {
        self.blocks.get(&id.into())
    }

    pub fn default_state_id(&self, id: impl Into<BlockId>) -> Option<BlockStateId> {
        self.block(id).map(BlockDefinition::default_state)
    }

    pub fn state_id(&self, state: &BlockState) -> Option<BlockStateId> {
        if state.properties.is_empty() {
            return self.default_state_id(state.block.clone());
        }

        self.state_ids.get(state).copied()
    }

    pub fn state_by_id(&self, id: impl Into<BlockStateId>) -> Option<&BlockState> {
        self.states_by_id.get(&id.into())
    }
}

pub fn default_state_id(id: impl Into<BlockId>) -> Result<BlockStateId> {
    let id = id.into();
    BlockRegistry::cached()?
        .default_state_id(id.clone())
        .with_context(|| format!("block not found in registry: {}", id.as_str()))
}

pub fn block_state_id(state: &BlockState) -> Result<BlockStateId> {
    BlockRegistry::cached()?
        .state_id(state)
        .with_context(|| format!("block state not found in registry: {:?}", state))
}

pub fn block_state_varint(state: &BlockState) -> Result<VarInt> {
    Ok(block_state_id(state)?.into())
}

fn normalize_identifier(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.contains(':') {
        trimmed.to_string()
    } else {
        format!("minecraft:{trimmed}")
    }
}

fn default_state_id_from_report(states: &[ReportState]) -> Option<BlockStateId> {
    states
        .iter()
        .find(|state| state.default)
        .or_else(|| states.first())
        .map(|state| BlockStateId::new(state.id))
}

#[derive(Debug, Deserialize)]
struct ReportBlock {
    #[serde(default)]
    properties: BTreeMap<String, Vec<String>>,
    states: Vec<ReportState>,
}

#[derive(Debug, Deserialize)]
struct ReportState {
    id: i32,
    #[serde(default)]
    default: bool,
    #[serde(default)]
    properties: BlockProperties,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn registry_resolves_default_and_property_state_ids() {
        let registry = BlockRegistry::from_blocks_report(&json!({
            "minecraft:oak_log": {
                "properties": {
                    "axis": ["x", "y", "z"]
                },
                "states": [
                    {"id": 10, "properties": {"axis": "x"}},
                    {"id": 11, "properties": {"axis": "y"}, "default": true}
                ]
            }
        }))
        .unwrap();

        assert_eq!(
            registry.default_state_id("oak_log").unwrap(),
            BlockStateId::new(11)
        );
        assert_eq!(
            registry
                .state_id(&BlockState::with_properties(
                    "minecraft:oak_log",
                    [("axis", "x")]
                ))
                .unwrap(),
            BlockStateId::new(10)
        );
    }

    #[test]
    fn block_state_id_converts_to_packet_varint() {
        let packet_id: VarInt = BlockStateId::new(42).into();

        assert_eq!(packet_id, VarInt(42));
        assert_eq!(BlockStateId::from(packet_id), BlockStateId::new(42));
    }
}
