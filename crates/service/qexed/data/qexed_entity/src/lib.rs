pub mod engine;
pub mod manage;
pub mod message;
pub mod spilt_id_task;
mod run;
pub use run::run as run;
// 包含生成的代码
include!(concat!(env!("OUT_DIR"), "/entity_registry_generated.rs"));

/// 错误类型
pub mod error {
    use thiserror::Error;
    
    /// 实体相关错误
    #[derive(Debug, Error)]
    pub enum EntityError {
        /// 无效的实体ID
        #[error("Invalid entity ID: {0}")]
        InvalidId(u32),
        
        /// 无效的实体名称
        #[error("Invalid entity name: {0}")]
        InvalidName(String),
        
        /// 序列化/反序列化错误
        #[error("Serialization error: {0}")]
        Serialization(#[from] serde_json::Error),
    }
}

/// 实体注册表管理器
#[derive(Debug, Clone, Default)]
pub struct EntityRegistry;

impl EntityRegistry {
    /// 创建新的实体注册表
    pub fn new() -> Self {
        Self
    }
    
    /// 验证实体ID是否有效
    pub fn is_valid_entity_id(&self, id: u32) -> bool {
        get_entity_by_id(id).is_some()
    }
    
    /// 获取实体信息
    pub fn get_entity_info(&self, id: u32) -> Option<EntityInfo> {
        ENTITY_INFO_BY_ID.get(&id).cloned()
    }
    
    /// 解析实体名称
    pub fn parse_entity_name(&self, name: &str) -> Option<u32> {
        get_entity_id_by_name(name)
    }
    
    /// 获取实体的显示名称
    pub fn get_display_name(&self, id: u32) -> Option<String> {
        self.get_entity_info(id)
            .map(|info| info.display_name.to_string())
    }
    
    /// 检查实体是否为生物
    pub fn is_living_entity(&self, id: u32) -> bool {
        get_entity_by_id(id)
            .map(is_living_entity)
            .unwrap_or(false)
    }
    
    /// 检查实体是否为物品实体
    pub fn is_item_entity(&self, id: u32) -> bool {
        get_entity_by_id(id)
            .map(is_item_entity)
            .unwrap_or(false)
    }
    
    /// 检查实体是否为弹射物
    pub fn is_projectile(&self, id: u32) -> bool {
        get_entity_by_id(id)
            .map(is_projectile)
            .unwrap_or(false)
    }
}

/// 实体位置
#[derive(Debug, Clone, Copy)]
pub struct EntityPosition {
    /// X 坐标
    pub x: f64,
    /// Y 坐标
    pub y: f64,
    /// Z 坐标
    pub z: f64,
    /// 偏航角（水平旋转，以度为单位）
    pub yaw: f32,
    /// 俯仰角（垂直旋转，以度为单位）
    pub pitch: f32,
}

impl EntityPosition {
    /// 创建新的实体位置
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self {
            x, y, z,
            yaw: 0.0,
            pitch: 0.0,
        }
    }
    
    /// 设置朝向
    pub fn with_rotation(mut self, yaw: f32, pitch: f32) -> Self {
        self.yaw = yaw;
        self.pitch = pitch;
        self
    }
}

/// 实体数据
#[derive(Debug, Clone)]
pub struct EntityData {
    /// 实体ID（由服务器分配的唯一标识符）
    pub entity_id: u32,
    /// 实体类型
    pub entity_type: EntityId,
    /// 位置信息
    pub position: EntityPosition,
    /// 速度向量 (x, y, z)
    pub velocity: (f32, f32, f32),
    /// 当前生命值
    pub health: f32,
    /// 是否在地面上
    pub on_ground: bool,
}

impl EntityData {
    /// 创建新的实体数据
    pub fn new(entity_type: EntityId, position: EntityPosition) -> Self {
        Self {
            entity_id: 0, // 将由服务器分配
            entity_type,
            position,
            velocity: (0.0, 0.0, 0.0),
            health: get_entity_max_health(entity_type),
            on_ground: false,
        }
    }
    
    /// 设置实体ID
    pub fn with_entity_id(mut self, entity_id: u32) -> Self {
        self.entity_id = entity_id;
        self
    }
    
    /// 设置生命值
    pub fn with_health(mut self, health: f32) -> Self {
        self.health = health;
        self
    }
}

/// 实体管理器
#[derive(Debug, Clone, Default)]
pub struct EntityManager {
    /// 实体存储
    entities: std::collections::HashMap<u32, EntityData>,
    /// 下一个可用的实体ID
    next_entity_id: u32,
}

impl EntityManager {
    /// 创建新的实体管理器
    pub fn new() -> Self {
        Self {
            entities: std::collections::HashMap::new(),
            next_entity_id: 1, // 0通常保留
        }
    }
    
    /// 创建新实体
    pub fn create_entity(&mut self, entity_type: EntityId, position: EntityPosition) -> u32 {
        let entity_id = self.next_entity_id;
        self.next_entity_id += 1;
        
        let entity_data = EntityData::new(entity_type, position)
            .with_entity_id(entity_id);
        
        self.entities.insert(entity_id, entity_data);
        entity_id
    }
    
    /// 获取实体数据
    pub fn get_entity(&self, entity_id: u32) -> Option<&EntityData> {
        self.entities.get(&entity_id)
    }
    
    /// 获取实体数据（可变）
    pub fn get_entity_mut(&mut self, entity_id: u32) -> Option<&mut EntityData> {
        self.entities.get_mut(&entity_id)
    }
    
    /// 移除实体
    pub fn remove_entity(&mut self, entity_id: u32) -> Option<EntityData> {
        self.entities.remove(&entity_id)
    }
    
    /// 获取所有实体ID
    pub fn all_entity_ids(&self) -> Vec<u32> {
        self.entities.keys().copied().collect()
    }
    
    /// 获取所有实体数据
    pub fn all_entities(&self) -> Vec<&EntityData> {
        self.entities.values().collect()
    }
    
    /// 获取范围内的实体
    pub fn get_entities_in_range(&self, position: EntityPosition, range: f64) -> Vec<&EntityData> {
        self.entities.values()
            .filter(|entity| {
                let dx = entity.position.x - position.x;
                let dy = entity.position.y - position.y;
                let dz = entity.position.z - position.z;
                (dx * dx + dy * dy + dz * dz) <= range * range
            })
            .collect()
    }
}

