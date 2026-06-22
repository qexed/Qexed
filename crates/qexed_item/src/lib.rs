use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const DEFAULT_MAX_STACK_SIZE: u8 = 64;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ItemError {
    #[error("invalid item id: {0}")]
    InvalidItemId(String),
    #[error("invalid max stack size: {0}")]
    InvalidMaxStackSize(u8),
    #[error("invalid item count {count}, max {max}")]
    InvalidCount { count: u8, max: u8 },
    #[error("item not found: {0}")]
    NotFound(ItemId),
    #[error("incompatible item stacks")]
    IncompatibleStack,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ItemId(String);

impl ItemId {
    pub fn new(value: impl Into<String>) -> Result<Self, ItemError> {
        let value = value.into();
        if is_valid_identifier(&value) {
            Ok(Self(value))
        } else {
            Err(ItemError::InvalidItemId(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ItemId {
    type Error = ItemError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for ItemId {
    type Error = ItemError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl AsRef<str> for ItemId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for ItemId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemDefinition {
    pub id: ItemId,
    #[serde(default = "default_max_stack_size")]
    pub max_stack_size: u8,
}

impl ItemDefinition {
    pub fn new(id: ItemId, max_stack_size: u8) -> Result<Self, ItemError> {
        if max_stack_size == 0 {
            return Err(ItemError::InvalidMaxStackSize(max_stack_size));
        }

        Ok(Self { id, max_stack_size })
    }

    pub fn stack(&self, count: u8) -> Result<ItemStack, ItemError> {
        ItemStack::new(self.id.clone(), count, self.max_stack_size)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemStack {
    pub item: ItemId,
    pub count: u8,
    pub max_stack_size: u8,
    #[serde(default, skip_serializing_if = "ItemComponents::is_empty")]
    pub components: ItemComponents,
}

impl ItemStack {
    pub fn new(item: ItemId, count: u8, max_stack_size: u8) -> Result<Self, ItemError> {
        Self::with_components(item, count, max_stack_size, ItemComponents::default())
    }

    pub fn empty(item: ItemId, max_stack_size: u8) -> Result<Self, ItemError> {
        Self::new(item, 0, max_stack_size)
    }

    pub fn with_components(
        item: ItemId,
        count: u8,
        max_stack_size: u8,
        components: ItemComponents,
    ) -> Result<Self, ItemError> {
        validate_stack_size(count, max_stack_size)?;
        Ok(Self {
            item,
            count,
            max_stack_size,
            components,
        })
    }

    pub fn set_count(&mut self, count: u8) -> Result<(), ItemError> {
        validate_stack_size(count, self.max_stack_size)?;
        self.count = count;
        Ok(())
    }

    pub fn set_count_clamped(&mut self, count: u8) {
        self.count = count.min(self.max_stack_size);
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn remaining_capacity(&self) -> u8 {
        self.max_stack_size.saturating_sub(self.count)
    }

    pub fn can_merge_with(&self, other: &Self) -> bool {
        self.is_empty()
            || other.is_empty()
            || (self.item == other.item
                && self.max_stack_size == other.max_stack_size
                && self.components == other.components)
    }

    pub fn merge_from(&mut self, other: &mut Self) -> Result<u8, ItemError> {
        if other.is_empty() {
            return Ok(0);
        }

        if self.is_empty() {
            self.item = other.item.clone();
            self.max_stack_size = other.max_stack_size;
            self.components = other.components.clone();
        } else if !self.can_merge_with(other) {
            return Err(ItemError::IncompatibleStack);
        }

        let moved = self.remaining_capacity().min(other.count);
        self.count += moved;
        other.count -= moved;
        Ok(moved)
    }

    pub fn split(&mut self, count: u8) -> Result<Self, ItemError> {
        let split_count = count.min(self.count);
        self.count -= split_count;
        Self::with_components(
            self.item.clone(),
            split_count,
            self.max_stack_size,
            self.components.clone(),
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ItemComponents {
    inner: BTreeMap<String, ComponentValue>,
}

impl ItemComponents {
    pub fn new(inner: BTreeMap<String, ComponentValue>) -> Self {
        Self { inner }
    }

    pub fn get(&self, key: &str) -> Option<&ComponentValue> {
        self.inner.get(key)
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.get(key) {
            Some(ComponentValue::Bool(value)) => Some(*value),
            _ => None,
        }
    }

    pub fn get_i64(&self, key: &str) -> Option<i64> {
        match self.get(key) {
            Some(ComponentValue::I64(value)) => Some(*value),
            _ => None,
        }
    }

    pub fn get_f64(&self, key: &str) -> Option<f64> {
        match self.get(key) {
            Some(ComponentValue::F64(value)) => Some(*value),
            _ => None,
        }
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        match self.get(key) {
            Some(ComponentValue::String(value)) => Some(value),
            _ => None,
        }
    }

    pub fn insert(
        &mut self,
        key: impl Into<String>,
        value: ComponentValue,
    ) -> Option<ComponentValue> {
        self.inner.insert(key.into(), value)
    }

    pub fn remove(&mut self, key: &str) -> Option<ComponentValue> {
        self.inner.remove(key)
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn as_map(&self) -> &BTreeMap<String, ComponentValue> {
        &self.inner
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ComponentValue {
    Bool(bool),
    I64(i64),
    F64(f64),
    String(String),
    List(Vec<ComponentValue>),
    Compound(BTreeMap<String, ComponentValue>),
}

pub type ItemNbt = BTreeMap<String, ComponentValue>;

#[derive(Debug, Clone, Default)]
pub struct ItemRegistry {
    items: BTreeMap<ItemId, Arc<ItemDefinition>>,
}

impl ItemRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, item: ItemDefinition) -> Option<Arc<ItemDefinition>> {
        self.items.insert(item.id.clone(), Arc::new(item))
    }

    pub fn get(&self, id: &ItemId) -> Option<&ItemDefinition> {
        self.items.get(id).map(Arc::as_ref)
    }

    pub fn get_by_str(&self, id: &str) -> Option<&ItemDefinition> {
        let id = ItemId::new(id).ok()?;
        self.get(&id)
    }

    pub fn stack(&self, id: &ItemId, count: u8) -> Result<ItemStack, ItemError> {
        self.get(id)
            .ok_or_else(|| ItemError::NotFound(id.clone()))?
            .stack(count)
    }

    pub fn stack_by_str(&self, id: &str, count: u8) -> Result<ItemStack, ItemError> {
        let id = ItemId::new(id)?;
        self.stack(&id, count)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

fn validate_stack_size(count: u8, max_stack_size: u8) -> Result<(), ItemError> {
    if max_stack_size == 0 {
        return Err(ItemError::InvalidMaxStackSize(max_stack_size));
    }
    if count > max_stack_size {
        return Err(ItemError::InvalidCount {
            count,
            max: max_stack_size,
        });
    }
    Ok(())
}

fn default_max_stack_size() -> u8 {
    DEFAULT_MAX_STACK_SIZE
}

fn is_valid_identifier(value: &str) -> bool {
    let Some((namespace, path)) = value.split_once(':') else {
        return false;
    };

    !namespace.is_empty()
        && !path.is_empty()
        && namespace
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        && path.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'_' | b'-' | b'/' | b'.')
        })
}

#[cfg(test)]
mod tests {
    use super::{
        ComponentValue, DEFAULT_MAX_STACK_SIZE, ItemComponents, ItemDefinition, ItemError, ItemId,
        ItemRegistry, ItemStack,
    };

    #[test]
    fn validates_item_id_shape() {
        assert!(ItemId::new("minecraft:stone").is_ok());
        assert!(ItemId::new("minecraft:tools/iron_pickaxe").is_ok());
        assert_eq!(
            ItemId::new("Stone").unwrap_err(),
            ItemError::InvalidItemId("Stone".to_string())
        );
    }

    #[test]
    fn rejects_stack_count_above_item_limit() {
        let item = ItemDefinition::new(ItemId::new("minecraft:egg").unwrap(), 16).unwrap();

        assert!(item.stack(16).is_ok());
        assert_eq!(
            item.stack(17).unwrap_err(),
            ItemError::InvalidCount { count: 17, max: 16 }
        );
    }

    #[test]
    fn registry_builds_stack_from_definition() {
        let id = ItemId::new("minecraft:stone").unwrap();
        let mut registry = ItemRegistry::new();
        registry.register(ItemDefinition::new(id.clone(), DEFAULT_MAX_STACK_SIZE).unwrap());

        let stack = registry.stack(&id, 32).unwrap();

        assert_eq!(stack.item, id);
        assert_eq!(stack.count, 32);
        assert_eq!(stack.max_stack_size, DEFAULT_MAX_STACK_SIZE);
    }

    #[test]
    fn registry_reports_missing_items() {
        let registry = ItemRegistry::new();
        let id = ItemId::new("minecraft:missing").unwrap();

        assert_eq!(registry.stack(&id, 1).unwrap_err(), ItemError::NotFound(id));
    }

    #[test]
    fn components_hold_serializable_placeholders() {
        let mut components = ItemComponents::default();
        components.insert(
            "minecraft:custom_name",
            ComponentValue::String("Stone".into()),
        );

        assert_eq!(
            components.get("minecraft:custom_name"),
            Some(&ComponentValue::String("Stone".into()))
        );
    }

    #[test]
    fn clamps_stack_count_to_capacity() {
        let id = ItemId::new("minecraft:egg").unwrap();
        let mut stack = ItemStack::new(id, 3, 16).unwrap();

        stack.set_count_clamped(20);

        assert_eq!(stack.count, 16);
        assert_eq!(stack.remaining_capacity(), 0);
    }

    #[test]
    fn splits_stack_without_exceeding_available_count() {
        let id = ItemId::new("minecraft:stone").unwrap();
        let mut stack = ItemStack::new(id.clone(), 10, DEFAULT_MAX_STACK_SIZE).unwrap();

        let split = stack.split(64).unwrap();

        assert_eq!(split.item, id);
        assert_eq!(split.count, 10);
        assert_eq!(stack.count, 0);
        assert!(stack.is_empty());
    }

    #[test]
    fn merges_compatible_stacks_up_to_capacity() {
        let id = ItemId::new("minecraft:stone").unwrap();
        let mut target = ItemStack::new(id.clone(), 60, DEFAULT_MAX_STACK_SIZE).unwrap();
        let mut source = ItemStack::new(id, 8, DEFAULT_MAX_STACK_SIZE).unwrap();

        let moved = target.merge_from(&mut source).unwrap();

        assert_eq!(moved, 4);
        assert_eq!(target.count, DEFAULT_MAX_STACK_SIZE);
        assert_eq!(source.count, 4);
    }

    #[test]
    fn empty_stack_adopts_merged_item_identity() {
        let mut empty = ItemStack::empty(
            ItemId::new("minecraft:air").unwrap(),
            DEFAULT_MAX_STACK_SIZE,
        )
        .unwrap();
        let mut source = ItemStack::new(ItemId::new("minecraft:dirt").unwrap(), 12, 16).unwrap();

        let moved = empty.merge_from(&mut source).unwrap();

        assert_eq!(moved, 12);
        assert_eq!(empty.item.as_str(), "minecraft:dirt");
        assert_eq!(empty.max_stack_size, 16);
        assert_eq!(empty.count, 12);
        assert!(source.is_empty());
    }

    #[test]
    fn rejects_merge_with_different_components() {
        let id = ItemId::new("minecraft:stone").unwrap();
        let mut components = ItemComponents::default();
        components.insert("minecraft:custom_name", ComponentValue::String("A".into()));
        let mut target =
            ItemStack::with_components(id.clone(), 1, DEFAULT_MAX_STACK_SIZE, components).unwrap();
        let mut source = ItemStack::new(id, 1, DEFAULT_MAX_STACK_SIZE).unwrap();

        assert_eq!(
            target.merge_from(&mut source).unwrap_err(),
            ItemError::IncompatibleStack
        );
        assert_eq!(target.count, 1);
        assert_eq!(source.count, 1);
    }

    #[test]
    fn components_expose_typed_helpers() {
        let mut components = ItemComponents::default();
        components.insert("minecraft:unbreakable", ComponentValue::Bool(true));
        components.insert("minecraft:damage", ComponentValue::I64(7));
        components.insert("qexed:weight", ComponentValue::F64(1.5));
        components.insert(
            "minecraft:custom_name",
            ComponentValue::String("Pick".into()),
        );

        assert_eq!(components.get_bool("minecraft:unbreakable"), Some(true));
        assert_eq!(components.get_i64("minecraft:damage"), Some(7));
        assert_eq!(components.get_f64("qexed:weight"), Some(1.5));
        assert_eq!(components.get_str("minecraft:custom_name"), Some("Pick"));
        assert_eq!(components.get_str("minecraft:damage"), None);
    }
}
