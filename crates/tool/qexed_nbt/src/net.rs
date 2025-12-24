// crates/data/qexed_nbt/src/net.rs
use crate::{NbtError, Tag, tag_id};
use std::io::{Read, Write, Cursor};
use byteorder::{ ReadBytesExt, WriteBytesExt};
use std::collections::HashMap;
use std::sync::Arc;

/// 网络数据包NBT读写器
pub struct NetNbtIo;

impl NetNbtIo {
    /// 从网络数据流读取NBT（无根标签名，可选长度前缀）
    pub fn from_reader<R: Read>(
        mut reader: R,
        has_length_prefix: bool,
    ) -> Result<Tag, NbtError> {
        // 处理可选的长度前缀（VarInt类型）
        if has_length_prefix {
            let _length = read_varint(&mut reader)?;
            // 长度值通常用于验证或跳过，这里可以选择读取后忽略
        }
        
        let tag_id = reader.read_u8()?;
        if tag_id == tag_id::END {
            // 网络协议中，单个0x00可能表示null
            return Ok(Tag::End);
        }
        
        // 网络NBT没有根标签名，直接读取Compound内容
        if tag_id != tag_id::COMPOUND {
            return Err(NbtError::Deserialize(format!(
                "网络NBT根元素必须是Compound，实际是0x{:02X}",
                tag_id
            )));
        }
        
        // 注意：这里不读取标签名！
        let compound = Self::read_compound_content(&mut reader)?;
        Ok(Tag::Compound(Arc::new(compound)))
    }
    
    /// 将NBT写入网络数据流
    pub fn to_writer<W: Write>(
        mut writer: W,
        tag: &Tag,
        has_length_prefix: bool,
    ) -> Result<(), NbtError> {
        let mut buffer = Vec::new();
        
        match tag {
            Tag::End => {
                // 网络协议中，null值表示为单个0x00
                buffer.write_u8(tag_id::END)?;
            }
            Tag::Compound(map) => {
                buffer.write_u8(tag_id::COMPOUND)?;
                // 注意：不写入标签名！
                Self::write_compound_content(&mut buffer, map)?;
            }
            _ => {
                return Err(NbtError::Serialize(
                    "网络NBT根元素只能是Compound或null".to_string()
                ));
            }
        }
        
        // 添加长度前缀（如果需要）
        if has_length_prefix {
            write_varint(&mut writer, buffer.len() as i32)?;
        }
        
        writer.write_all(&buffer)?;
        Ok(())
    }
    
    // 复用你现有的读写方法（需从NbtIo中提取或调整）
    fn read_compound_content<R: Read>(reader: &mut R) -> Result<HashMap<String, Tag>, NbtError> {
        // 这里可以复用NbtIo::read_compound的逻辑，但需要调整
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
    
    fn write_compound_content<W: Write>(
        writer: &mut W,
        map: &HashMap<String, Tag>,
    ) -> Result<(), NbtError> {
        for (name, tag) in map {
            writer.write_u8(tag.tag_id())?;
            Self::write_string(writer, name)?;
            Self::write_tag_value(writer, tag)?;
        }
        writer.write_u8(tag_id::END)?;
        Ok(())
    }
    
    // 以下方法需要从NbtIo中提取或复制（字符串、数组、List等的读写）
    // 为了简洁，这里省略具体实现，你应该复用现有代码
    fn read_string<R: Read>(reader: &mut R) -> Result<String, NbtError> {
        // 复用NbtIo中的实现
        crate::NbtIo::read_string(reader)
    }
    
    fn write_string<W: Write>(writer: &mut W, s: &str) -> Result<(), NbtError> {
        // 复用NbtIo中的实现
        crate::NbtIo::write_string(writer, s)
    }
    
    fn read_tag<R: Read>(reader: &mut R, tag_id: u8) -> Result<Tag, NbtError> {
        // 复用NbtIo中的实现
        crate::NbtIo::read_tag(reader, tag_id)
    }
    
    fn write_tag_value<W: Write>(writer: &mut W, tag: &Tag) -> Result<(), NbtError> {
        // 复用NbtIo中的实现
        crate::NbtIo::write_tag_value(writer, tag)
    }
}

/// 读取VarInt（Minecraft协议的可变长度整数）
fn read_varint<R: Read>(reader: &mut R) -> Result<i32, NbtError> {
    let mut result = 0;
    let mut shift = 0;
    
    loop {
        let byte = reader.read_u8()?;
        result |= ((byte & 0x7F) as i32) << shift;
        shift += 7;
        
        if (byte & 0x80) == 0 {
            break;
        }
        
        if shift >= 32 {
            return Err(NbtError::Deserialize("VarInt太大".to_string()));
        }
    }
    
    // 处理符号扩展（Java的有符号整数）
    if (result & (1 << 31)) != 0 {
        result |= !0 << 31;
    }
    
    Ok(result)
}

/// 写入VarInt
fn write_varint<W: Write>(writer: &mut W, mut value: i32) -> Result<(), NbtError> {
    loop {
        let mut byte = (value as u8) & 0x7F;
        value = (value as i32) >> 7;
        
        if value != 0 {
            byte |= 0x80;
        }
        
        writer.write_u8(byte)?;
        
        if value == 0 {
            break;
        }
    }
    
    Ok(())
}

/// 便捷函数：从字节切片读取网络NBT
pub fn from_net_slice(
    data: &[u8],
    has_length_prefix: bool,
) -> Result<Tag, NbtError> {
    NetNbtIo::from_reader(Cursor::new(data), has_length_prefix)
}

/// 便捷函数：将NBT写入字节向量（网络格式）
pub fn to_net_vec(
    tag: &Tag,
    has_length_prefix: bool,
) -> Result<Vec<u8>, NbtError> {
    let mut buffer = Vec::new();
    NetNbtIo::to_writer(&mut buffer, tag, has_length_prefix)?;
    Ok(buffer)
}