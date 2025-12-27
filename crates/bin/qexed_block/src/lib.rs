// crates/bin/qexed_block/src/lib.rs
use thiserror::Error;
use std::collections::HashMap;
use once_cell::sync::Lazy;

// 包含生成的代码
include!(concat!(env!("OUT_DIR"), "/block_registry_generated.rs"));

/// 方块注册表管理器
#[derive(Debug, Clone)]
pub struct BlockRegistry {
    // 可以添加缓存或其他状态管理
}

impl Default for BlockRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl BlockRegistry {
    pub fn new() -> Self {
        Self {}
    }
    
    /// 验证方块ID是否有效
    pub fn is_valid_block_id(&self, id: u32) -> bool {
        get_block_name_by_id(id).is_some()
    }
    
    /// 获取方块信息（优化：直接使用匹配表达式）
    pub fn get_block_info(&self, id: u32) -> Option<&'static BlockInfo> {
        get_block_info_by_id(id)
    }
    
    /// 解析方块名称（优化：直接使用匹配表达式）
    pub fn parse_block_name(&self, name: &str) -> Option<u32> {
        get_block_id_by_name(name)
    }
    
    /// 获取方块状态信息（优化：直接使用数组索引）
    pub fn get_block_state(&self, state_id: u32) -> Option<&'static BlockStateInfo> {
        get_block_state_by_id(state_id)
    }
    
    /// 获取方块的所有状态ID（优化：使用切片引用）
    pub fn get_block_states(&self, block_name: &str) -> Option<&'static [u32]> {
        get_block_states_by_name(block_name)
    }
    
    /// 获取方块的默认状态ID
    pub fn get_default_state(&self, block_name: &str) -> Option<u32> {
        get_default_state_id(block_name)
    }
    
    /// 通过属性查找状态ID（优化：使用切片而非HashMap）
    pub fn find_state_by_properties(&self, block_name: &str, properties: &[(&str, &str)]) -> Option<u32> {
        find_state_id_by_properties(block_name, properties)
    }
    
    /// 解析方块状态字符串（新增：高性能版本）
    pub fn parse_block_state(&self, block_state_str: &str) -> BlockStateResult<ParsedBlockState> {
        BlockStateParser::parse(block_state_str)
    }
    
    /// 将方块状态字符串转换为状态ID
    pub fn block_state_to_id(&self, block_state_str: &str) -> BlockStateResult<u32> {
        BlockStateParser::parse_to_state_id(block_state_str)
    }
    
    /// 从状态ID获取方块状态字符串表示
    pub fn state_id_to_string(&self, state_id: u32) -> Option<String> {
        get_block_state_by_id(state_id).map(|info| {
            if info.properties.is_empty() {
                info.block_name.to_string()
            } else {
                // 优化：预分配容量
                let mut result = String::with_capacity(info.block_name.len() + info.properties.len() * 10);
                result.push_str(info.block_name);
                result.push('[');
                
                let mut first = true;
                for (k, v) in info.properties {
                    if !first {
                        result.push(',');
                    }
                    result.push_str(k);
                    result.push('=');
                    result.push_str(v);
                    first = false;
                }
                
                result.push(']');
                result
            }
        })
    }
    
    /// 批量获取方块信息（新增：提高批量操作性能）
    pub fn get_multiple_block_states(&self, state_ids: &[u32]) -> Vec<Option<&'static BlockStateInfo>> {
        state_ids.iter()
            .map(|&id| get_block_state_by_id(id))
            .collect()
    }
    
    /// 验证方块状态属性（新增：提前验证）
    pub fn validate_block_properties(&self, block_name: &str, properties: &[(&str, &str)]) -> BlockStateResult<()> {
        let states = self.get_block_states(block_name)
            .ok_or_else(|| BlockStateParseError::UnknownBlock(block_name.to_string()))?;
        
        for (key, value) in properties {
            // 这里可以添加更复杂的属性验证逻辑
            if key.is_empty() || value.is_empty() {
                return Err(BlockStateParseError::InvalidProperty(
                    key.to_string(),
                    value.to_string()
                ));
            }
        }
        
        Ok(())
    }
}

