// crates/data/qexed_nbt/src/lib.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error; // 用于清晰、链式的错误处理
pub mod net;
pub mod nbt_net;
pub mod nbt_serde;
#[derive(Debug, Clone, PartialEq)]
pub enum Tag {
    // 基础数值类型
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    // 使用Arc支持零拷贝和共享
    String(Arc<str>),
    ByteArray(Arc<[i8]>),
    IntArray(Arc<[i32]>),
    LongArray(Arc<[i64]>),
    // 容器类型
    List(ListHeader, Arc<[Tag]>),
    Compound(Arc<HashMap<String, Tag>>),
    // 特殊类型
    End,
}
impl Default for Tag {
    fn default() -> Self {
        Tag::Compound(std::sync::Arc::new(std::collections::HashMap::new()))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListHeader {
    pub tag_id: u8,  // 内部元素的NBT类型ID
    pub length: i32,
}

// 为方便使用，定义NBT类型ID常量
pub mod tag_id {
    pub const END: u8 = 0;
    pub const BYTE: u8 = 1;
    pub const SHORT: u8 = 2;
    pub const INT: u8 = 3;
    pub const LONG: u8 = 4;
    pub const FLOAT: u8 = 5;
    pub const DOUBLE: u8 = 6;
    pub const BYTE_ARRAY: u8 = 7;
    pub const STRING: u8 = 8;
    pub const LIST: u8 = 9;
    pub const COMPOUND: u8 = 10;
    pub const INT_ARRAY: u8 = 11;
    pub const LONG_ARRAY: u8 = 12;
}

// 在 src/lib.rs 中，修改 NbtError 的定义和实现

// 修改 NbtError 的定义，添加更多错误变体
#[derive(Error, Debug)]
pub enum NbtError {
    #[error("序列化错误: {0}")]
    Serialize(String),
    #[error("反序列化错误: {0}")]
    Deserialize(String),
    #[error("列表元素类型不一致，期望类型ID: {expected}, 实际: {actual}")]
    ListTypeMismatch { expected: u8, actual: u8 },
    #[error("IO错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("不支持的类型转换")]
    UnsupportedConversion,
    #[error("超出范围的值")]
    OutOfRange,
    #[error("类型不匹配: 期望 {expected}, 实际 {actual}")]
    TypeMismatch { expected: String, actual: String },
    #[error("缺少字段: {0}")]
    MissingField(String),
    #[error("自定义错误: {0}")]
    Custom(String),
}

// 实现 serde::ser::Error trait
impl serde::ser::Error for NbtError {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        NbtError::Serialize(msg.to_string())
    }
}

// 实现 serde::de::Error trait
impl serde::de::Error for NbtError {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        NbtError::Deserialize(msg.to_string())
    }
}
// 在 src/lib.rs 中继续
impl Serialize for Tag {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Tag::Byte(v) => serializer.serialize_i8(*v),
            Tag::Short(v) => serializer.serialize_i16(*v),
            Tag::Int(v) => serializer.serialize_i32(*v),
            Tag::Long(v) => serializer.serialize_i64(*v),
            Tag::Float(v) => serializer.serialize_f32(*v),
            Tag::Double(v) => serializer.serialize_f64(*v),
            Tag::String(v) => serializer.serialize_str(v),
            // 注意：数组序列化为Vec。如果直接序列化Arc<[T]>，某些格式可能不支持。
            Tag::ByteArray(v) => v.serialize(serializer),
            Tag::IntArray(v) => v.serialize(serializer),
            Tag::LongArray(v) => v.serialize(serializer),
            // List: 需要展平结构。一种策略是先序列化为（header, vec），但更常见的是直接序列化为Vec。
            // 这里简化处理，直接序列化内部Vec。同质性约束由构造器保证。
            Tag::List(_, v) => v.serialize(serializer),
            // Compound: 类似处理
            Tag::Compound(v) => v.serialize(serializer),
            Tag::End => serializer.serialize_unit(), // 将End视为空单元
        }
    }
}

