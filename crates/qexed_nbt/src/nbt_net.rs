// src/codec/nbt_net.rs
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::Arc;

use crate::{ListHeader, Tag, tag_id};

/// Java版网络格式序列化：将Tag写入writer
pub fn serialize_java<W: Write>(w: &mut W, tag: &Tag) -> Result<()> {
    match tag {
        Tag::End => {
            w.write_u8(tag_id::END)?;
        }
        Tag::Compound(map) => {
            w.write_u8(tag_id::COMPOUND)?;
            // 注意：网络格式省略根标签名
            write_compound_content_java(w, map)?;
        }
        _ => {
            return Err(anyhow!("网络NBT根元素只能是Compound或End"));
        }
    }
    Ok(())
}

/// Java版网络格式反序列化：从reader读取Tag
pub fn deserialize_java<R: Read>(r: &mut R) -> Result<Tag> {
    let tag_id = r.read_u8()?;
    match tag_id {
        tag_id::END => Ok(Tag::End),
        tag_id::COMPOUND => {
            let map = read_compound_content_java(r)?;
            Ok(Tag::Compound(Arc::new(map)))
        }
        _ => Err(anyhow!("网络NBT根元素必须是Compound或End，实际是: 0x{:02X}", tag_id)),
    }
}

// --- 以下是具体的读写辅助函数 ---

fn write_compound_content_java<W: Write>(w: &mut W, map: &HashMap<String, Tag>) -> Result<()> {
    for (name, tag) in map {
        w.write_u8(tag.tag_id())?;
        // 写入标签名
        let name_bytes = name.as_bytes();
        w.write_u16::<BigEndian>(name_bytes.len() as u16)?;
        w.write_all(name_bytes)?;
        // 递归写入标签值
        write_tag_value_java(w, tag)?;
    }
    // 写入End标签表示Compound结束
    w.write_u8(tag_id::END)?;
    Ok(())
}

fn write_tag_value_java<W: Write>(w: &mut W, tag: &Tag) -> Result<()> {
    match tag {
        Tag::Byte(v) => w.write_i8(*v)?,
        Tag::Short(v) => w.write_i16::<BigEndian>(*v)?,
        Tag::Int(v) => w.write_i32::<BigEndian>(*v)?,
        Tag::Long(v) => w.write_i64::<BigEndian>(*v)?,
        Tag::Float(v) => w.write_f32::<BigEndian>(*v)?,
        Tag::Double(v) => w.write_f64::<BigEndian>(*v)?,
        Tag::String(v) => {
            let bytes = v.as_bytes();
            w.write_u16::<BigEndian>(bytes.len() as u16)?;
            w.write_all(bytes)?;
        }
        Tag::ByteArray(v) => {
            let slice = v.as_ref();
            w.write_i32::<BigEndian>(slice.len() as i32)?;
            for &byte in slice {
                w.write_u8(byte as u8)?;
            }
        }
        Tag::IntArray(v) => {
            let slice = v.as_ref();
            w.write_i32::<BigEndian>(slice.len() as i32)?;
            for &value in slice {
                w.write_i32::<BigEndian>(value)?;
            }
        }
        Tag::LongArray(v) => {
            let slice = v.as_ref();
            w.write_i32::<BigEndian>(slice.len() as i32)?;
            for &value in slice {
                w.write_i64::<BigEndian>(value)?;
            }
        }
        Tag::List(header, items) => {
            w.write_u8(header.tag_id)?;
            w.write_i32::<BigEndian>(header.length)?;
            for item in items.as_ref() {
                write_tag_value_java(w, item)?;
            }
        }
        Tag::Compound(map) => {
            write_compound_content_java(w, map)?;
        }
        Tag::End => {
            return Err(anyhow!("End标签不应作为值出现"));
        }
    }
    Ok(())
}

fn read_compound_content_java<R: Read>(r: &mut R) -> Result<HashMap<String, Tag>> {
    let mut map = HashMap::new();
    loop {
        let tag_id = r.read_u8()?;
        if tag_id == tag_id::END {
            break;
        }
        // 读取标签名
        let name_len = r.read_u16::<BigEndian>()? as usize;
        let mut name_bytes = vec![0u8; name_len];
        r.read_exact(&mut name_bytes)?;
        let name = String::from_utf8(name_bytes)
            .map_err(|e| anyhow!("标签名UTF-8错误: {}", e))?;
        // 读取标签值
        let tag = read_tag_value_java(r, tag_id)?;
        map.insert(name, tag);
    }
    Ok(map)
}

fn read_tag_value_java<R: Read>(r: &mut R, tag_id: u8) -> Result<Tag> {
    match tag_id {
        tag_id::BYTE => Ok(Tag::Byte(r.read_i8()?)),
        tag_id::SHORT => Ok(Tag::Short(r.read_i16::<BigEndian>()?)),
        tag_id::INT => Ok(Tag::Int(r.read_i32::<BigEndian>()?)),
        tag_id::LONG => Ok(Tag::Long(r.read_i64::<BigEndian>()?)),
        tag_id::FLOAT => Ok(Tag::Float(r.read_f32::<BigEndian>()?)),
        tag_id::DOUBLE => Ok(Tag::Double(r.read_f64::<BigEndian>()?)),
        tag_id::STRING => {
            let len = r.read_u16::<BigEndian>()? as usize;
            let mut bytes = vec![0u8; len];
            r.read_exact(&mut bytes)?;
            let s = String::from_utf8(bytes)
                .map_err(|e| anyhow!("字符串UTF-8错误: {}", e))?;
            Ok(Tag::String(Arc::from(s)))
        }
        tag_id::BYTE_ARRAY => {
            let len = r.read_i32::<BigEndian>()?;
            let mut vec = Vec::with_capacity(len as usize);
            for _ in 0..len {
                vec.push(r.read_i8()?);
            }
            Ok(Tag::ByteArray(Arc::from(vec)))
        }
        tag_id::INT_ARRAY => {
            let len = r.read_i32::<BigEndian>()?;
            let mut vec = Vec::with_capacity(len as usize);
            for _ in 0..len {
                vec.push(r.read_i32::<BigEndian>()?);
            }
            Ok(Tag::IntArray(Arc::from(vec)))
        }
        tag_id::LONG_ARRAY => {
            let len = r.read_i32::<BigEndian>()?;
            let mut vec = Vec::with_capacity(len as usize);
            for _ in 0..len {
                vec.push(r.read_i64::<BigEndian>()?);
            }
            Ok(Tag::LongArray(Arc::from(vec)))
        }
        tag_id::LIST => {
            let elem_type = r.read_u8()?;
            let length = r.read_i32::<BigEndian>()?;
            let mut items = Vec::with_capacity(length as usize);
            for _ in 0..length {
                items.push(read_tag_value_java(r, elem_type)?);
            }
            Ok(Tag::List(
                ListHeader { tag_id: elem_type, length },
                Arc::from(items),
            ))
        }
        tag_id::COMPOUND => {
            let map = read_compound_content_java(r)?;
            Ok(Tag::Compound(Arc::new(map)))
        }
        _ => Err(anyhow!("未知的标签ID: 0x{:02X}", tag_id)),
    }
}