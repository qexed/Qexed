// crates/bin/qexed_block/src/lib.rs
// 在 lib.rs 中添加以下内容
use thiserror::Error;

// 包含生成的代码
include!(concat!(env!("OUT_DIR"), "/block_registry_generated.rs"));

// 方块注册表管理器
#[derive(Debug, Clone)]
pub struct BlockRegistry {
    // 可以添加状态管理等
}

impl BlockRegistry {
    pub fn new() -> Self {
        Self {}
    }
    
    /// 验证方块ID是否有效
    pub fn is_valid_block_id(&self, id: u32) -> bool {
        get_block_name_by_id(id).is_some()
    }
    
    /// 获取方块信息
    pub fn get_block_info(&self, id: u32) -> Option<BlockInfo> {
        BLOCK_INFO_BY_ID.get(&id).cloned()
    }
    
    /// 解析方块名称
    pub fn parse_block_name(&self, name: &str) -> Option<u32> {
        get_block_id_by_name(name)
    }
    
    // 新增：获取方块状态信息
    pub fn get_block_state(&self, state_id: u32) -> Option<BlockStateInfo> {
        BLOCK_STATE_BY_ID.get(&state_id).cloned()
    }
    
    // 新增：获取方块的所有状态ID
    pub fn get_block_states(&self, block_name: &str) -> Option<Vec<u32>> {
        BLOCK_STATES_BY_NAME.get(block_name).cloned()
    }
    
    // 新增：获取方块的默认状态ID
    pub fn get_default_state(&self, block_name: &str) -> Option<u32> {
        get_default_state_id(block_name)
    }
    
    // 新增：通过属性查找状态ID
    pub fn find_state_by_properties(&self, block_name: &str, properties: &HashMap<&str, &str>) -> Option<u32> {
        find_state_id_by_properties(block_name, properties)
    }
}

// 方块位置结构
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

// 方块状态（更新）
#[derive(Debug, Clone)]
pub struct BlockState {
    pub id: u32,
    pub position: BlockPos,
    pub properties: HashMap<String, String>,
}

impl BlockState {
    pub fn new(state_id: u32, position: BlockPos) -> Self {
        Self {
            id: state_id,
            position,
            properties: HashMap::new(),
        }
    }
    
    /// 从状态ID创建方块状态，并自动填充属性
    pub fn from_state_id(state_id: u32, position: BlockPos) -> Option<Self> {
        BLOCK_STATE_BY_ID.get(&state_id).map(|info| {
            let properties = info.properties.iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            Self {
                id: state_id,
                position,
                properties,
            }
        })
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
}

/// 解析方块状态字符串的结果类型
pub type BlockStateResult<T> = Result<T, BlockStateParseError>;

/// 方块状态解析器
#[derive(Debug, Clone)]
pub struct BlockStateParser;

impl BlockStateParser {
    /// 解析方块状态字符串
    /// 示例: "minecraft:barrier[waterlogged=true]"
    pub fn parse(block_state_str: &str) -> BlockStateResult<ParsedBlockState> {
        // 检查是否包含属性部分
        if let Some(bracket_start) = block_state_str.find('[') {
            let bracket_end = block_state_str.find(']')
                .ok_or_else(|| BlockStateParseError::InvalidFormat("缺少结束括号".to_string()))?;
            
            // 提取方块ID部分 (括号之前)
            let block_id_str = &block_state_str[..bracket_start].trim();
            // 提取属性部分 (括号内部)
            let properties_str = &block_state_str[bracket_start + 1..bracket_end].trim();
            
            // 解析方块ID
            let block_id = Self::parse_block_id(block_id_str)?;
            // 解析属性
            let properties = Self::parse_properties(properties_str)?;
            
            Ok(ParsedBlockState {
                block_id: block_id.to_string(),
                properties,
            })
        } else {
            // 没有属性，只有方块ID
            let block_id = Self::parse_block_id(block_state_str)?;
            Ok(ParsedBlockState {
                block_id: block_id.to_string(),
                properties: HashMap::new(),
            })
        }
    }
    