// 实现一个辅助函数，从Tag变体获取其对应的NBT类型ID
impl Tag {
    pub fn tag_id(&self) -> u8 {
        match self {
            Tag::End => tag_id::END,
            Tag::Byte(_) => tag_id::BYTE,
            Tag::Short(_) => tag_id::SHORT,
            Tag::Int(_) => tag_id::INT,
            Tag::Long(_) => tag_id::LONG,
            Tag::Float(_) => tag_id::FLOAT,
            Tag::Double(_) => tag_id::DOUBLE,
            Tag::ByteArray(_) => tag_id::BYTE_ARRAY,
            Tag::String(_) => tag_id::STRING,
            Tag::List(_, _) => tag_id::LIST,
            Tag::Compound(_) => tag_id::COMPOUND,
            Tag::IntArray(_) => tag_id::INT_ARRAY,
            Tag::LongArray(_) => tag_id::LONG_ARRAY,
        }
    }
}
// 在 impl Tag 块中继续
impl Tag {
    /// 安全地创建一个同质List
    pub fn new_list(tag_id: u8, items: Vec<Tag>) -> Result<Self, NbtError> {
        if items.is_empty() {
            // 空列表是允许的，header中的tag_id通常为End (0)
            return Ok(Tag::List(
                ListHeader { tag_id, length: 0 },
                Arc::new([]),
            ));
        }
        // 检查所有元素类型是否一致
        let first_id = items[0].tag_id();
        if !items.iter().all(|tag| tag.tag_id() == first_id) {
            return Err(NbtError::ListTypeMismatch {
                expected: first_id,
                actual: items.iter().find(|t| t.tag_id() != first_id).unwrap().tag_id(),
            });
        }
        if first_id != tag_id {
            // 提供的tag_id与实际元素类型不符，这是一个逻辑错误
            return Err(NbtError::Deserialize(format!(
                "List tag_id mismatch. Expected {}, got {}",
                tag_id, first_id
            )));
        }
        Ok(Tag::List(
            ListHeader {
                tag_id,
                length: items.len() as i32,
            },
            Arc::from(items),
        ))
    }

    /// 便捷方法：从 &[u8] 创建 ByteArray（处理有/无符号转换）
    pub fn byte_array_from_u8_slice(data: &[u8]) -> Self {
        // 将 u8 转换为 i8（位模式不变，解释不同）
        let i8_data: Vec<i8> = data.iter().map(|&b| b as i8).collect();
        Tag::ByteArray(Arc::from(i8_data))
    }

    /// 便捷方法：从 Vec<String> 创建 String List
    pub fn string_list(strings: Vec<String>) -> Result<Self, NbtError> {
        let tags: Vec<Tag> = strings
            .into_iter()
            .map(|s| Tag::String(Arc::from(s)))
            .collect();
        Self::new_list(tag_id::STRING, tags)
    }
}

// crates/data/qexed_nbt/src/lib.rs 新增部分
use std::io::{Read, Write, Cursor, Seek, SeekFrom};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

/// NBT 读写器，封装了所有二进制格式操作
pub struct NbtIo;

impl NbtIo {
    /// 从字节流读取一个完整的 NBT 结构（含根标签名）
    pub fn from_reader<R: Read>(mut reader: R) -> Result<(String, Tag), NbtError> {
        let tag_id = reader.read_u8()?;
        
        if tag_id != tag_id::COMPOUND {
            return Err(NbtError::Deserialize(format!(
                "根标签必须是 Compound (0x0A)，实际是 0x{:02X}",
                tag_id
            )));
        }
        
        let name = Self::read_string(&mut reader)?;
        let compound = Self::read_compound(&mut reader)?;
        
        Ok((name, Tag::Compound(Arc::new(compound))))
    }
    
    /// 将 NBT 结构写入字节流
    pub fn to_writer<W: Write>(mut writer: W, name: &str, tag: &Tag) -> Result<(), NbtError> {
        match tag {
            Tag::Compound(compound) => {
                writer.write_u8(tag_id::COMPOUND)?;
                Self::write_string(&mut writer, name)?;
                Self::write_compound_content(&mut writer, compound)?;
                Ok(())
            }
            _ => Err(NbtError::Serialize("根标签必须是 Compound".to_string())),
        }
    }
    
