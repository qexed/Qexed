use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use bytes::{Buf, BufMut as _};
use qexed_nbt::{ListHeader, Tag, tag_id};

use crate::{PacketCodec, PacketReader, PacketWriter};

impl PacketCodec for Tag {
    fn serialize(&self, w: &mut PacketWriter) -> Result<()> {
        w.buf.put_u8(self.tag_id());
        write_tag_value(w.buf, self)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> Result<()> {
        if !r.buf.has_remaining() {
            return Err(anyhow::anyhow!("missing NBT tag id"));
        }

        let tag_id = r.buf.get_u8();
        *self = read_tag_value(r.buf, tag_id)?;
        Ok(())
    }
}

fn write_tag_value(buf: &mut bytes::BytesMut, tag: &Tag) -> Result<()> {
    match tag {
        Tag::End => Ok(()),
        Tag::Byte(v) => {
            buf.put_i8(*v);
            Ok(())
        }
        Tag::Short(v) => {
            buf.put_i16(*v);
            Ok(())
        }
        Tag::Int(v) => {
            buf.put_i32(*v);
            Ok(())
        }
        Tag::Long(v) => {
            buf.put_i64(*v);
            Ok(())
        }
        Tag::Float(v) => {
            buf.put_f32(*v);
            Ok(())
        }
        Tag::Double(v) => {
            buf.put_f64(*v);
            Ok(())
        }
        Tag::String(v) => write_string(buf, v.as_bytes()),
        Tag::ByteArray(v) => {
            write_len_i32(buf, v.len(), "NBT byte array")?;
            buf.put_slice(&i8_slice_as_u8(v.as_ref()));
            Ok(())
        }
        Tag::IntArray(v) => {
            write_len_i32(buf, v.len(), "NBT int array")?;
            for value in v.as_ref() {
                buf.put_i32(*value);
            }
            Ok(())
        }
        Tag::LongArray(v) => {
            write_len_i32(buf, v.len(), "NBT long array")?;
            for value in v.as_ref() {
                buf.put_i64(*value);
            }
            Ok(())
        }
        Tag::List(header, items) => {
            if header.length >= 0 && header.length as usize != items.len() {
                return Err(anyhow::anyhow!(
                    "NBT list header length {} does not match item count {}",
                    header.length,
                    items.len()
                ));
            }

            buf.put_u8(header.tag_id);
            write_len_i32(buf, items.len(), "NBT list")?;
            for item in items.as_ref() {
                write_tag_value(buf, item)?;
            }
            Ok(())
        }
        Tag::Compound(map) => {
            for (name, tag) in map.as_ref() {
                buf.put_u8(tag.tag_id());
                write_string(buf, name.as_bytes())?;
                write_tag_value(buf, tag)?;
            }
            buf.put_u8(tag_id::END);
            Ok(())
        }
    }
}

fn read_tag_value(buf: &mut dyn Buf, id: u8) -> Result<Tag> {
    match id {
        tag_id::END => Ok(Tag::End),
        tag_id::BYTE => Ok(Tag::Byte(buf.get_i8())),
        tag_id::SHORT => Ok(Tag::Short(buf.get_i16())),
        tag_id::INT => Ok(Tag::Int(buf.get_i32())),
        tag_id::LONG => Ok(Tag::Long(buf.get_i64())),
        tag_id::FLOAT => Ok(Tag::Float(buf.get_f32())),
        tag_id::DOUBLE => Ok(Tag::Double(buf.get_f64())),
        tag_id::STRING => {
            let bytes = read_string(buf)?;
            Ok(Tag::String(Arc::from(String::from_utf8(bytes)?)))
        }
        tag_id::BYTE_ARRAY => {
            let len = read_len_i32(buf, "NBT byte array")?;
            ensure_remaining_bytes(buf, len, "NBT byte array")?;
            Ok(Tag::ByteArray(Arc::from(read_i8_vec(buf, len))))
        }
        tag_id::INT_ARRAY => {
            let len = read_len_i32(buf, "NBT int array")?;
            ensure_remaining_elements(buf, len, size_of::<i32>(), "NBT int array")?;
            let mut values = Vec::with_capacity(len);
            for _ in 0..len {
                values.push(buf.get_i32());
            }
            Ok(Tag::IntArray(Arc::from(values)))
        }
        tag_id::LONG_ARRAY => {
            let len = read_len_i32(buf, "NBT long array")?;
            ensure_remaining_elements(buf, len, size_of::<i64>(), "NBT long array")?;
            let mut values = Vec::with_capacity(len);
            for _ in 0..len {
                values.push(buf.get_i64());
            }
            Ok(Tag::LongArray(Arc::from(values)))
        }
        tag_id::LIST => {
            let item_id = buf.get_u8();
            let len = read_len_i32(buf, "NBT list")?;
            let mut values = Vec::with_capacity(len);
            for _ in 0..len {
                values.push(read_tag_value(buf, item_id)?);
            }
            Ok(Tag::List(
                ListHeader {
                    tag_id: item_id,
                    length: len as i32,
                },
                Arc::from(values),
            ))
        }
        tag_id::COMPOUND => {
            let mut values = HashMap::new();
            loop {
                let item_id = buf.get_u8();
                if item_id == tag_id::END {
                    break;
                }

                let name = String::from_utf8(read_string(buf)?)?;
                let value = read_tag_value(buf, item_id)?;
                values.insert(name, value);
            }
            Ok(Tag::Compound(Arc::new(values)))
        }
        _ => Err(anyhow::anyhow!("unknown NBT tag id: 0x{:02X}", id)),
    }
}

fn write_string(buf: &mut bytes::BytesMut, bytes: &[u8]) -> Result<()> {
    if bytes.len() > u16::MAX as usize {
        return Err(anyhow::anyhow!("NBT string is too long: {}", bytes.len()));
    }

    buf.put_u16(bytes.len() as u16);
    buf.put_slice(bytes);
    Ok(())
}

fn read_string(buf: &mut dyn Buf) -> Result<Vec<u8>> {
    let len = buf.get_u16() as usize;
    ensure_remaining_bytes(buf, len, "NBT string")?;

    Ok(crate::read_exact_vec(buf, len))
}

fn read_i8_vec(buf: &mut dyn Buf, len: usize) -> Vec<i8> {
    let mut values = vec![0; len];
    buf.copy_to_slice(i8_slice_as_u8_mut(&mut values));
    values
}

fn ensure_remaining_elements(
    buf: &mut dyn Buf,
    len: usize,
    element_size: usize,
    name: &str,
) -> Result<()> {
    let byte_len = len
        .checked_mul(element_size)
        .ok_or_else(|| anyhow::anyhow!("{} byte length overflows: {}", name, len))?;
    ensure_remaining_bytes(buf, byte_len, name)
}

fn ensure_remaining_bytes(buf: &mut dyn Buf, len: usize, name: &str) -> Result<()> {
    if buf.remaining() < len {
        return Err(anyhow::anyhow!(
            "{} length {} exceeds remaining {}",
            name,
            len,
            buf.remaining()
        ));
    }

    Ok(())
}

fn i8_slice_as_u8(values: &[i8]) -> &[u8] {
    // i8 and u8 have identical layout; only the signed interpretation changes.
    unsafe { std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), values.len()) }
}

fn i8_slice_as_u8_mut(values: &mut [i8]) -> &mut [u8] {
    // i8 and u8 have identical layout; only the signed interpretation changes.
    unsafe { std::slice::from_raw_parts_mut(values.as_mut_ptr().cast::<u8>(), values.len()) }
}

fn write_len_i32(buf: &mut bytes::BytesMut, len: usize, name: &str) -> Result<()> {
    if len > i32::MAX as usize {
        return Err(anyhow::anyhow!("{} length {} exceeds i32 max", name, len));
    }

    buf.put_i32(len as i32);
    Ok(())
}

fn read_len_i32(buf: &mut dyn Buf, name: &str) -> Result<usize> {
    let len = buf.get_i32();
    if len < 0 {
        return Err(anyhow::anyhow!("negative {} length: {}", name, len));
    }

    Ok(len as usize)
}
