use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use anyhow::{Context, Result};

/// A block state entry with name and properties.
#[derive(Debug, Clone)]
pub struct BlockStateEntry {
    pub name: String,
    pub properties: Vec<(String, String)>,
}

struct BlockRegistry {
    /// Key: "name|prop1=val1;prop2=val2;" → block_state_id
    id_by_state: HashMap<String, i32>,
    /// block_state_id → BlockStateEntry
    state_by_id: HashMap<i32, BlockStateEntry>,
    /// Block name → default block_state_id
    default_by_name: HashMap<String, i32>,
}

fn registry() -> &'static BlockRegistry {
    static REGISTRY: OnceLock<BlockRegistry> = OnceLock::new();
    REGISTRY.get_or_init(BlockRegistry::fallback)
}

impl BlockRegistry {
    fn fallback() -> Self {
        Self {
            id_by_state: HashMap::new(),
            state_by_id: HashMap::new(),
            default_by_name: HashMap::new(),
        }
    }

    fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read block registry: {}", path.display()))?;
        let blocks: serde_json::Value = serde_json::from_str(&content)
            .with_context(|| "failed to parse block registry JSON")?;

        let blocks_obj = blocks
            .as_object()
            .context("block report root is not an object")?;

        let mut id_by_state = HashMap::new();
        let mut state_by_id = HashMap::new();
        let mut default_by_name = HashMap::new();

        for (name, block) in blocks_obj {
            let Some(states) = block.get("states").and_then(|s| s.as_array()) else {
                continue;
            };

            for state in states {
                let Some(id) = state.get("id").and_then(|v| v.as_i64()) else {
                    continue;
                };
                let Ok(id) = i32::try_from(id) else {
                    continue;
                };

                let properties = state
                    .get("properties")
                    .and_then(|p| p.as_object())
                    .map(|props| {
                        let mut pairs: Vec<(String, String)> = props
                            .iter()
                            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                            .collect();
                        pairs.sort_by(|a, b| a.0.cmp(&b.0));
                        pairs
                    })
                    .unwrap_or_default();

                id_by_state.insert(state_key(name, &properties), id);
                state_by_id.insert(
                    id,
                    BlockStateEntry {
                        name: name.clone(),
                        properties: properties.clone(),
                    },
                );

                if state
                    .get("default")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    default_by_name.insert(name.clone(), id);
                }
            }

            // If no default was found, use the first state
            if !default_by_name.contains_key(name) {
                if let Some(state) = states.first() {
                    if let Some(id) = state.get("id").and_then(|v| v.as_i64()) {
                        if let Ok(id) = i32::try_from(id) {
                            default_by_name.insert(name.clone(), id);
                        }
                    }
                }
            }
        }

        Ok(Self {
            id_by_state,
            state_by_id,
            default_by_name,
        })
    }

    fn lookup_id(&self, name: &str, properties: &[(String, String)]) -> Option<i32> {
        let name = normalize_identifier(name);
        if properties.is_empty() {
            if let Some(&id) = self.default_by_name.get(&name) {
                return Some(id);
            }
        }
        self.id_by_state.get(&state_key(&name, properties)).copied()
    }

    fn lookup_entry(&self, id: i32) -> Option<BlockStateEntry> {
        self.state_by_id.get(&id).cloned()
    }

    fn list_blocks(&self) -> Vec<(String, i32)> {
        let mut list: Vec<_> = self
            .default_by_name
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        list.sort_by(|a, b| a.0.cmp(&b.0));
        list
    }
}

fn state_key(name: &str, properties: &[(String, String)]) -> String {
    let mut key = String::from(name);
    key.push('|');
    for (prop_name, prop_val) in properties {
        key.push_str(prop_name);
        key.push('=');
        key.push_str(prop_val);
        key.push(';');
    }
    key
}

fn normalize_identifier(value: &str) -> String {
    let value = value.trim();
    if value.contains(':') {
        value.to_string()
    } else {
        format!("minecraft:{value}")
    }
}

/// Load the block registry from a blocks.json report file.
/// Returns the number of loaded blocks.
pub fn load_registry(path: &Path) -> Result<usize> {
    let reg = BlockRegistry::load(path)?;
    let count = reg.default_by_name.len();
    // Store in the global OnceLock - we need a way to update it
    // Since OnceLock can only be set once, we use a different approach
    set_global_registry(reg);
    Ok(count)
}

fn set_global_registry(reg: BlockRegistry) {
    // Use Box::leak to store the registry with static lifetime
    let leaked = Box::new(reg);
    let ptr: &'static BlockRegistry = Box::leak(leaked);
    // The static REGISTRY is already initialized with fallback,
    // so we need to use a mutable global.
    // Actually, let's use a different approach with a Mutex or RwLock.
    GLOBAL_REGISTRY.get_or_init(|| std::sync::RwLock::new(ptr));
}

static GLOBAL_REGISTRY: OnceLock<std::sync::RwLock<&'static BlockRegistry>> = OnceLock::new();

fn get_registry() -> &'static BlockRegistry {
    if let Some(lock) = GLOBAL_REGISTRY.get() {
        if let Ok(guard) = lock.read() {
            return *guard;
        }
    }
    registry()
}

/// Look up a block state ID by name and optional properties.
pub fn block_state_id(name: &str, properties: &[(String, String)]) -> Option<i32> {
    get_registry().lookup_id(name, properties)
}

/// Look up a block state entry by ID.
pub fn block_state_entry(id: i32) -> BlockStateEntry {
    get_registry()
        .lookup_entry(id)
        .unwrap_or_else(|| BlockStateEntry {
            name: "minecraft:air".to_string(),
            properties: Vec::new(),
        })
}

/// List all known block names with their default state IDs.
pub fn list_block_names() -> Vec<(String, i32)> {
    get_registry().list_blocks()
}