    /// 读取单个标签（不包含标签ID）
    fn read_tag<R: Read>(reader: &mut R, tag_id: u8) -> Result<Tag, NbtError> {
        match tag_id {
            tag_id::END => Ok(Tag::End),
            tag_id::BYTE => Ok(Tag::Byte(reader.read_i8()?)),
            tag_id::SHORT => Ok(Tag::Short(reader.read_i16::<BigEndian>()?)),
            tag_id::INT => Ok(Tag::Int(reader.read_i32::<BigEndian>()?)),
            tag_id::LONG => Ok(Tag::Long(reader.read_i64::<BigEndian>()?)),
            tag_id::FLOAT => Ok(Tag::Float(reader.read_f32::<BigEndian>()?)),
            tag_id::DOUBLE => Ok(Tag::Double(reader.read_f64::<BigEndian>()?)),
            tag_id::BYTE_ARRAY => Ok(Tag::ByteArray(Arc::from(Self::read_byte_array(reader)?))),
            tag_id::STRING => Ok(Tag::String(Arc::from(Self::read_string(reader)?))),
            tag_id::LIST => Self::read_list(reader),
            tag_id::COMPOUND => Ok(Tag::Compound(Arc::new(Self::read_compound(reader)?))),
            tag_id::INT_ARRAY => Ok(Tag::IntArray(Arc::from(Self::read_int_array(reader)?))),
            tag_id::LONG_ARRAY => Ok(Tag::LongArray(Arc::from(Self::read_long_array(reader)?))),
            id => Err(NbtError::Deserialize(format!("未知的标签ID: 0x{:02X}", id))),
        }
    }
    
    /// 写入单个命名标签（包含标签ID和名字）
    fn write_tag<W: Write>(writer: &mut W, name: &str, tag: &Tag) -> Result<(), NbtError> {
        writer.write_u8(tag.tag_id())?;
        Self::write_string(writer, name)?;
        Self::write_tag_value(writer, tag)
    }
    
    /// 写入标签值（不包含标签ID和名字）
    fn write_tag_value<W: Write>(writer: &mut W, tag: &Tag) -> Result<(), NbtError> {
        match tag {
            Tag::End => Ok(()),
            Tag::Byte(v) => Ok(writer.write_i8(*v)?),
            Tag::Short(v) => Ok(writer.write_i16::<BigEndian>(*v)?),
            Tag::Int(v) => Ok(writer.write_i32::<BigEndian>(*v)?),
            Tag::Long(v) => Ok(writer.write_i64::<BigEndian>(*v)?),
            Tag::Float(v) => Ok(writer.write_f32::<BigEndian>(*v)?),
            Tag::Double(v) => Ok(writer.write_f64::<BigEndian>(*v)?),
            Tag::String(v) => Self::write_string(writer, v),
            Tag::ByteArray(v) => Self::write_byte_array(writer, v),
            Tag::IntArray(v) => Self::write_int_array(writer, v),
            Tag::LongArray(v) => Self::write_long_array(writer, v),
            Tag::List(header, items) => Self::write_list_content(writer, header, items),
            Tag::Compound(map) => Self::write_compound_content(writer, map),
        }
    }
    
    // === 各种类型的读写实现 ===
    
    fn read_string<R: Read>(reader: &mut R) -> Result<String, NbtError> {
        let length = reader.read_u16::<BigEndian>()? as usize;
        if length == 0 {
            return Ok(String::new());
        }
        
        let mut buffer = vec![0u8; length];
        reader.read_exact(&mut buffer)?;
        
        String::from_utf8(buffer)
            .map_err(|e| NbtError::Deserialize(format!("字符串UTF-8错误: {}", e)))
    }
    
    fn write_string<W: Write>(writer: &mut W, s: &str) -> Result<(), NbtError> {
        let bytes = s.as_bytes();
        let length = bytes.len();
        
        if length > u16::MAX as usize {
            return Err(NbtError::Serialize(format!(
                "字符串过长: {}字节 (最大: {})",
                length, u16::MAX
            )));
        }
        
        writer.write_u16::<BigEndian>(length as u16)?;
        writer.write_all(bytes)?;
        Ok(())
    }
    
    fn read_byte_array<R: Read>(reader: &mut R) -> Result<Vec<i8>, NbtError> {
        let length = reader.read_i32::<BigEndian>()?;
        if length < 0 {
            return Err(NbtError::Deserialize(format!(
                "ByteArray 长度不能为负数: {}",
                length
            )));
        }
        
        let mut array = vec![0i8; length as usize];
        let mut byte_buf = vec![0u8; length as usize];
        reader.read_exact(&mut byte_buf)?;
        
        for (i, &byte) in byte_buf.iter().enumerate() {
            array[i] = byte as i8;
        }
        
        Ok(array)
    }
    
