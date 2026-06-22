use std::collections::HashMap;
use std::sync::Arc;

use qexed_nbt::Tag;
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
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ItemStack {
    pub item: ItemId,
    pub count: u8,
    pub max_stack_size: u8,
    pub components: ItemComponents,
}

impl ItemStack {
    pub fn new(item: ItemId, count: u8, max_stack_size: u8) -> Result<Self, ItemError> {
        Self::with_components(item, count, max_stack_size, ItemComponents::default())
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

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ItemComponents {
    inner: HashMap<String, Tag>,
}

impl ItemComponents {
    pub fn new(inner: HashMap<String, Tag>) -> Self {
        Self { inner }
    }

    pub fn get(&self, key: &str) -> Option<&Tag> {
        self.inner.get(key)
    }

    pub fn insert(&mut self, key: impl Into<String>, value: Tag) -> Option<Tag> {
        self.inner.insert(key.into(), value)
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn as_map(&self) -> &HashMap<String, Tag> {
        &self.inner
    }
}

#[derive(Debug, Clone, Default)]
pub struct ItemRegistry {
    items: HashMap<ItemId, Arc<ItemDefinition>>,
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

    pub fn stack(&self, id: &ItemId, count: u8) -> Option<Result<ItemStack, ItemError>> {
        self.get(id).map(|item| item.stack(count))
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
    use super::{DEFAULT_MAX_STACK_SIZE, ItemDefinition, ItemError, ItemId, ItemRegistry};

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

        let stack = registry.stack(&id, 32).unwrap().unwrap();

        assert_eq!(stack.item, id);
        assert_eq!(stack.count, 32);
        assert_eq!(stack.max_stack_size, DEFAULT_MAX_STACK_SIZE);
    }
}
