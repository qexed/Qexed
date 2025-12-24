// src/codec/nbt.rs
use crate::PacketCodec;
use anyhow::Result;
use qexed_nbt::Tag;

// 注意：我们假设您的PacketCodec定义在 `src/lib.rs` 中
// 并且有一个类似的签名:
// pub trait PacketCodec {
//     fn serialize(&self, w: &mut PacketWriter) -> Result<()>;
//     fn deserialize(&mut self, r: &mut PacketReader) -> Result<()>;
// }

// crates/packet/qexed_packet/src/codec/nbt.rs
use crate::{PacketReader, PacketWriter};
use bytes::{Buf, BufMut, BytesMut};
use qexed_nbt::{tag_id};
use std::collections::HashMap;
use std::sync::Arc;

impl<'a> PacketCodec for Tag {
    fn serialize(&self, w: &mut PacketWriter) -> Result<()> {
        // 获取底层 BytesMut 的引用，它将作为 std::io::Write 被使用
        let writer = &mut w.buf;
        match self {
            Tag::End => {
                writer.put_u8(tag_id::END);
            }
            Tag::Compound(map) => {
                writer.put_u8(tag_id::COMPOUND);
                // 注意：网络格式省略根标签名
                write_compound_content(writer, map)?;
            }
            _ => {
                return Err(anyhow::anyhow!("网络NBT根元素只能是Compound或End"));
            }
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> Result<()> {
        // 注意：这里 self 已经是一个通过 Default::default() 创建的 Tag 实例
        // 我们需要从 reader 中读取数据来替换它
        if !r.buf.has_remaining() {
            return Ok(());
        }
        let tag_id = r.buf.get_u8();
        match tag_id {
            tag_id::END => {
                *self = Tag::End;
            }
            tag_id::COMPOUND => {
                let map = read_compound_content(&mut r.buf)?;
                *self = Tag::Compound(Arc::new(map));
            }
            _ => {
                return Err(anyhow::anyhow!("网络NBT根元素必须是Compound或End，实际是: 0x{:02X}", tag_id));
            }
        }
        Ok(())
    }
}

// === 内部辅助函数：处理 Compound 内容 ===

fn write_compound_content(writer: &mut BytesMut, map: &HashMap<String, Tag>) -> Result<()> {
    for (name, tag) in map {
        writer.put_u8(tag.tag_id());

        // 写入标签名（长度 + UTF-8 字节）
        let name_bytes = name.as_bytes();
        if name_bytes.len() > u16::MAX as usize {
            return Err(anyhow::anyhow!("标签名过长: {}", name_bytes.len()));
        }
        writer.put_u16(name_bytes.len() as u16);
        writer.extend_from_slice(name_bytes);

        // 递归写入标签值
        write_tag_value(writer, tag)?;
    }
    // 写入 End 标签 (0x00) 表示 Compound 结束
    writer.put_u8(tag_id::END);
    Ok(())
}

fn write_tag_value(writer: &mut BytesMut, tag: &Tag) -> Result<()> {
    match tag {
        Tag::Byte(v) => writer.put_i8(*v),
        Tag::Short(v) => writer.put_i16(*v),
        Tag::Int(v) => writer.put_i32(*v),
        Tag::Long(v) => writer.put_i64(*v),
        Tag::Float(v) => writer.put_f32(*v),
        Tag::Double(v) => writer.put_f64(*v),
        Tag::String(v) => {
            let bytes = v.as_bytes();
            writer.put_u16(bytes.len() as u16);
            writer.extend_from_slice(bytes);
        }
        Tag::ByteArray(v) => {
            let slice = v.as_ref();
            writer.put_i32(slice.len() as i32);
            for &byte in slice {
                writer.put_u8(byte as u8);
            }
        }
        Tag::IntArray(v) => {
            let slice = v.as_ref();
            writer.put_i32(slice.len() as i32);
            for &value in slice {
                writer.put_i32(value);
            }
        }
        Tag::LongArray(v) => {
            let slice = v.as_ref();
            writer.put_i32(slice.len() as i32);
            for &value in slice {
                writer.put_i64(value);
            }
        }
        Tag::List(header, items) => {
            writer.put_u8(header.tag_id);
            writer.put_i32(header.length);
            for item in items.as_ref() {
                write_tag_value(writer, item)?;
            }
        }
        Tag::Compound(map) => {
            write_compound_content(writer, map)?;
        }
        Tag::End => {
            // End 标签不应该作为值出现在这里
            return Err(anyhow::anyhow!("End标签不应作为值出现"));
        }
    }
    Ok(())
}

fn read_compound_content(buf: &mut dyn Buf) -> Result<HashMap<String, Tag>> {
    let mut map = HashMap::new();

    loop {
        if !buf.has_remaining() {
            return Err(anyhow::anyhow!("Unexpected EOF while reading compound"));
        }
        let tag_id = buf.get_u8();
        if tag_id == tag_id::END {
            break;
        }

        // 读取标签名
        let name_len = buf.get_u16() as usize;
        if buf.remaining() < name_len {
            return Err(anyhow::anyhow!("Insufficient data for tag name"));
        }
        let name_bytes = buf.copy_to_bytes(name_len);
        let name = String::from_utf8(name_bytes.to_vec())
            .map_err(|e| anyhow::anyhow!("标签名UTF-8错误: {}", e))?;

        // 读取标签值
        let tag = read_tag_value(buf, tag_id)?;
        map.insert(name, tag);
    }

    Ok(map)
}

fn read_tag_value(buf: &mut dyn Buf, tag_id: u8) -> Result<Tag> {
    match tag_id {
        tag_id::BYTE => Ok(Tag::Byte(buf.get_i8())),
        tag_id::SHORT => Ok(Tag::Short(buf.get_i16())),
        tag_id::INT => Ok(Tag::Int(buf.get_i32())),
        tag_id::LONG => Ok(Tag::Long(buf.get_i64())),
        tag_id::FLOAT => Ok(Tag::Float(buf.get_f32())),
        tag_id::DOUBLE => Ok(Tag::Double(buf.get_f64())),
        tag_id::STRING => {
            let len = buf.get_u16() as usize;
            let bytes = buf.copy_to_bytes(len);
            let s = String::from_utf8(bytes.to_vec())
                .map_err(|e| anyhow::anyhow!("字符串UTF-8错误: {}", e))?;
            Ok(Tag::String(Arc::from(s)))
        }
        tag_id::BYTE_ARRAY => {
            let len = buf.get_i32() as usize;
            let mut vec = Vec::with_capacity(len);
            for _ in 0..len {
                vec.push(buf.get_i8());
            }
            Ok(Tag::ByteArray(Arc::from(vec)))
        }
        tag_id::INT_ARRAY => {
            let len = buf.get_i32() as usize;
            let mut vec = Vec::with_capacity(len);
            for _ in 0..len {
                vec.push(buf.get_i32());
            }
            Ok(Tag::IntArray(Arc::from(vec)))
        }
        tag_id::LONG_ARRAY => {
            let len = buf.get_i32() as usize;
            let mut vec = Vec::with_capacity(len);
            for _ in 0..len {
                vec.push(buf.get_i64());
            }
            Ok(Tag::LongArray(Arc::from(vec)))
        }
        tag_id::LIST => {
            let elem_type = buf.get_u8();
            let length = buf.get_i32() as usize;
            let mut items = Vec::with_capacity(length);
            for _ in 0..length {
                items.push(read_tag_value(buf, elem_type)?);
            }
            Ok(Tag::List(
                qexed_nbt::ListHeader {
                    tag_id: elem_type,
                    length: length as i32,
                },
                Arc::from(items),
            ))
        }
        tag_id::COMPOUND => {
            let map = read_compound_content(buf)?;
            Ok(Tag::Compound(Arc::new(map)))
        }
        _ => Err(anyhow::anyhow!("未知的标签ID: 0x{:02X}", tag_id)),
    }
}