/// 方块位置结构
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BlockPos {
    /// 创建新位置
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }
    
    /// 获取相邻位置
    pub fn adjacent(&self, dx: i32, dy: i32, dz: i32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
            z: self.z + dz,
        }
    }
}

/// 优化的方块状态结构
#[derive(Debug, Clone)]
pub struct BlockState {
    pub id: u32,
    pub position: BlockPos,
    pub properties: Vec<(&'static str, &'static str)>, // 使用切片而非HashMap
}

impl BlockState {
    pub fn new(state_id: u32, position: BlockPos) -> Self {
        Self {
            id: state_id,
            position,
            properties: Vec::new(),
        }
    }
    
    /// 从状态ID创建方块状态，并自动填充属性
    pub fn from_state_id(state_id: u32, position: BlockPos) -> Option<Self> {
        get_block_state_by_id(state_id).map(|info| {
            let properties = info.properties.to_vec(); // 转换为Vec
            Self {
                id: state_id,
                position,
                properties,
            }
        })
    }
    
    /// 获取属性值（优化：线性搜索，对于少量属性更快）
    pub fn get_property(&self, key: &str) -> Option<&'static str> {
        self.properties.iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| *v)
    }
    
    /// 设置属性值
    pub fn set_property(&mut self, key: &'static str, value: &'static str) {
        if let Some(pos) = self.properties.iter().position(|(k, _)| *k == key) {
            self.properties[pos] = (key, value);
        } else {
            self.properties.push((key, value));
        }
    }
}

/// 方块状态解析错误
#[derive(Debug, Error, Clone)]
pub enum BlockStateParseError {
    #[error("无效的方块状态格式: {0}")]
    InvalidFormat(String),
    #[error("未知的方块: {0}")]
    UnknownBlock(String),
    #[error("无效的属性 '{0}': {1}")]
    InvalidProperty(String, String),
    #[error("属性值超出范围: {0}={1}")]
    PropertyValueOutOfRange(String, String),
    #[error("状态ID不存在: {0}")]
    InvalidStateId(u32),
}

/// 解析方块状态字符串的结果类型
pub type BlockStateResult<T> = Result<T, BlockStateParseError>;

/// 高性能方块状态解析器
#[derive(Debug, Clone)]
pub struct BlockStateParser;

impl BlockStateParser {
    /// 解析方块状态字符串（优化版本）
    /// 示例: "minecraft:barrier[waterlogged=true]"
    pub fn parse(block_state_str: &str) -> BlockStateResult<ParsedBlockState> {
        let input = block_state_str.trim();
        if input.is_empty() {
            return Err(BlockStateParseError::InvalidFormat("输入不能为空".to_string()));
        }
        
        // 查找属性部分
        if let Some(bracket_start) = input.find('[') {
            let bracket_end = input.find(']')
                .ok_or_else(|| BlockStateParseError::InvalidFormat("缺少结束括号".to_string()))?;
            
            if bracket_end <= bracket_start {
                return Err(BlockStateParseError::InvalidFormat("括号不匹配".to_string()));
            }
            
            let block_id_str = &input[..bracket_start].trim();
            let properties_str = &input[bracket_start + 1..bracket_end].trim();
            
            let block_id = Self::parse_block_id(block_id_str)?;
            let properties = Self::parse_properties_fast(properties_str)?;
            
            Ok(ParsedBlockState {
                block_id: block_id.to_string(),
                properties,
            })
        } else {
            // 没有属性
            let block_id = Self::parse_block_id(input)?;
            Ok(ParsedBlockState {
                block_id: block_id.to_string(),
                properties: HashMap::new(),
            })
        }
    }
    
    /// 快速解析方块ID
    fn parse_block_id(block_id_str: &str) -> BlockStateResult<&str> {
        let block_id = block_id_str.trim();
        if block_id.is_empty() {
            return Err(BlockStateParseError::InvalidFormat("方块ID不能为空".to_string()));
        }
        
        // 添加默认命名空间（如果未指定）
        if !block_id.contains(':') {
            Ok(Box::leak(format!("minecraft:{}", block_id).into_boxed_str()))
        } else {
            Ok(block_id)
        }
    }
    