    /// 解析方块ID部分
    fn parse_block_id(block_id_str: &str) -> BlockStateResult<&str> {
        let block_id = block_id_str.trim();
        if block_id.is_empty() {
            return Err(BlockStateParseError::InvalidFormat("方块ID不能为空".to_string()));
        }
        
        // 添加默认命名空间（如果未指定）
        let full_id = if !block_id.contains(':') {
            format!("minecraft:{}", block_id)
        } else {
            block_id.to_string()
        };
        
        // 这里可以添加方块ID的验证逻辑
        Ok(Box::leak(full_id.into_boxed_str()))
    }
    
    /// 解析属性部分
    fn parse_properties(properties_str: &str) -> BlockStateResult<HashMap<String, String>> {
        let mut properties = HashMap::new();
        
        if properties_str.is_empty() {
            return Ok(properties);
        }
        
        for property_pair in properties_str.split(',') {
            let pair = property_pair.trim();
            if pair.is_empty() {
                continue;
            }
            
            // 分割属性名和值
            let parts: Vec<&str> = pair.splitn(2, '=').collect();
            if parts.len() != 2 {
                return Err(BlockStateParseError::InvalidFormat(
                    format!("无效的属性格式: {}", pair)
                ));
            }
            
            let name = parts[0].trim().to_string();
            let value = parts[1].trim().to_string();
            
            if name.is_empty() {
                return Err(BlockStateParseError::InvalidFormat("属性名不能为空".to_string()));
            }
            
            properties.insert(name, value);
        }
        
        Ok(properties)
    }
    
    /// 将方块状态转换为状态ID
    pub fn parse_to_state_id(block_state_str: &str) -> BlockStateResult<u32> {
        let parsed = Self::parse(block_state_str)?;
        Self::find_matching_state_id(&parsed.block_id, &parsed.properties)
    }
    
    /// 查找匹配的状态ID
    fn find_matching_state_id(block_id: &str, properties: &HashMap<String, String>) -> BlockStateResult<u32> {
        // 获取该方块的所有状态
        let state_ids = BLOCK_STATES_BY_NAME.get(block_id)
            .ok_or_else(|| BlockStateParseError::UnknownBlock(block_id.to_string()))?;
        
        // 查找匹配的状态
        for &state_id in state_ids {
            if let Some(state_info) = BLOCK_STATE_BY_ID.get(&state_id) {
                // 检查属性是否匹配
                if Self::properties_match(properties, &state_info.properties) {
                    return Ok(state_id);
                }
            }
        }
        
        // 如果没有完全匹配，尝试查找默认状态
        if let Some(default_id) = get_default_state_id(block_id) {
            return Ok(default_id);
        }
        
        Err(BlockStateParseError::InvalidProperty(
            "无法找到匹配的方块状态".to_string(),
            format!("{} with properties {:?}", block_id, properties)
        ))
    }
    
    /// 检查属性是否匹配
    fn properties_match(query_props: &HashMap<String, String>, state_props: &HashMap<&str, &str>) -> bool {
        for (key, value) in query_props {
            match state_props.get(key.as_str()) {
                Some(state_value) if state_value == &value.as_str() => continue,
                _ => return false,
            }
        }
        true
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
}

// 为 BlockRegistry 添加新方法
impl BlockRegistry {
    /// 解析方块状态字符串
    pub fn parse_block_state(&self, block_state_str: &str) -> BlockStateResult<ParsedBlockState> {
        BlockStateParser::parse(block_state_str)
    }
    
    /// 将方块状态字符串转换为状态ID
    pub fn block_state_to_id(&self, block_state_str: &str) -> BlockStateResult<u32> {
        BlockStateParser::parse_to_state_id(block_state_str)
    }
    
    /// 从状态ID获取方块状态字符串表示
    pub fn state_id_to_string(&self, state_id: u32) -> Option<String> {
        BLOCK_STATE_BY_ID.get(&state_id).map(|info| {
            if info.properties.is_empty() {
                info.block_name.to_string()
            } else {
                let props: Vec<String> = info.properties.iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect();
                format!("{}[{}]", info.block_name, props.join(","))
            }
        })
    }
}