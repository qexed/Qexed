//! Network (Java edition protocol) NBT codec: root without a name.
use crate::{NbtError, Tag, tag_id, ListHeader};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Arc;

/// Network NBT reader/writer (root compound without a name).
pub struct NetNbtIo;

impl NetNbtIo {
    /// Reads a network NBT root: a single END byte (null) or a nameless compound.
    pub fn from_reader<R: Read>(mut reader: R, _has_length_prefix: bool) -> Result<Tag, NbtError> {
        let id = reader.read_u8()?;
        match id {
            tag_id::END => Ok(Tag::End),
            tag_id::COMPOUND => Ok(Tag::Compound(Arc::new(read_compound_content(&mut reader)?))),
            other => Err(NbtError::Deserialize(format!(
                "network NBT root must be Compound or END, got 0x{other:02X}"))),
        }
    }

    /// Writes a network NBT root (nameless compound, or END for null).
    pub fn to_writer<W: Write>(mut writer: W, tag: &Tag, _has_length_prefix: bool) -> Result<(), NbtError> {
        match tag {
            Tag::End => writer.write_u8(tag_id::END).map_err(Into::into),
            Tag::Compound(map) => {
                writer.write_u8(tag_id::COMPOUND)?;
                write_compound_content(&mut writer, map)
            }
            _ => Err(NbtError::Serialize(
                "network NBT root must be Compound or END".into())),
        }
    }
}

// ===== shared value-level codec (used by network and named formats) =====

pub(crate) fn read_string<R: Read>(r: &mut R) -> Result<String, NbtError> {
    let len = r.read_u16::<BigEndian>()? as usize;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    String::from_utf8(buf).map_err(|e| NbtError::Deserialize(format!("invalid UTF-8 string: {e}")))
}

pub(crate) fn write_string<W: Write>(w: &mut W, s: &str) -> Result<(), NbtError> {
    let bytes = s.as_bytes();
    if bytes.len() > u16::MAX as usize {
        return Err(NbtError::Serialize(format!("string too long: {} bytes", bytes.len())));
    }
    w.write_u16::<BigEndian>(bytes.len() as u16)?;
    w.write_all(bytes)?;
    Ok(())
}

pub(crate) fn read_compound_content<R: Read>(r: &mut R) -> Result<HashMap<String, Tag>, NbtError> {
    let mut map = HashMap::new();
    loop {
        let id = r.read_u8()?;
        if id == tag_id::END {
            return Ok(map);
        }
        let name = read_string(r)?;
        map.insert(name, read_value(r, id)?);
    }
}

pub(crate) fn write_compound_content<W: Write>(
    w: &mut W,
    map: &HashMap<String, Tag>,
) -> Result<(), NbtError> {
    for (name, tag) in map {
        w.write_u8(tag.tag_id())?;
        write_string(w, name)?;
        write_value(w, tag)?;
    }
    w.write_u8(tag_id::END)?;
    Ok(())
}

pub(crate) fn read_value<R: Read>(r: &mut R, id: u8) -> Result<Tag, NbtError> {
    Ok(match id {
        tag_id::END => Tag::End,
        tag_id::BYTE => Tag::Byte(r.read_i8()?),
        tag_id::SHORT => Tag::Short(r.read_i16::<BigEndian>()?),
        tag_id::INT => Tag::Int(r.read_i32::<BigEndian>()?),
        tag_id::LONG => Tag::Long(r.read_i64::<BigEndian>()?),
        tag_id::FLOAT => Tag::Float(r.read_f32::<BigEndian>()?),
        tag_id::DOUBLE => Tag::Double(r.read_f64::<BigEndian>()?),
        tag_id::BYTE_ARRAY => {
            let len = read_len(r)?;
            let mut v = Vec::with_capacity(len);
            for _ in 0..len {
                v.push(r.read_i8()?);
            }
            Tag::ByteArray(Arc::from(v))
        }
        tag_id::STRING => Tag::String(Arc::from(read_string(r)?)),
        tag_id::LIST => {
            let elem = r.read_u8()?;
            let len = r.read_i32::<BigEndian>()?;
            let mut items = Vec::with_capacity(len.max(0) as usize);
            for _ in 0..len {
                items.push(read_value(r, elem)?);
            }
            Tag::List(ListHeader { tag_id: elem, length: len }, Arc::from(items))
        }
        tag_id::COMPOUND => Tag::Compound(Arc::new(read_compound_content(r)?)),
        tag_id::INT_ARRAY => {
            let len = read_len(r)?;
            let mut v = Vec::with_capacity(len);
            for _ in 0..len {
                v.push(r.read_i32::<BigEndian>()?);
            }
            Tag::IntArray(Arc::from(v))
        }
        tag_id::LONG_ARRAY => {
            let len = read_len(r)?;
            let mut v = Vec::with_capacity(len);
            for _ in 0..len {
                v.push(r.read_i64::<BigEndian>()?);
            }
            Tag::LongArray(Arc::from(v))
        }
        other => {
            return Err(NbtError::Deserialize(format!("unknown tag id 0x{other:02X}")))
        }
    })
}

pub(crate) fn write_value<W: Write>(w: &mut W, tag: &Tag) -> Result<(), NbtError> {
    match tag {
        Tag::End => Ok(()),
        Tag::Byte(v) => w.write_i8(*v).map_err(Into::into),
        Tag::Short(v) => w.write_i16::<BigEndian>(*v).map_err(Into::into),
        Tag::Int(v) => w.write_i32::<BigEndian>(*v).map_err(Into::into),
        Tag::Long(v) => w.write_i64::<BigEndian>(*v).map_err(Into::into),
        Tag::Float(v) => w.write_f32::<BigEndian>(*v).map_err(Into::into),
        Tag::Double(v) => w.write_f64::<BigEndian>(*v).map_err(Into::into),
        Tag::String(v) => write_string(w, v),
        Tag::ByteArray(v) => {
            w.write_i32::<BigEndian>(v.len() as i32)?;
            for &b in v.iter() {
                w.write_u8(b as u8)?;
            }
            Ok(())
        }
        Tag::IntArray(v) => {
            w.write_i32::<BigEndian>(v.len() as i32)?;
            for &x in v.iter() {
                w.write_i32::<BigEndian>(x)?;
            }
            Ok(())
        }
        Tag::LongArray(v) => {
            w.write_i32::<BigEndian>(v.len() as i32)?;
            for &x in v.iter() {
                w.write_i64::<BigEndian>(x)?;
            }
            Ok(())
        }
        Tag::List(header, items) => {
            w.write_u8(header.tag_id)?;
            w.write_i32::<BigEndian>(items.len() as i32)?;
            for item in items.iter() {
                write_value(w, item)?;
            }
            Ok(())
        }
        Tag::Compound(map) => write_compound_content(w, map),
    }
}

fn read_len<R: Read>(r: &mut R) -> Result<usize, NbtError> {
    let len = r.read_i32::<BigEndian>()?;
    if len < 0 {
        return Err(NbtError::Deserialize(format!("negative array length: {len}")));
    }
    Ok(len as usize)
}
