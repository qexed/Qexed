pub mod world;
pub mod engine;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::{app::qexed_chunk::engine::Engine, tool::AppConfigTrait};
#[derive(Debug, Serialize, Deserialize,Clone)]
pub struct ChunkConfig {
    pub version: i32,
    pub world_dir:String,
    pub world:HashMap<Uuid,world::World>,
    // 世界管理引擎(注意子服需要保持一致)
    pub engine:Engine,
    // 主世界（进服后的世界，不是说主世界维度)
    pub main_world:Uuid,
    // 进服位置
    pub join_pos:[i64;3],
    // 玩家视野
    pub view_distance:u32,

}
impl Default for ChunkConfig {
    fn default() -> Self {
        let mut worlds = HashMap::new();
        let main_world = uuid::Uuid::new_v4();
        worlds.insert(main_world, world::World::overworld());
        worlds.insert(uuid::Uuid::new_v4(), world::World::the_nether());
        worlds.insert(uuid::Uuid::new_v4(), world::World::the_end());

        Self {
            version: 0,
            world_dir:"./world/".to_string(),
            world:worlds,
            main_world:main_world,
            join_pos:qexed_random::pos::pos_join_spawn_area(),
            view_distance:12,
            engine:Engine::Original,
        }
    }
}
impl AppConfigTrait for ChunkConfig {
    const PATH: &'static str = "./config/qexed_chunk/";
    const NAME: &'static str = "config";
}
