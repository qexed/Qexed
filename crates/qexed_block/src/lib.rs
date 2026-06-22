use std::{
    collections::{BTreeMap, HashMap},
    sync::OnceLock,
};

use anyhow::{Context as _, Result};
use qexed_packet::net_types::{Position, VarInt};
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

    pub fn property(&self, name: &str) -> Option<&str> {
        self.properties.get(name).map(String::as_str)
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

    pub fn property_values(&self, name: &str) -> Option<&[String]> {
        self.properties.get(name).map(Vec::as_slice)
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

    pub fn contains_block(&self, id: impl Into<BlockId>) -> bool {
        self.blocks.contains_key(&id.into())
    }

    pub fn blocks(&self) -> impl Iterator<Item = &BlockDefinition> {
        self.blocks.values()
    }

    pub fn states(&self) -> impl Iterator<Item = (BlockStateId, &BlockState)> {
        self.states_by_id.iter().map(|(id, state)| (*id, state))
    }

    pub fn default_state_id(&self, id: impl Into<BlockId>) -> Option<BlockStateId> {
        self.block(id).map(BlockDefinition::default_state)
    }

    pub fn default_state(&self, id: impl Into<BlockId>) -> Option<&BlockState> {
        self.default_state_id(id)
            .and_then(|state_id| self.state_by_id(state_id))
    }

    pub fn state_id(&self, state: &BlockState) -> Option<BlockStateId> {
        if state.properties.is_empty() {
            return self.default_state_id(state.block.clone());
        }

        self.state_ids.get(state).copied()
    }

    pub fn state_id_by_name(
        &self,
        block: impl Into<BlockId>,
        properties: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Option<BlockStateId> {
        self.state_id(&BlockState::with_properties(block, properties))
    }

    pub fn state_by_id(&self, id: impl Into<BlockStateId>) -> Option<&BlockState> {
        self.states_by_id.get(&id.into())
    }

    pub fn definition_by_state_id(
        &self,
        id: impl Into<BlockStateId>,
    ) -> Option<(&BlockDefinition, &BlockState)> {
        let state = self.state_by_id(id)?;
        let definition = self.block(state.block.clone())?;
        Some((definition, state))
    }

    pub fn validate_state_id(&self, id: impl Into<BlockStateId>) -> BlockValidationResult {
        let state_id = id.into();
        if self.states_by_id.contains_key(&state_id) {
            BlockValidationResult::Allowed
        } else {
            BlockValidationResult::Denied(BlockValidationError::UnknownBlockState(state_id))
        }
    }

    pub fn validate_state(&self, state: &BlockState) -> BlockValidationResult {
        if self.state_id(state).is_some() {
            BlockValidationResult::Allowed
        } else {
            BlockValidationResult::Denied(BlockValidationError::UnknownBlockStateName(
                state.clone(),
            ))
        }
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

pub fn block_state_id_by_name(
    block: impl Into<BlockId>,
    properties: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
) -> Result<BlockStateId> {
    let state = BlockState::with_properties(block, properties);
    block_state_id(&state)
}

pub fn block_state_varint(state: &BlockState) -> Result<VarInt> {
    Ok(block_state_id(state)?.into())
}

pub fn block_state(id: impl Into<BlockStateId>) -> Result<BlockState> {
    let id = id.into();
    BlockRegistry::cached()?
        .state_by_id(id)
        .cloned()
        .with_context(|| format!("block state id not found in registry: {}", id.get()))
}

pub fn is_air_block(id: impl AsRef<str>) -> bool {
    matches!(
        normalize_identifier(id.as_ref()).as_str(),
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    )
}

pub fn block_state_is_air(state: &BlockState) -> bool {
    is_air_block(state.block.as_str())
}

pub fn block_state_id_is_air(id: impl Into<BlockStateId>) -> Result<bool> {
    Ok(block_state_is_air(&block_state(id)?))
}

pub fn block_state_has_fluid(state: &BlockState) -> bool {
    state.block.as_str() == "minecraft:water"
        || state.block.as_str() == "minecraft:lava"
        || is_always_water_filled_block(state.block.as_str())
        || state.property("waterlogged") == Some("true")
}

pub fn block_state_id_has_fluid(id: impl Into<BlockStateId>) -> Result<bool> {
    Ok(block_state_has_fluid(&block_state(id)?))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InteractionHand {
    MainHand,
    OffHand,
}

impl InteractionHand {
    pub fn from_protocol_id(id: i32) -> Option<Self> {
        match id {
            0 => Some(Self::MainHand),
            1 => Some(Self::OffHand),
            _ => None,
        }
    }

    pub fn protocol_id(self) -> i32 {
        match self {
            Self::MainHand => 0,
            Self::OffHand => 1,
        }
    }
}

impl From<InteractionHand> for VarInt {
    fn from(value: InteractionHand) -> Self {
        Self(value.protocol_id())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockFace {
    Bottom,
    Top,
    North,
    South,
    West,
    East,
}

impl BlockFace {
    pub fn from_protocol_id(id: i32) -> Option<Self> {
        match id {
            0 => Some(Self::Bottom),
            1 => Some(Self::Top),
            2 => Some(Self::North),
            3 => Some(Self::South),
            4 => Some(Self::West),
            5 => Some(Self::East),
            _ => None,
        }
    }

    pub fn protocol_id(self) -> i32 {
        match self {
            Self::Bottom => 0,
            Self::Top => 1,
            Self::North => 2,
            Self::South => 3,
            Self::West => 4,
            Self::East => 5,
        }
    }

    pub fn adjacent_position(self, position: &Position) -> Position {
        let mut adjacent = position.clone();
        match self {
            Self::Bottom => adjacent.y -= 1,
            Self::Top => adjacent.y += 1,
            Self::North => adjacent.z -= 1,
            Self::South => adjacent.z += 1,
            Self::West => adjacent.x -= 1,
            Self::East => adjacent.x += 1,
        }
        adjacent
    }
}

impl From<BlockFace> for VarInt {
    fn from(value: BlockFace) -> Self {
        Self(value.protocol_id())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockActionKind {
    Place,
    Break,
    Interact,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockHit {
    pub face: BlockFace,
    pub cursor_x: f32,
    pub cursor_y: f32,
    pub cursor_z: f32,
    pub inside_block: bool,
    pub world_border_hit: bool,
}

impl BlockHit {
    pub fn new(face: BlockFace, cursor_x: f32, cursor_y: f32, cursor_z: f32) -> Self {
        Self {
            face,
            cursor_x,
            cursor_y,
            cursor_z,
            inside_block: false,
            world_border_hit: false,
        }
    }

    pub fn validate(&self) -> BlockValidationResult {
        if self.world_border_hit {
            return BlockValidationResult::Denied(BlockValidationError::WorldBorderHit);
        }

        if valid_cursor_coordinate(self.cursor_x)
            && valid_cursor_coordinate(self.cursor_y)
            && valid_cursor_coordinate(self.cursor_z)
        {
            BlockValidationResult::Allowed
        } else {
            BlockValidationResult::Denied(BlockValidationError::InvalidHitCursor)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockPlacementRequest {
    pub position: Position,
    pub placed_state: BlockStateId,
    pub replaced_state: Option<BlockStateId>,
    pub hand: InteractionHand,
    pub hit: BlockHit,
    pub sequence: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockBreakRequest {
    pub position: Position,
    pub current_state: Option<BlockStateId>,
    pub face: BlockFace,
    pub sequence: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockInteractEvent {
    pub position: Position,
    pub state: Option<BlockStateId>,
    pub hand: InteractionHand,
    pub hit: BlockHit,
    pub sequence: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockChangeEvent {
    pub kind: BlockActionKind,
    pub position: Position,
    pub previous_state: Option<BlockStateId>,
    pub new_state: Option<BlockStateId>,
    pub sequence: Option<i32>,
}

impl BlockPlacementRequest {
    pub fn target_position(&self) -> Position {
        self.hit.face.adjacent_position(&self.position)
    }

    pub fn validate(&self, registry: &BlockRegistry) -> BlockValidationResult {
        if let BlockValidationResult::Denied(reason) = validate_world_position(&self.position) {
            return BlockValidationResult::Denied(reason);
        }
        if let BlockValidationResult::Denied(reason) = self.hit.validate() {
            return BlockValidationResult::Denied(reason);
        }
        if let BlockValidationResult::Denied(reason) = registry.validate_state_id(self.placed_state)
        {
            return BlockValidationResult::Denied(reason);
        }
        if let Some(replaced_state) = self.replaced_state {
            if let BlockValidationResult::Denied(reason) =
                registry.validate_state_id(replaced_state)
            {
                return BlockValidationResult::Denied(reason);
            }
            if !is_replaceable_state(registry, replaced_state) {
                return BlockValidationResult::Denied(BlockValidationError::TargetNotReplaceable(
                    replaced_state,
                ));
            }
        }
        BlockValidationResult::Allowed
    }
}

impl BlockBreakRequest {
    pub fn validate(&self, registry: &BlockRegistry) -> BlockValidationResult {
        if let BlockValidationResult::Denied(reason) = validate_world_position(&self.position) {
            return BlockValidationResult::Denied(reason);
        }
        if let Some(current_state) = self.current_state {
            if let BlockValidationResult::Denied(reason) = registry.validate_state_id(current_state)
            {
                return BlockValidationResult::Denied(reason);
            }
            if is_air_state(registry, current_state) {
                return BlockValidationResult::Denied(BlockValidationError::TargetIsAir);
            }
        }
        BlockValidationResult::Allowed
    }
}

impl BlockInteractEvent {
    pub fn validate(&self, registry: &BlockRegistry) -> BlockValidationResult {
        if let BlockValidationResult::Denied(reason) = validate_world_position(&self.position) {
            return BlockValidationResult::Denied(reason);
        }
        if let BlockValidationResult::Denied(reason) = self.hit.validate() {
            return BlockValidationResult::Denied(reason);
        }
        if let Some(state) = self.state
            && let BlockValidationResult::Denied(reason) = registry.validate_state_id(state)
        {
            return BlockValidationResult::Denied(reason);
        }
        BlockValidationResult::Allowed
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockValidationResult {
    Allowed,
    Denied(BlockValidationError),
}

impl BlockValidationResult {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }

    pub fn denied_reason(&self) -> Option<&BlockValidationError> {
        match self {
            Self::Allowed => None,
            Self::Denied(reason) => Some(reason),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockValidationError {
    UnknownBlockState(BlockStateId),
    UnknownBlockStateName(BlockState),
    PositionOutOfBounds { y: i32 },
    InvalidHitCursor,
    WorldBorderHit,
    TargetIsAir,
    TargetNotReplaceable(BlockStateId),
}

fn normalize_identifier(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.contains(':') {
        trimmed.to_string()
    } else {
        format!("minecraft:{trimmed}")
    }
}

fn is_replaceable_state(registry: &BlockRegistry, state_id: BlockStateId) -> bool {
    registry
        .state_by_id(state_id)
        .is_some_and(|state| block_state_is_air(state) || block_state_has_fluid(state))
}

fn is_air_state(registry: &BlockRegistry, state_id: BlockStateId) -> bool {
    registry
        .state_by_id(state_id)
        .is_some_and(block_state_is_air)
}

fn validate_world_position(position: &Position) -> BlockValidationResult {
    const VANILLA_MIN_Y: i32 = -64;
    const VANILLA_MAX_Y: i32 = 319;

    if (VANILLA_MIN_Y..=VANILLA_MAX_Y).contains(&position.y) {
        BlockValidationResult::Allowed
    } else {
        BlockValidationResult::Denied(BlockValidationError::PositionOutOfBounds { y: position.y })
    }
}

fn valid_cursor_coordinate(value: f32) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn is_always_water_filled_block(name: &str) -> bool {
    matches!(
        name,
        "minecraft:bubble_column"
            | "minecraft:kelp"
            | "minecraft:kelp_plant"
            | "minecraft:seagrass"
            | "minecraft:tall_seagrass"
    )
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

    #[test]
    fn registry_resolves_state_id_from_vanilla_block_name_and_properties() {
        let registry = BlockRegistry::from_blocks_report(&json!({
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

        assert_eq!(
            registry
                .state_id_by_name("oak_log", [("axis", "z")])
                .unwrap(),
            BlockStateId::new(12)
        );
        assert_eq!(
            registry
                .state_id_by_name("minecraft:oak_log", [] as [(&str, &str); 0])
                .unwrap(),
            BlockStateId::new(11)
        );
        assert_eq!(
            registry.state_id_by_name("oak_log", [("axis", "north")]),
            None
        );
    }

    #[test]
    fn registry_exposes_state_lookup_and_metadata_iteration() {
        let registry = BlockRegistry::from_blocks_report(&json!({
            "minecraft:air": {
                "states": [{"id": 0, "default": true}]
            },
            "minecraft:water": {
                "properties": {"level": ["0"]},
                "states": [{"id": 86, "properties": {"level": "0"}, "default": true}]
            }
        }))
        .unwrap();

        assert!(registry.contains_block("air"));
        assert_eq!(registry.blocks().count(), 2);
        assert_eq!(registry.states().count(), 2);
        assert_eq!(
            registry.default_state("water").unwrap().property("level"),
            Some("0")
        );

        let (definition, state) = registry
            .definition_by_state_id(BlockStateId::new(86))
            .unwrap();
        assert_eq!(definition.id().as_str(), "minecraft:water");
        assert!(block_state_has_fluid(state));
    }

    #[test]
    fn block_action_validation_rejects_unknown_states_and_solid_replacement() {
        let registry = BlockRegistry::from_blocks_report(&json!({
            "minecraft:air": {
                "states": [{"id": 0, "default": true}]
            },
            "minecraft:stone": {
                "states": [{"id": 1, "default": true}]
            }
        }))
        .unwrap();

        let request = BlockPlacementRequest {
            position: Position { x: 0, y: 64, z: 0 },
            placed_state: BlockStateId::new(1),
            replaced_state: Some(BlockStateId::new(0)),
            hand: InteractionHand::MainHand,
            hit: BlockHit::new(BlockFace::Top, 0.5, 1.0, 0.5),
            sequence: Some(7),
        };

        assert_eq!(request.validate(&registry), BlockValidationResult::Allowed);
        assert_eq!(request.target_position(), Position { x: 0, y: 65, z: 0 });

        let mut blocked = request.clone();
        blocked.replaced_state = Some(BlockStateId::new(1));
        assert_eq!(
            blocked.validate(&registry),
            BlockValidationResult::Denied(BlockValidationError::TargetNotReplaceable(
                BlockStateId::new(1)
            ))
        );

        let mut unknown = request;
        unknown.placed_state = BlockStateId::new(99);
        assert_eq!(
            unknown.validate(&registry),
            BlockValidationResult::Denied(BlockValidationError::UnknownBlockState(
                BlockStateId::new(99)
            ))
        );
    }

    #[test]
    fn block_break_and_interact_validation_cover_air_bounds_and_hit_cursor() {
        let registry = BlockRegistry::from_blocks_report(&json!({
            "minecraft:air": {
                "states": [{"id": 0, "default": true}]
            },
            "minecraft:stone": {
                "states": [{"id": 1, "default": true}]
            }
        }))
        .unwrap();

        let air_break = BlockBreakRequest {
            position: Position { x: 0, y: 64, z: 0 },
            current_state: Some(BlockStateId::new(0)),
            face: BlockFace::Top,
            sequence: None,
        };
        assert_eq!(
            air_break.validate(&registry),
            BlockValidationResult::Denied(BlockValidationError::TargetIsAir)
        );

        let bad_y = BlockInteractEvent {
            position: Position { x: 0, y: 400, z: 0 },
            state: Some(BlockStateId::new(1)),
            hand: InteractionHand::OffHand,
            hit: BlockHit::new(BlockFace::North, 1.2, 0.5, 0.5),
            sequence: None,
        };
        assert_eq!(
            bad_y.validate(&registry),
            BlockValidationResult::Denied(BlockValidationError::PositionOutOfBounds { y: 400 })
        );

        let bad_cursor = BlockInteractEvent {
            position: Position { x: 0, y: 64, z: 0 },
            state: Some(BlockStateId::new(1)),
            hand: InteractionHand::OffHand,
            hit: BlockHit::new(BlockFace::North, 1.2, 0.5, 0.5),
            sequence: None,
        };
        assert_eq!(
            bad_cursor.validate(&registry),
            BlockValidationResult::Denied(BlockValidationError::InvalidHitCursor)
        );

        assert_eq!(
            InteractionHand::from_protocol_id(1),
            Some(InteractionHand::OffHand)
        );
        assert_eq!(BlockFace::from_protocol_id(4), Some(BlockFace::West));
    }
}
