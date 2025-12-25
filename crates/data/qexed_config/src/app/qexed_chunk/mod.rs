pub mod world;
pub mod engine;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::{app::qexed_chunk::engine::Engine, tool::AppConfigTrait};
#[derive(Debug, Serialize, Deserialize,Clone)]
pub struct ChunkConfig {
    pub version: i32,
    // 世界管理引擎(注意子服需要保持一致)
    pub engine:Engine,
    // 原版引擎
    pub engine_setting:EngineSetting
}
#[derive(Debug, Serialize, Deserialize,Clone,Default)]
pub struct EngineSetting{
    pub original:engine::original::OriginalConfig,
    pub minilobby:engine::mini_lobby::MiniLobbyConfig,
    
}
impl Default for ChunkConfig {
    fn default() -> Self {
        Self {
            version: 0,
            engine_setting:Default::default(),
            engine:Engine::Original,
        }
    }
}
impl AppConfigTrait for ChunkConfig {
    const PATH: &'static str = "./config/qexed_chunk/";
    const NAME: &'static str = "config";
}
