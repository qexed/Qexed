use std::{mem, slice};

use serde::{Serialize, de::DeserializeOwned};

#[link(wasm_import_module = "qexed")]
unsafe extern "C" {
    #[link_name = "log"]
    fn host_log(ptr: i32, len: i32);
}

#[macro_export]
macro_rules! qexed_plugin_memory {
    () => {
        #[unsafe(no_mangle)]
        pub extern "C" fn qexed_plugin_alloc(len: i32) -> i32 {
            $crate::alloc(len)
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn qexed_plugin_dealloc(ptr: i32, len: i32) {
            unsafe {
                $crate::dealloc(ptr, len);
            }
        }
    };
}

pub mod payload {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct PlayerPayload {
        pub uuid: String,
        pub username: String,
        pub entity_id: i32,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct PlayerPayloadOwned {
        pub uuid: String,
        pub username: String,
        pub entity_id: i32,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ChunkPayload {
        pub dimension: String,
        pub chunk_x: i32,
        pub chunk_z: i32,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ConfigReloadPayload {
        pub path: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct LanguagePayload {
        pub language: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct ItemEnchantment {
        pub id: String,
        pub level: i32,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    pub struct PluginEnchantment {
        pub id: String,
        pub level: i32,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MiningSpeedQuery {
        pub block_state: i32,
        pub block_name: String,
        pub item_id: Option<i32>,
        pub enchantments: Vec<ItemEnchantment>,
        pub plugin_enchantments: Vec<PluginEnchantment>,
        pub speed: f32,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct MiningSpeedResponse {
        pub speed: Option<f32>,
        pub multiplier: Option<f32>,
        pub add: Option<f32>,
    }

    #[derive(Debug, Clone, Copy, Serialize, Deserialize)]
    pub struct BlockDropPosition {
        pub x: i32,
        pub y: i32,
        pub z: i32,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct BlockDropQuery {
        pub block_state: i32,
        pub block_name: String,
        pub position: BlockDropPosition,
        pub tool_item_id: Option<i32>,
        pub enchantments: Vec<ItemEnchantment>,
        pub plugin_enchantments: Vec<PluginEnchantment>,
        pub default_item_id: Option<i32>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct BlockDropResponse {
        pub replace: bool,
        pub items: Vec<BlockDropItem>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct BlockDropItem {
        pub item_id: i32,
        pub count: i32,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct PluginCommandDefinition {
        pub name: String,
        pub description_key: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct PluginCommandQuery {
        pub command: String,
        pub argument: String,
        pub player: PlayerPayloadOwned,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct PluginCommandResponse {
        pub handled: bool,
        pub actions: Vec<PlayerAction>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct NpcInteractPayload {
        pub player: PlayerPayloadOwned,
        pub entity: NpcEntityPayload,
        pub action: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct NpcEntityPayload {
        pub key: String,
        pub entity_id: i32,
        pub dimension: String,
        pub x: f64,
        pub y: f64,
        pub z: f64,
        pub yaw: f32,
        pub pitch: f32,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct NpcMutationQuery {
        pub reason: String,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct NpcMutationResponse {
        pub operations: Vec<NpcMutationOp>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum NpcMutationOp {
        Upsert { npc: NpcUpsert },
        Remove { key: String },
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct NpcUpsert {
        pub key: String,
        pub dimension: String,
        pub x: f64,
        pub y: f64,
        pub z: f64,
        pub yaw: f32,
        pub pitch: f32,
        pub name: String,
        pub display_name: String,
        pub skin_textures: String,
        pub skin_signature: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum PlayerAction {
        SystemMessage {
            text: String,
            translate: String,
            with: Vec<String>,
            overlay: bool,
        },
        Teleport {
            x: f64,
            y: f64,
            z: f64,
            yaw: Option<f32>,
            pitch: Option<f32>,
        },
        Transfer {
            host: String,
            port: u16,
            message: String,
        },
        ProxyConnect {
            server: String,
            message: String,
        },
    }
}

pub use payload::*;

pub fn alloc(len: i32) -> i32 {
    if len <= 0 {
        return 0;
    }
    let mut buffer = Vec::<u8>::with_capacity(len as usize);
    let ptr = buffer.as_mut_ptr();
    mem::forget(buffer);
    ptr as i32
}

pub unsafe fn dealloc(ptr: i32, len: i32) {
    if ptr <= 0 || len <= 0 {
        return;
    }
    unsafe {
        drop(Vec::from_raw_parts(ptr as *mut u8, 0, len as usize));
    }
}

pub fn log(message: &str) {
    unsafe {
        host_log(message.as_ptr() as i32, message.len() as i32);
    }
}

pub unsafe fn payload_bytes<'a>(ptr: i32, len: i32) -> Option<&'a [u8]> {
    if len == 0 {
        return Some(&[]);
    }
    if ptr <= 0 || len < 0 {
        return None;
    }
    Some(unsafe { slice::from_raw_parts(ptr as *const u8, len as usize) })
}

pub unsafe fn decode_payload<T: DeserializeOwned>(ptr: i32, len: i32) -> Option<T> {
    let bytes = unsafe { payload_bytes(ptr, len) }?;
    postcard::from_bytes(bytes).ok()
}

pub fn response_ptr_len<T: Serialize>(response: &T) -> i64 {
    let Ok(bytes) = postcard::to_allocvec(response) else {
        return 0;
    };
    response_bytes_ptr_len(&bytes)
}

pub fn response_bytes_ptr_len(response: &[u8]) -> i64 {
    let len = match i32::try_from(response.len()) {
        Ok(len) if len > 0 => len,
        _ => return 0,
    };
    let ptr = alloc(len);
    if ptr <= 0 {
        return 0;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(response.as_ptr(), ptr as *mut u8, len as usize);
    }
    ((ptr as i64) << 32) | (len as u32 as i64)
}
