use serde::{Deserialize, Deserializer, Serialize, Serializer, ser::SerializeSeq};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NbtUuid(pub Uuid);

// 手动实现 Serialize：将 UUID 转为 List<Int>（4个i32整数）
impl Serialize for NbtUuid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let bytes = self.0.as_bytes();
        
        // 将 16 字节拆分为 4 个 i32（大端序）
        let part1 = i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let part2 = i32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let part3 = i32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let part4 = i32::from_be_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        
        let mut seq = serializer.serialize_seq(Some(4))?; // 固定长度4
        seq.serialize_element(&part1)?;
        seq.serialize_element(&part2)?;
        seq.serialize_element(&part3)?;
        seq.serialize_element(&part4)?;
        seq.end()
    }
}

// 手动实现 Deserialize：从 List<Int> 解析为 UUID
impl<'de> Deserialize<'de> for NbtUuid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // 反序列化为 Vec<i32>
        let values = <Vec<i32>>::deserialize(deserializer)?;
        
        // 验证列表长度必须为4
        if values.len() != 4 {
            return Err(serde::de::Error::invalid_length(
                values.len(),
                &"a sequence of exactly 4 elements (UUID parts)",
            ));
        }

        // 将 4 个 i32 合并为 16 字节
        let mut uuid_bytes = [0u8; 16];
        uuid_bytes[0..4].copy_from_slice(&values[0].to_be_bytes());
        uuid_bytes[4..8].copy_from_slice(&values[1].to_be_bytes());
        uuid_bytes[8..12].copy_from_slice(&values[2].to_be_bytes());
        uuid_bytes[12..16].copy_from_slice(&values[3].to_be_bytes());
        
        Ok(NbtUuid(Uuid::from_bytes(uuid_bytes)))
    }
}

// 便捷方法和转换
impl NbtUuid {
    pub fn new() -> Self {
        Self(generate_simple_uuid())
    }
    
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
    
    pub fn into_uuid(self) -> Uuid {
        self.0
    }
    
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
    
    // 从字符串解析
    pub fn parse_str(s: &str) -> Result<Self, uuid::Error> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for NbtUuid {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<NbtUuid> for Uuid {
    fn from(nbt_uuid: NbtUuid) -> Self {
        nbt_uuid.0
    }
}

// 简单的 UUID 生成函数
fn generate_simple_uuid() -> Uuid {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    
    let nanos = duration.as_nanos();
    let mut bytes = [0u8; 16];
    
    // 使用时间戳填充 UUID
    let time_bytes = nanos.to_be_bytes();
    bytes[0..8].copy_from_slice(&time_bytes[8..16]); // 使用后8字节
    bytes[8..16].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]); // 简单随机部分
    
    Uuid::from_bytes(bytes)
}

// 测试用例
