// qexed_nbt: Minecraft NBT (named binary tag) tree + Java big-endian codecs.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

pub mod named;
pub mod net;

pub use named::{from_file, from_slice, from_slice_lossy, to_file, to_vec, NbtIo};
pub mod nbt_serde;

#[derive(Error, Debug)]
pub enum NbtError {
    #[error("serialize: {0}")]
    Serialize(String),
    #[error("deserialize: {0}")]
    Deserialize(String),
    #[error("list element type mismatch: expected id {expected}, got {actual}")]
    ListTypeMismatch { expected: u8, actual: u8 },
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported conversion")]
    UnsupportedConversion,
    #[error("out of range")]
    OutOfRange,
    #[error("type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },
    #[error("missing field: {0}")]
    MissingField(String),
}

impl serde::ser::Error for NbtError {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        NbtError::Serialize(msg.to_string())
    }
}

impl serde::de::Error for NbtError {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        NbtError::Deserialize(msg.to_string())
    }
}

/// NBT tag type ids (Java edition).
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListHeader {
    pub tag_id: u8,
    pub length: i32,
}

/// NBT value tree; compounds and lists share their buffers (Arc).
#[derive(Debug, Clone, PartialEq)]
pub enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    String(Arc<str>),
    ByteArray(Arc<[i8]>),
    IntArray(Arc<[i32]>),
    LongArray(Arc<[i64]>),
    List(ListHeader, Arc<[Tag]>),
    Compound(Arc<HashMap<String, Tag>>),
    End,
}

impl Default for Tag {
    fn default() -> Self {
        Tag::Compound(Arc::new(HashMap::new()))
    }
}

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

    /// Builds a homogeneous list tag; errors when element types differ.
    pub fn new_list(tag_id: u8, items: Vec<Tag>) -> Result<Self, NbtError> {
        if let Some(first) = items.first() {
            let first_id = first.tag_id();
            if let Some(bad) = items.iter().find(|t| t.tag_id() != first_id) {
                return Err(NbtError::ListTypeMismatch { expected: first_id, actual: bad.tag_id() });
            }
            if first_id != tag_id {
                return Err(NbtError::ListTypeMismatch { expected: tag_id, actual: first_id });
            }
        }
        let length = items.len() as i32;
        Ok(Tag::List(ListHeader { tag_id, length }, Arc::from(items)))
    }

    pub fn byte_array_from_u8_slice(data: &[u8]) -> Self {
        Tag::ByteArray(Arc::from(data.iter().map(|&b| b as i8).collect::<Vec<_>>()))
    }

    pub fn string_list(strings: Vec<String>) -> Result<Self, NbtError> {
        Self::new_list(
            tag_id::STRING,
            strings.into_iter().map(|s| Tag::String(Arc::from(s))).collect(),
        )
    }
}

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
            Tag::ByteArray(v) => v.serialize(serializer),
            Tag::IntArray(v) => v.serialize(serializer),
            Tag::LongArray(v) => v.serialize(serializer),
            // lists/compounds serialize as plain sequences; homogeneity
            // is guaranteed by construction
            Tag::List(_, v) => v.serialize(serializer),
            Tag::Compound(v) => v.serialize(serializer),
            Tag::End => serializer.serialize_unit(),
        }
    }
}