    fn write_byte_array<W: Write>(writer: &mut W, array: &[i8]) -> Result<(), NbtError> {
        writer.write_i32::<BigEndian>(array.len() as i32)?;
        for &byte in array {
            writer.write_u8(byte as u8)?;
        }
        Ok(())
    }
    
    fn read_int_array<R: Read>(reader: &mut R) -> Result<Vec<i32>, NbtError> {
        let length = reader.read_i32::<BigEndian>()?;
        if length < 0 {
            return Err(NbtError::Deserialize(format!(
                "IntArray 长度不能为负数: {}",
                length
            )));
        }
        
        let mut array = vec![0i32; length as usize];
        for i in 0..length as usize {
            array[i] = reader.read_i32::<BigEndian>()?;
        }
        Ok(array)
    }
    
    fn write_int_array<W: Write>(writer: &mut W, array: &[i32]) -> Result<(), NbtError> {
        writer.write_i32::<BigEndian>(array.len() as i32)?;
        for &value in array {
            writer.write_i32::<BigEndian>(value)?;
        }
        Ok(())
    }
    
    fn read_long_array<R: Read>(reader: &mut R) -> Result<Vec<i64>, NbtError> {
        let length = reader.read_i32::<BigEndian>()?;
        if length < 0 {
            return Err(NbtError::Deserialize(format!(
                "LongArray 长度不能为负数: {}",
                length
            )));
        }
        
        let mut array = vec![0i64; length as usize];
        for i in 0..length as usize {
            array[i] = reader.read_i64::<BigEndian>()?;
        }
        Ok(array)
    }
    
    fn write_long_array<W: Write>(writer: &mut W, array: &[i64]) -> Result<(), NbtError> {
        writer.write_i32::<BigEndian>(array.len() as i32)?;
        for &value in array {
            writer.write_i64::<BigEndian>(value)?;
        }
        Ok(())
    }
    
    fn read_list<R: Read>(reader: &mut R) -> Result<Tag, NbtError> {
        let tag_id = reader.read_u8()?;
        let length = reader.read_i32::<BigEndian>()?;
        
        if length == 0 {
            return Ok(Tag::List(
                ListHeader { tag_id, length: 0 },
                Arc::new([]),
            ));
        }
        
        let mut items = Vec::with_capacity(length as usize);
        for _ in 0..length {
            let item = Self::read_tag(reader, tag_id)?;
            
            if item.tag_id() != tag_id {
                return Err(NbtError::ListTypeMismatch {
                    expected: tag_id,
                    actual: item.tag_id(),
                });
            }
            
            items.push(item);
        }
        
        Ok(Tag::List(
            ListHeader { tag_id, length },
            Arc::from(items),
        ))
    }
    
    /// 写入List内容（包含头部）
    fn write_list_content<W: Write>(
        writer: &mut W,
        header: &ListHeader,
        items: &[Tag],
    ) -> Result<(), NbtError> {
        writer.write_u8(header.tag_id)?;
        writer.write_i32::<BigEndian>(header.length)?;
        
        for item in items {
            Self::write_tag_value(writer, item)?;
        }
        Ok(())
    }
    
    fn read_compound<R: Read>(reader: &mut R) -> Result<HashMap<String, Tag>, NbtError> {
        let mut map = HashMap::new();
        
        loop {
            let tag_id = reader.read_u8()?;
            if tag_id == tag_id::END {
                break;
            }
            
            let name = Self::read_string(reader)?;
            let tag = Self::read_tag(reader, tag_id)?;
            map.insert(name, tag);
        }
        
        Ok(map)
    }
    
    /// 写入Compound内容（不包含标签ID和名字）
    fn write_compound_content<W: Write>(
        writer: &mut W,
        map: &HashMap<String, Tag>,
    ) -> Result<(), NbtError> {
        for (name, tag) in map {
            Self::write_tag(writer, name, tag)?;
        }
        writer.write_u8(tag_id::END)?;
        Ok(())
    }
}

