//! Qexed 实体注册表模块
//! 提供类型安全的实体ID和实体信息管理


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

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_entity_id_conversion() {
        // 测试ID转换
        let pig = EntityId::try_from(0u32);
        if let Ok(entity) = pig {
            assert_eq!(entity.to_i32(), 0);
            assert_eq!(entity.to_u32(), 0);
            
            // 测试从ID解析
            let from_id = EntityId::try_from(0u32);
            assert!(from_id.is_ok());
            assert_eq!(from_id.unwrap(), entity);
        }
    }
    
    #[test]
    fn test_entity_registry() {
        let registry = EntityRegistry::new();
        
        // 测试默认实体
        assert!(registry.is_valid_entity_id(DEFAULT_ENTITY_ID));
        
        // 测试获取信息
        if let Some(info) = registry.get_entity_info(DEFAULT_ENTITY_ID) {
            assert_eq!(info.id, DEFAULT_ENTITY_ID);
            assert!(!info.name.is_empty());
        }
    }
    
    #[test]
    fn test_entity_manager() {
        let mut manager = EntityManager::new();
        
        // 创建实体
        let position = EntityPosition::new(0.0, 64.0, 0.0);
        let entity_id = manager.create_entity(DEFAULT_ENTITY, position);
        
        // 验证实体创建
        assert!(entity_id > 0);
        assert!(manager.get_entity(entity_id).is_some());
        
        // 获取实体数据
        if let Some(entity) = manager.get_entity(entity_id) {
            assert_eq!(entity.entity_type, DEFAULT_ENTITY);
            assert_eq!(entity.position.x, 0.0);
            assert_eq!(entity.position.y, 64.0);
            assert_eq!(entity.position.z, 0.0);
        }
        
        // 移除实体
        let removed = manager.remove_entity(entity_id);
        assert!(removed.is_some());
        assert!(manager.get_entity(entity_id).is_none());
    }
    
    #[test]
    fn test_entity_functions() {
        // 测试辅助函数
        assert!(!all_entity_ids().is_empty());
        
        // 测试实体枚举
        let entities = all_entities();
        assert!(!entities.is_empty());
        
        // 确保包含默认实体
        assert!(entities.contains(&DEFAULT_ENTITY));
    }
}