    /// 快速解析属性（优化版本）
    fn parse_properties_fast(properties_str: &str) -> BlockStateResult<HashMap<String, String>> {
        let mut properties = HashMap::new();
        
        if properties_str.is_empty() {
            return Ok(properties);
        }
        
        // 预分割，避免多次分配
        let pairs: Vec<&str> = properties_str.split(',').collect();
        
        for pair in pairs {
            let pair = pair.trim();
            if pair.is_empty() {
                continue;
            }
            
            // 使用split_once更高效
            if let Some((name, value)) = pair.split_once('=') {
                let name = name.trim().to_string();
                let value = value.trim().to_string();
                
                if name.is_empty() {
                    return Err(BlockStateParseError::InvalidFormat("属性名不能为空".to_string()));
                }
                
                properties.insert(name, value);
            } else {
                return Err(BlockStateParseError::InvalidFormat(
                    format!("无效的属性格式: {}", pair)
                ));
            }
        }
        
        Ok(properties)
    }
    
    /// 将方块状态转换为状态ID
    pub fn parse_to_state_id(block_state_str: &str) -> BlockStateResult<u32> {
        let parsed = Self::parse(block_state_str)?;
        Self::find_matching_state_id(&parsed.block_id, &parsed.properties)
    }
    
    /// 查找匹配的状态ID（优化版本）
    fn find_matching_state_id(block_id: &str, properties: &HashMap<String, String>) -> BlockStateResult<u32> {
        let state_ids = get_block_states_by_name(block_id)
            .ok_or_else(|| BlockStateParseError::UnknownBlock(block_id.to_string()))?;
        
        // 转换为静态切片进行快速比较
        let prop_slice: Vec<(&str, &str)> = properties.iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        
        // 查找精确匹配
        for &state_id in state_ids {
            if let Some(state_info) = get_block_state_by_id(state_id) {
                if Self::properties_match_fast(&prop_slice, &state_info.properties) {
                    return Ok(state_id);
                }
            }
        }
        
        // 返回默认状态
        get_default_state_id(block_id)
            .ok_or_else(|| BlockStateParseError::InvalidProperty(
                "无法找到匹配的方块状态".to_string(),
                format!("{} with properties {:?}", block_id, properties)
            ))
    }
    
    /// 快速属性匹配
    fn properties_match_fast(query_props: &[(&str, &str)], state_props: &[(&'static str, &'static str)]) -> bool {
        if query_props.len() != state_props.len() {
            return false;
        }
        
        // 对于少量属性，线性搜索更快
        for (qk, qv) in query_props {
            if !state_props.iter().any(|(sk, sv)| sk == qk && sv == qv) {
                return false;
            }
        }
        
        true
    }
    
    /// 批量解析方块状态（新增：提高批量操作性能）
    pub fn parse_batch(block_state_strs: &[&str]) -> Vec<BlockStateResult<ParsedBlockState>> {
        block_state_strs.iter()
            .map(|s| Self::parse(s))
            .collect()
    }
}

/// 解析后的方块状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedBlockState {
    pub block_id: String,
    pub properties: HashMap<String, String>,
}

impl ParsedBlockState {
    /// 转换为状态ID
    pub fn to_state_id(&self) -> BlockStateResult<u32> {
        BlockStateParser::find_matching_state_id(&self.block_id, &self.properties)
    }
    
    /// 获取方块ID
    pub fn block_id(&self) -> &str {
        &self.block_id
    }
    
    /// 获取属性
    pub fn properties(&self) -> &HashMap<String, String> {
        &self.properties
    }
    
    /// 转换为优化的BlockState
    pub fn to_block_state(&self, position: BlockPos) -> BlockStateResult<BlockState> {
        let state_id = self.to_state_id()?;
        BlockState::from_state_id(state_id, position)
            .ok_or_else(|| BlockStateParseError::InvalidStateId(state_id))
    }
}

// 单元测试
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_block_registry_creation() {
        let registry = BlockRegistry::new();
        assert!(registry.is_valid_block_id(0)); // 假设0是有效的方块ID
    }
    
    #[test]
    fn test_block_state_parsing() {
        let result = BlockStateParser::parse("minecraft:stone");
        assert!(result.is_ok());
    }
}