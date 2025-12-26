use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Debug, Serialize, Deserialize,Clone)]
pub struct MiniLobbyConfig {
    pub version: i32,
    pub world_dir:String,
    // 世界名
    pub name:String,
    // 主世界（进服后的世界，不是说主世界维度)
    pub main_world:Uuid,
    // 进服位置
    pub join_pos:[i64;3],
    // 地图范围
    pub map_range:[[i64;2];2],
}
impl Default for MiniLobbyConfig {
    fn default() -> Self {
        Self {
            version: 0,
            world_dir:"./world/".to_string(),
            name:"大厅".to_string(),
            main_world:uuid::Uuid::new_v4(),
            join_pos:qexed_random::pos::pos_join_spawn_area(),
            map_range:[[-100,-100],[100,100]],
        }
    }
}