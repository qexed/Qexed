use qexed_block::BlockStateParser;
use serde::Serialize;
use serde::Deserialize;
use thiserror::Error;
use std::collections::HashMap;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockStates{
    #[serde(rename="Name")]
    pub name:String,
    #[serde(rename="Properties")]
    pub properties:Option<HashMap<String,String>>,
}
impl BlockStates {
    pub fn to_custom_string(&self) -> String {
        let mut result = String::new();
        
        // 添加名称
        result.push_str(&self.name);
        
        // 处理属性
        if let Some(properties) = &self.properties {
            if !properties.is_empty() {
                result.push('[');
                
                // 将属性格式化为 key=value 对
                let prop_pairs: Vec<String> = properties
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect();
                
                result.push_str(&prop_pairs.join(","));
                result.push(']');
            }
        }
        
        result
    }
    /// 获取方块状态对应的状态ID
    /// 如果找到匹配的状态则返回 Some(state_id)，否则返回 None
    pub fn get_state_id(&self) -> Option<u32> {
        // 如果属性为None，则查找默认状态
        // 方案1：简单高效的切片版本
        let properties_slice = match &self.properties {
            Some(props) => {
                props.iter()
                    .map(|(k, v)| (k.as_str(), v.as_str()))
                    .collect::<Vec<_>>()
            },
            None => Vec::new(),
        };
        // 调用之前定义的查找函数
        qexed_block::find_state_id_by_properties(&self.name, &properties_slice)
    }
    
    /// 检查方块状态是否有效
    pub fn is_valid(&self) -> bool {
        self.get_state_id().is_some()
    }
    
    /// 获取方块的显示名称（移除命名空间）
    pub fn get_display_name(&self) -> &str {
        self.name.split(':').last().unwrap_or(&self.name)
    }
    
    /// 创建一个新的 BlockStates 实例
    pub fn new<S: Into<String>>(name: S) -> Self {
        Self {
            name: name.into(),
            properties: None,
        }
    }
    
    /// 使用属性创建一个新的 BlockStates 实例
    pub fn with_properties<S: Into<String>>(
        name: S, 
        properties: HashMap<String, String>
    ) -> Self {
        Self {
            name: name.into(),
            properties: Some(properties),
        }
    }
    
    /// 添加一个属性（链式调用）
    pub fn with_property<S: Into<String>, V: Into<String>>(
        mut self, 
        key: S, 
        value: V
    ) -> Self {
        if self.properties.is_none() {
            self.properties = Some(HashMap::new());
        }
        
        if let Some(props) = &mut self.properties {
            props.insert(key.into(), value.into());
        }
        
        self
    }
    pub fn parse_from_string(block_state_str: &str) -> Result<Self, BlockStateParseError> {
        let parsed = BlockStateParser::parse(block_state_str)?;
        
        Ok(Self {
            name: parsed.block_id,
            properties: if parsed.properties.is_empty() {
                None
            } else {
                Some(parsed.properties)
            },
        })
    }
    
    /// 获取属性值，如果属性不存在则返回 None
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties
            .as_ref()
            .and_then(|props| props.get(key))
            .map(|s| s.as_str())
    }
    
    /// 检查是否包含特定属性
    pub fn has_property(&self, key: &str) -> bool {
        self.properties
            .as_ref()
            .map_or(false, |props| props.contains_key(key))
    }
    
    /// 获取所有属性名的迭代器
    pub fn property_names(&self) -> impl Iterator<Item = &String> {
        self.properties
            .as_ref()
            .into_iter()
            .flat_map(|props| props.keys())
    }
    
    /// 转换为字符串表示（用于调试和显示）
    pub fn to_string_representation(&self) -> String {
        if let Some(props) = &self.properties {
            let prop_strings: Vec<String> = props
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            
            format!("{}[{}]", self.name, prop_strings.join(","))
        } else {
            self.name.clone()
        }
    }
}
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
    #[error("方块状态数据未初始化")]
    RegistryNotInitialized,
    #[error("解析错误: {source}")]
    ParserError {
        #[from]  // 这会自动实现 From 转换
        source: qexed_block::BlockStateParseError,
    },
}

// 为 BlockStates 实现 From 转换
impl From<BlockStateParseError> for String {
    fn from(error: BlockStateParseError) -> Self {
        error.to_string()
    }
}