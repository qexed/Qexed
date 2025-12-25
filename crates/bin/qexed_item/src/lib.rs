// Qexed 物品注册表模块
// 提供类型安全的物品ID和物品信息管理


// 包含生成的代码
include!(concat!(env!("OUT_DIR"), "/item_registry_generated.rs"));
// 重新导出生成的模块
#[cfg(feature = "generate")]
pub use crate::generated::*;


/// 错误类型
pub mod error {
    use thiserror::Error;
    
    /// 物品相关错误
    #[derive(Debug, Error)]
    pub enum ItemError {
        /// 无效的物品ID
        #[error("Invalid item ID: {0}")]
        InvalidId(u32),
        
        /// 无效的物品名称
        #[error("Invalid item name: {0}")]
        InvalidName(String),
        
        /// 序列化/反序列化错误
        #[error("Serialization error: {0}")]
        Serialization(#[from] serde_json::Error),
    }
}

/// 物品注册表管理器
#[derive(Debug, Clone)]
pub struct ItemRegistry {
    // 可以添加缓存或其他状态
}

impl ItemRegistry {
    /// 创建新的物品注册表
    pub fn new() -> Self {
        Self {}
    }
    
    /// 验证物品ID是否有效
    pub fn is_valid_item_id(&self, id: u32) -> bool {
        get_item_by_id(id).is_some()
    }
    
    /// 获取物品信息
    pub fn get_item_info(&self, id: u32) -> Option<ItemInfo> {
        ITEM_INFO_BY_ID.get(&id).cloned()
    }
    
    /// 解析物品名称
    pub fn parse_item_name(&self, name: &str) -> Option<u32> {
        get_item_id_by_name(name)
    }
    
    /// 获取物品的显示名称
    pub fn get_display_name(&self, id: u32) -> Option<String> {
        self.get_item_info(id)
            .map(|info| info.display_name.to_string())
    }
    
    /// 检查物品是否可以堆叠
    pub fn is_stackable(&self, id: u32) -> bool {
        // 默认大部分物品可以堆叠，除了工具、武器等
        !self.is_tool(id) && !self.is_weapon(id)
    }
    
    /// 检查物品是否为工具
    pub fn is_tool(&self, id: u32) -> bool {
        get_item_by_id(id)
            .map(is_tool)
            .unwrap_or(false)
    }
    
    /// 检查物品是否为武器
    pub fn is_weapon(&self, id: u32) -> bool {
        get_item_by_id(id)
            .map(is_weapon)
            .unwrap_or(false)
    }
    
    /// 检查物品是否为食物
    pub fn is_food(&self, id: u32) -> bool {
        get_item_by_id(id)
            .map(is_food)
            .unwrap_or(false)
    }
    
    /// 获取物品的最大堆叠数量
    pub fn get_max_stack_size(&self, id: u32) -> u8 {
        if self.is_stackable(id) {
            64
        } else {
            1
        }
    }
}

/// 物品槽位
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ItemSlot {
    /// 槽位索引
    pub index: u8,
    /// 物品数量
    pub count: u8,
    /// 物品ID
    pub item: ItemId,
    /// 物品耐久度（如果有）
    pub durability: Option<u16>,
}

impl ItemSlot {
    /// 创建新的物品槽位
    pub fn new(item: ItemId, count: u8) -> Self {
        Self {
            index: 0,
            count,
            item,
            durability: None,
        }
    }
    
    /// 检查槽位是否为空
    pub fn is_empty(&self) -> bool {
        self.count == 0 || self.item == DEFAULT_ITEM
    }
    
    /// 设置耐久度
    pub fn with_durability(mut self, durability: u16) -> Self {
        self.durability = Some(durability);
        self
    }
}

/// 物品库存
#[derive(Debug, Clone)]
pub struct ItemInventory {
    /// 槽位列表
    slots: Vec<ItemSlot>,
    /// 最大大小
    max_size: usize,
}

impl ItemInventory {
    /// 创建新的库存
    pub fn new(max_size: usize) -> Self {
        Self {
            slots: vec![ItemSlot::new(DEFAULT_ITEM, 0); max_size],
            max_size,
        }
    }
    
    /// 获取槽位中的物品
    pub fn get_slot(&self, index: usize) -> Option<&ItemSlot> {
        self.slots.get(index)
    }
    
    /// 设置槽位中的物品
    pub fn set_slot(&mut self, index: usize, slot: ItemSlot) -> Result<(), error::ItemError> {
        if index >= self.max_size {
            return Ok(()); // 或者返回错误
        }
        
        self.slots[index] = slot;
        Ok(())
    }
    
    /// 添加物品到库存
    pub fn add_item(&mut self, item: ItemId, count: u8) -> Result<u8, error::ItemError> {
        let mut remaining = count;
        
        // 先尝试堆叠到已有槽位
        for slot in &mut self.slots {
            if slot.item == item && !slot.is_empty() {
                let max_stack = 64; // 获取实际的最大堆叠
                let space = max_stack - slot.count;
                let to_add = remaining.min(space);
                
                slot.count += to_add;
                remaining -= to_add;
                
                if remaining == 0 {
                    return Ok(0);
                }
            }
        }
        
        // 寻找空槽位
        for slot in &mut self.slots {
            if slot.is_empty() {
                slot.item = item;
                slot.count = remaining;
                return Ok(0);
            }
        }
        
        Ok(remaining) // 返回剩余无法添加的数量
    }
}

/// 物品工具函数
pub mod utils {
    use super::*;
    
    /// 从网络数据包解析物品
    pub fn from_network_data(data: &[u8]) -> Result<ItemSlot, error::ItemError> {
        // 这里实现从网络数据包解析物品的逻辑
        // 示例实现
        if data.len() < 2 {
            return Ok(ItemSlot::new(DEFAULT_ITEM, 0));
        }
        
        let item_id = u16::from_le_bytes([data[0], data[1]]) as u32;
        let count = if data.len() > 2 { data[2] } else { 1 };
        
        let item = ItemId::try_from(item_id)?;
        Ok(ItemSlot::new(item, count))
    }
    
    /// 将物品转换为网络数据
    pub fn to_network_data(slot: &ItemSlot) -> Vec<u8> {
        let mut data = Vec::new();
        
        // 写入物品ID
        let id = slot.item.to_u32() as u16;
        data.extend_from_slice(&id.to_le_bytes());
        
        // 写入数量
        data.push(slot.count);
        
        // 写入耐久度（如果有）
        if let Some(durability) = slot.durability {
            data.extend_from_slice(&durability.to_le_bytes());
        }
        
        data
    }
}