/// 便捷函数：从文件读取（自动处理Gzip压缩）
pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<(String, Tag), NbtError> {
    use std::fs::File;
    use std::io::BufReader;
    use flate2::read::GzDecoder;
    
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    
    // 检查Gzip魔数
    let mut magic_bytes = [0u8; 2];
    reader.read_exact(&mut magic_bytes)?;
    reader.seek(SeekFrom::Start(0))?; // 重置位置
    
    let is_gzipped = magic_bytes == [0x1F, 0x8B];
    
    if is_gzipped {
        let decoder = GzDecoder::new(reader);
        NbtIo::from_reader(decoder)
    } else {
        NbtIo::from_reader(reader)
    }
}

/// 便捷函数：写入文件（可选Gzip压缩）
pub fn to_file<P: AsRef<std::path::Path>>(
    path: P,
    name: &str,
    tag: &Tag,
    compress: bool,
) -> Result<(), NbtError> {
    use std::fs::File;
    use std::io::BufWriter;
    use flate2::{write::GzEncoder, Compression};
    
    let file = File::create(path)?;
    
    if compress {
        let encoder = GzEncoder::new(BufWriter::new(file), Compression::default());
        NbtIo::to_writer(encoder, name, tag)
    } else {
        NbtIo::to_writer(BufWriter::new(file), name, tag)
    }
}

/// 从字节切片读取
pub fn from_slice(data: &[u8]) -> Result<(String, Tag), NbtError> {
    NbtIo::from_reader(Cursor::new(data))
}

/// 写入字节向量
pub fn to_vec(name: &str, tag: &Tag) -> Result<Vec<u8>, NbtError> {
    let mut buffer = Vec::new();
    NbtIo::to_writer(&mut buffer, name, tag)?;
    Ok(buffer)
}
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_basic_roundtrip() -> Result<(), NbtError> {
        // 创建一个测试 Compound
        use std::collections::HashMap;
        
        let mut map = HashMap::new();
        map.insert("TestByte".to_string(), Tag::Byte(42));
        map.insert("TestInt".to_string(), Tag::Int(1000));
        map.insert("TestFloat".to_string(), Tag::Float(3.14));
        map.insert("TestString".to_string(), Tag::String(Arc::from("Hello NBT")));
        
        // 创建 List
        let list = Tag::new_list(
            tag_id::INT,
            vec![Tag::Int(1), Tag::Int(2), Tag::Int(3)],
        )?;
        map.insert("TestList".to_string(), list);
        
        // 创建 ByteArray
        let byte_array = Tag::byte_array_from_u8_slice(&[0x00, 0x01, 0x02, 0x03]);
        map.insert("TestByteArray".to_string(), byte_array);
        
        let compound = Tag::Compound(Arc::new(map));
        
        // 序列化和反序列化
        let data = to_vec("TestRoot", &compound)?;
        let (name, decoded) = from_slice(&data)?;
        
        assert_eq!(name, "TestRoot");
        assert_eq!(compound, decoded);
        
        Ok(())
    }
    
    #[test]
    fn test_list_homogeneity() {
        // 测试 List 同质性检查
        let result = Tag::new_list(
            tag_id::INT,
            vec![Tag::Int(1), Tag::Byte(2)], // 类型不一致！
        );
        
        assert!(result.is_err());
        if let Err(NbtError::ListTypeMismatch { expected, actual }) = result {
            assert_eq!(expected, tag_id::INT);
            assert_eq!(actual, tag_id::BYTE);
        } else {
            panic!("应该返回 ListTypeMismatch 错误");
        }
    }
    
    #[test]
    fn test_arrays() -> Result<(), NbtError> {
        // 测试各种数组类型
        let mut map = HashMap::new();
        
        // IntArray
        let int_array = Tag::IntArray(Arc::from([1, 2, 3, 4, 5]));
        map.insert("IntArray".to_string(), int_array);
        
        // LongArray
        let long_array = Tag::LongArray(Arc::from([1000i64, 2000, 3000]));
        map.insert("LongArray".to_string(), long_array);
        
        let compound = Tag::Compound(Arc::new(map));
        let data = to_vec("ArrayTest", &compound)?;
        let (_, decoded) = from_slice(&data)?;
        
        assert_eq!(compound, decoded);
        Ok(())
    }
}