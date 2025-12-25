use serde::{Deserialize, Serialize};

use crate::app::qexed_chunk::engine::Engine;
#[derive(Debug, Serialize, Deserialize,Clone)]
pub struct World {
    pub version: i32,
    // 世界名
    pub name:String,
    // 世界命名空间(例如主世界)
    pub namespace:String,
    // 随机种子
    pub seed: i64,
    // ===== 游戏规则 =====
    // 袭击(原版貌似主世界会用，其他维度没啥用)
    pub raid:bool,
    // 兴趣点(村民交易、蜜蜂等使用)
    // 警告:不建议设置为false,该机制影响大部分原版机制
    pub poi:bool,
    // 实体
    pub entitie:bool,
}
impl Default for World {
    fn default() -> Self {
        Self {
            version: 0,
            name:"未命名的世界".to_string(),
            namespace:"minecraft:overworld".to_string(),
            seed:rand::random(),
            raid:false,
            poi:false,
            entitie:false,
        }
    }
}
impl World {
    pub fn overworld()->Self{
        let mut world : Self = Default::default();
        world.name = "主世界".to_string();
        world.namespace = "minecraft:overworld".to_string();
        world.raid=true;
        world.poi=true;
        world.entitie=true;
        world
    }
    pub fn the_nether()->Self{
        let mut world : Self = Default::default();
        world.name = "下界".to_string();
        world.namespace = "minecraft:the_nether".to_string();
        world.raid=false;
        world.poi=true;
        world.entitie=true;
        world
    }
    pub fn the_end()->Self{
        let mut world : Self = Default::default();
        world.name = "末地".to_string();
        world.namespace = "minecraft:the_end".to_string();
        world.raid=false;
        world.poi=true;
        world.entitie=true;
        world
    }
}