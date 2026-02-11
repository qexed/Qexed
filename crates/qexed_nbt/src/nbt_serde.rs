// crates/data/qexed_nbt/src/nbt_serde.rs
use serde::de::{self, Deserialize, Deserializer, IntoDeserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{self, Serialize, SerializeSeq, Serializer};
use super::{Tag, ListHeader, NbtError, tag_id};
use std::sync::Arc;
use std::collections::HashMap;

/// 为 NBT 提供 Serde 序列化/反序列化支持
pub mod nbt_serde {
    use super::*;
    
    /// 将 Rust 值序列化为 NBT Tag
    pub fn to_tag<T>(value: &T) -> Result<Tag, NbtError>
    where
        T: Serialize,
    {
        value.serialize(NbtSerializer)
    }
    
    /// 从 NBT Tag 反序列化为 Rust 值
    pub fn from_tag<'a, T>(tag: &'a Tag) -> Result<T, NbtError>
    where
        T: Deserialize<'a>,
    {
        T::deserialize(NbtDeserializer::new(tag))
    }
    
    /// 从命名 NBT 结构体反序列化
    pub fn from_named_tag<'a, T>(name: &str, tag: &'a Tag) -> Result<T, NbtError>
    where
        T: Deserialize<'a>,
    {
        // 创建一个临时的 Compound 来包裹根标签
        match tag {
            Tag::Compound(map) => {
                // 如果已经是 Compound，检查是否有同名的根标签
                if let Some(root) = map.get(name) {
                    T::deserialize(NbtDeserializer::new(root))
                } else if map.len() == 1 {
                    // 如果只有一个元素，使用它
                    let (_, value) = map.iter().next().unwrap();
                    T::deserialize(NbtDeserializer::new(value))
                } else {
                    // 否则尝试反序列化整个 Compound
                    T::deserialize(NbtDeserializer::new(tag))
                }
            }
            _ => T::deserialize(NbtDeserializer::new(tag)),
        }
    }
}

/// NBT 序列化器
struct NbtSerializer;

impl Serializer for NbtSerializer {
    type Ok = Tag;
    type Error = NbtError;
    type SerializeSeq = NbtSeqSerializer;
    type SerializeTuple = NbtSeqSerializer;
    type SerializeTupleStruct = NbtSeqSerializer;
    type SerializeTupleVariant = NbtVariantSerializer;
    type SerializeMap = NbtMapSerializer;
    type SerializeStruct = NbtMapSerializer;
    type SerializeStructVariant = NbtVariantSerializer;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        Ok(Tag::Byte(v as i8))
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        Ok(Tag::Byte(v))
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        Ok(Tag::Short(v))
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        Ok(Tag::Int(v))
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        Ok(Tag::Long(v))
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        Ok(Tag::Byte(v as i8))
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        Ok(Tag::Short(v as i16))
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        if v <= i32::MAX as u32 {
            Ok(Tag::Int(v as i32))
        } else {
            Ok(Tag::Long(v as i64))
        }
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        if v <= i64::MAX as u64 {
            Ok(Tag::Long(v as i64))
        } else {
            Err(NbtError::Serialize(format!(
                "无法将 u64 值 {} 转换为有符号 NBT 类型",
                v
            )))
        }
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        Ok(Tag::Float(v))
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        Ok(Tag::Double(v))
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(&v.to_string())
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        Ok(Tag::String(Arc::from(v)))
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Ok(Tag::byte_array_from_u8_slice(v))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        // 序列化为 End 标签
        Ok(Tag::End)
    }

    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        // 序列化为 End 标签
        Ok(Tag::End)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        // 序列化为 End 标签
        Ok(Tag::End)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        // 枚举变体序列化为字符串
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        // 创建一个包含变体名和值的 Compound
        let mut map = HashMap::new();
        map.insert(variant.to_string(), value.serialize(self)?);
        Ok(Tag::Compound(Arc::new(map)))
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(NbtSeqSerializer {
            elements: Vec::with_capacity(len.unwrap_or(0)),
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(NbtVariantSerializer {
            variant: variant.to_string(),
            seq: NbtSeqSerializer {
                elements: Vec::with_capacity(len),
            },
        })
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(NbtMapSerializer {
            map: HashMap::with_capacity(len.unwrap_or(0)),
            next_key: None,
        })
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.serialize_map(Some(len))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(NbtVariantSerializer {
            variant: variant.to_string(),
            seq: NbtSeqSerializer {
                elements: Vec::with_capacity(len),
            },
        })
    }
}

/// 序列化序列（Vec、数组等）的助手结构
struct NbtSeqSerializer {
    elements: Vec<Tag>,
}

impl ser::SerializeSeq for NbtSeqSerializer {
    type Ok = Tag;
    type Error = NbtError;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        self.elements.push(value.serialize(NbtSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        if self.elements.is_empty() {
            // 空列表
            Ok(Tag::List(
                ListHeader {
                    tag_id: tag_id::END,
                    length: 0,
                },
                Arc::new([]),
            ))
        } else {
            // 检查同质性
            let first_id = self.elements[0].tag_id();
            for (_i, tag) in self.elements.iter().enumerate().skip(1) {
                if tag.tag_id() != first_id {
                    return Err(NbtError::ListTypeMismatch {
                        expected: first_id,
                        actual: tag.tag_id(),
                    });
                }
            }
            Ok(Tag::List(
                ListHeader {
                    tag_id: first_id,
                    length: self.elements.len() as i32,
                },
                Arc::from(self.elements),
            ))
        }
    }
}

impl ser::SerializeTuple for NbtSeqSerializer {
    type Ok = Tag;
    type Error = NbtError;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        ser::SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleStruct for NbtSeqSerializer {
    type Ok = Tag;
    type Error = NbtError;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        ser::SerializeSeq::end(self)
    }
}

/// 序列化映射/结构体的助手结构
struct NbtMapSerializer {
    map: HashMap<String, Tag>,
    next_key: Option<String>,
}

impl ser::SerializeMap for NbtMapSerializer {
    type Ok = Tag;
    type Error = NbtError;

    fn serialize_key<T: ?Sized>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        // 将 key 序列化为字符串
        let key_tag = key.serialize(NbtSerializer)?;
        self.next_key = match key_tag {
            Tag::String(s) => Some(s.to_string()),
            _ => {
                return Err(NbtError::Serialize(
                    "Map 键必须是字符串类型".to_string(),
                ))
            }
        };
        Ok(())
    }

    fn serialize_value<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        let key = self
            .next_key
            .take()
            .ok_or_else(|| NbtError::Serialize("缺少 Map 键".to_string()))?;
        self.map.insert(key, value.serialize(NbtSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Tag::Compound(Arc::new(self.map)))
    }
}

impl ser::SerializeStruct for NbtMapSerializer {
    type Ok = Tag;
    type Error = NbtError;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        self.map
            .insert(key.to_string(), value.serialize(NbtSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Tag::Compound(Arc::new(self.map)))
    }
}

/// 序列化枚举变体的助手结构
struct NbtVariantSerializer {
    variant: String,
    seq: NbtSeqSerializer,
}

impl ser::SerializeTupleVariant for NbtVariantSerializer {
    type Ok = Tag;
    type Error = NbtError;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        self.seq.serialize_element(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        // 创建一个包含变体名和序列值的 Compound
        let value = self.seq.end()?;
        let mut map = HashMap::new();
        map.insert(self.variant, value);
        Ok(Tag::Compound(Arc::new(map)))
    }
}

impl ser::SerializeStructVariant for NbtVariantSerializer {
    type Ok = Tag;
    type Error = NbtError;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        // 将结构体字段序列化为 Compound
        let mut map = HashMap::new();
        map.insert(key.to_string(), value.serialize(NbtSerializer)?);
        self.seq.elements.push(Tag::Compound(Arc::new(map)));
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        // 将所有字段合并到一个 Compound 中
        let mut result_map = HashMap::new();
        for tag in self.seq.elements {
            if let Tag::Compound(map) = tag {
                for (k, v) in map.iter() {
                    result_map.insert(k.clone(), v.clone());
                }
            }
        }
        // 用变体名包裹
        let mut variant_map = HashMap::new();
        variant_map.insert(self.variant, Tag::Compound(Arc::new(result_map)));
        Ok(Tag::Compound(Arc::new(variant_map)))
    }
}

/// NBT 反序列化器
struct NbtDeserializer<'de> {
    tag: &'de Tag,
}

impl<'de> NbtDeserializer<'de> {
    fn new(tag: &'de Tag) -> Self {
        NbtDeserializer { tag }
    }
}

impl<'de> Deserializer<'de> for NbtDeserializer<'de> {
    type Error = NbtError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.tag {
            Tag::Byte(v) => visitor.visit_i8(*v),
            Tag::Short(v) => visitor.visit_i16(*v),
            Tag::Int(v) => visitor.visit_i32(*v),
            Tag::Long(v) => visitor.visit_i64(*v),
            Tag::Float(v) => visitor.visit_f32(*v),
            Tag::Double(v) => visitor.visit_f64(*v),
            Tag::String(v) => visitor.visit_borrowed_str(&*v),  // 修复：使用 &*v 而不是 v.as_str()
            Tag::ByteArray(v) => {
                let u8_slice: Vec<u8> = v.iter().map(|&b| b as u8).collect();
                visitor.visit_bytes(&u8_slice)
            }
            Tag::IntArray(v) => {
                visitor.visit_seq(IntArrayAccess { iter: v.iter(), len: v.len() })
            }
            Tag::LongArray(v) => {
                visitor.visit_seq(LongArrayAccess { iter: v.iter(), len: v.len() })
            }
            Tag::List(_, v) => {
                visitor.visit_seq(ListAccess { iter: v.iter() })
            }
            Tag::Compound(map) => {
                visitor.visit_map(CompoundAccess { iter: map.iter(), value: None })
            }
            Tag::End => visitor.visit_none(),
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.tag {
            Tag::Byte(v) => visitor.visit_bool(*v != 0),
            Tag::String(v) => {
                if v.to_lowercase() == "true" {
                    visitor.visit_bool(true)
                } else if v.to_lowercase() == "false" {
                    visitor.visit_bool(false)
                } else {
                    Err(NbtError::Deserialize(format!("无法将字符串 '{}' 解析为布尔值", v)))
                }
            }
            _ => Err(NbtError::Deserialize(format!(
                "无法将 {:?} 反序列化为布尔值",
                self.tag
            ))),
        }
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.tag {
            Tag::Byte(v) => visitor.visit_i8(*v),
            Tag::Short(v) => {
                if *v >= i8::MIN as i16 && *v <= i8::MAX as i16 {
                    visitor.visit_i8(*v as i8)
                } else {
                    Err(NbtError::Deserialize(format!(
                        "值 {} 超出 i8 范围",
                        v
                    )))
                }
            }
            Tag::Int(v) => {
                if *v >= i8::MIN as i32 && *v <= i8::MAX as i32 {
                    visitor.visit_i8(*v as i8)
                } else {
                    Err(NbtError::Deserialize(format!(
                        "值 {} 超出 i8 范围",
                        v
                    )))
                }
            }
            Tag::Long(v) => {
                if *v >= i8::MIN as i64 && *v <= i8::MAX as i64 {
                    visitor.visit_i8(*v as i8)
                } else {
                    Err(NbtError::Deserialize(format!(
                        "值 {} 超出 i8 范围",
                        v
                    )))
                }
            }
            _ => Err(NbtError::Deserialize(format!(
                "无法将 {:?} 反序列化为 i8",
                self.tag
            ))),
        }
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.tag {
            Tag::Byte(v) => visitor.visit_i16(*v as i16),
            Tag::Short(v) => visitor.visit_i16(*v),
            Tag::Int(v) => {
                if *v >= i16::MIN as i32 && *v <= i16::MAX as i32 {
                    visitor.visit_i16(*v as i16)
                } else {
                    Err(NbtError::Deserialize(format!(
                        "值 {} 超出 i16 范围",
                        v
                    )))
                }
            }
            Tag::Long(v) => {
                if *v >= i16::MIN as i64 && *v <= i16::MAX as i64 {
                    visitor.visit_i16(*v as i16)
                } else {
                    Err(NbtError::Deserialize(format!(
                        "值 {} 超出 i16 范围",
                        v
                    )))
                }
            }
            _ => Err(NbtError::Deserialize(format!(
                "无法将 {:?} 反序列化为 i16",
                self.tag
            ))),
        }
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.tag {
            Tag::Byte(v) => visitor.visit_i32(*v as i32),
            Tag::Short(v) => visitor.visit_i32(*v as i32),
            Tag::Int(v) => visitor.visit_i32(*v),
            Tag::Long(v) => {
                if *v >= i32::MIN as i64 && *v <= i32::MAX as i64 {
                    visitor.visit_i32(*v as i32)
                } else {
                    Err(NbtError::Deserialize(format!(
                        "值 {} 超出 i32 范围",
                        v
                    )))
                }
            }
            _ => Err(NbtError::Deserialize(format!(
                "无法将 {:?} 反序列化为 i32",
                self.tag
            ))),
        }
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.tag {
            Tag::Byte(v) => visitor.visit_i64(*v as i64),
            Tag::Short(v) => visitor.visit_i64(*v as i64),
            Tag::Int(v) => visitor.visit_i64(*v as i64),
            Tag::Long(v) => visitor.visit_i64(*v),
            _ => Err(NbtError::Deserialize(format!(
                "无法将 {:?} 反序列化为 i64",
                self.tag
            ))),
        }
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.tag {
            Tag::Byte(v) => {
                if *v >= 0 {
                    visitor.visit_u8(*v as u8)
                } else {
                    Err(NbtError::Deserialize(format!(
                        "值 {} 超出 u8 范围",
                        v
                    )))
                }
            }
            Tag::Short(v) => {
                if *v >= 0 && *v <= u8::MAX as i16 {
                    visitor.visit_u8(*v as u8)
                } else {
                    Err(NbtError::Deserialize(format!(
                        "值 {} 超出 u8 范围",
                        v
                    )))
                }
            }
            _ => Err(NbtError::Deserialize(format!(
                "无法将 {:?} 反序列化为 u8",
                self.tag
            ))),
        }
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.tag {
            Tag::Byte(v) => {
                if *v >= 0 {
                    visitor.visit_u16(*v as u16)
                } else {
                    Err(NbtError::Deserialize(format!(
                        "值 {} 超出 u16 范围",
                        v
                    )))
                }
            }
            Tag::Short(v) => {
                if *v >= 0 {
                    visitor.visit_u16(*v as u16)
                } else {
                    Err(NbtError::Deserialize(format!(
                        "值 {} 超出 u16 范围",
                        v
                    )))
                }
            }
            Tag::Int(v) => {
                if *v >= 0 && *v <= u16::MAX as i32 {
                    visitor.visit_u16(*v as u16)
                } else {
                    Err(NbtError::Deserialize(format!(
                        "值 {} 超出 u16 范围",
                        v
                    )))
                }
            }
            _ => Err(NbtError::Deserialize(format!(
                "无法将 {:?} 反序列化为 u16",
                self.tag
            ))),
        }
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.tag {
            Tag::Int(v) => {
                if *v >= 0 {
                    visitor.visit_u32(*v as u32)
                } else {
                    Err(NbtError::Deserialize(format!(
                        "值 {} 超出 u32 范围",
                        v
                    )))
                }
            }
            Tag::Long(v) => {
                if *v >= 0 && *v <= u32::MAX as i64 {
                    visitor.visit_u32(*v as u32)
                } else {
                    Err(NbtError::Deserialize(format!(
                        "值 {} 超出 u32 范围",
                        v
                    )))
                }
            }
            Tag::Byte(v) => {
                if *v >= 0 {
                    visitor.visit_u32(*v as u32)
                } else {
                    Err(NbtError::Deserialize(format!(
                        "值 {} 超出 u32 范围",
                        v
                    )))
                }
            }
            Tag::Short(v) => {
                if *v >= 0 {
                    visitor.visit_u32(*v as u32)
                } else {
                    Err(NbtError::Deserialize(format!(
                        "值 {} 超出 u32 范围",
                        v
                    )))
                }
            }
            _ => Err(NbtError::Deserialize(format!(
                "无法将 {:?} 反序列化为 u32",
                self.tag
            ))),
        }
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.tag {
            Tag::Long(v) => {
                if *v >= 0 {
                    visitor.visit_u64(*v as u64)
                } else {
                    Err(NbtError::Deserialize(format!(
                        "值 {} 超出 u64 范围",
                        v
                    )))
                }
            }
            Tag::Int(v) => {
                if *v >= 0 {
                    visitor.visit_u64(*v as u64)
                } else {
                    Err(NbtError::Deserialize(format!(
                        "值 {} 超出 u64 范围",
                        v
                    )))
                }
            }
            Tag::Byte(v) => {
                if *v >= 0 {
                    visitor.visit_u64(*v as u64)
                } else {
                    Err(NbtError::Deserialize(format!(
                        "值 {} 超出 u64 范围",
                        v
                    )))
                }
            }
            Tag::Short(v) => {
                if *v >= 0 {
                    visitor.visit_u64(*v as u64)
                } else {
                    Err(NbtError::Deserialize(format!(
                        "值 {} 超出 u64 范围",
                        v
                    )))
                }
            }
            _ => Err(NbtError::Deserialize(format!(
                "无法将 {:?} 反序列化为 u64",
                self.tag
            ))),
        }
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.tag {
            Tag::Float(v) => visitor.visit_f32(*v),
            Tag::Double(v) => visitor.visit_f32(*v as f32),
            Tag::Int(v) => visitor.visit_f32(*v as f32),
            Tag::Long(v) => visitor.visit_f32(*v as f32),
            _ => Err(NbtError::Deserialize(format!(
                "无法将 {:?} 反序列化为 f32",
                self.tag
            ))),
        }
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.tag {
            Tag::Double(v) => visitor.visit_f64(*v),
            Tag::Float(v) => visitor.visit_f64(*v as f64),
            Tag::Int(v) => visitor.visit_f64(*v as f64),
            Tag::Long(v) => visitor.visit_f64(*v as f64),
            _ => Err(NbtError::Deserialize(format!(
                "无法将 {:?} 反序列化为 f64",
                self.tag
            ))),
        }
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.tag {
            Tag::String(s) => {
                if s.chars().count() == 1 {
                    visitor.visit_char(s.chars().next().unwrap())
                } else {
                    Err(NbtError::Deserialize(format!(
                        "字符串 '{}' 长度不为1，无法解析为字符",
                        s
                    )))
                }
            }
            _ => Err(NbtError::Deserialize(format!(
                "无法将 {:?} 反序列化为字符",
                self.tag
            ))),
        }
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.tag {
            Tag::String(s) => visitor.visit_borrowed_str(&*s),  // 修复：使用 &*s 而不是 s.as_str()
            _ => Err(NbtError::Deserialize(format!(
                "无法将 {:?} 反序列化为字符串",
                self.tag
            ))),
        }
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.tag {
            Tag::String(s) => visitor.visit_string(s.to_string()),
            _ => Err(NbtError::Deserialize(format!(
                "无法将 {:?} 反序列化为字符串",
                self.tag
            ))),
        }
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.tag {
            Tag::ByteArray(v) => {
                let u8_slice: Vec<u8> = v.iter().map(|&b| b as u8).collect();
                visitor.visit_bytes(&u8_slice)
            }
            Tag::String(s) => visitor.visit_bytes(s.as_bytes()),
            _ => Err(NbtError::Deserialize(format!(
                "无法将 {:?} 反序列化为字节数组",
                self.tag
            ))),
        }
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.tag {
            Tag::End => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.tag {
            Tag::List(_, v) => visitor.visit_seq(ListAccess { iter: v.iter() }),
            Tag::IntArray(v) => visitor.visit_seq(IntArrayAccess { iter: v.iter(), len: v.len() }),
            Tag::LongArray(v) => visitor.visit_seq(LongArrayAccess { iter: v.iter(), len: v.len() }),
            Tag::ByteArray(v) => {
                visitor.visit_seq(ByteArrayAccess { iter: v.iter(), len: v.len() })
            }
            _ => Err(NbtError::Deserialize(format!(
                "无法将 {:?} 反序列化为序列",
                self.tag
            ))),
        }
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.tag {
            Tag::List(_, v) if v.len() == len => {
                visitor.visit_seq(ListAccess { iter: v.iter() })
            }
            Tag::IntArray(v) if v.len() == len => {
                visitor.visit_seq(IntArrayAccess { iter: v.iter(), len: v.len() })
            }
            Tag::LongArray(v) if v.len() == len => {
                visitor.visit_seq(LongArrayAccess { iter: v.iter(), len: v.len() })
            }
            _ => Err(NbtError::Deserialize(format!(
                "无法将 {:?} 反序列化为长度为 {} 的元组",
                self.tag, len
            ))),
        }
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_tuple(len, visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.tag {
            Tag::Compound(map) => {
                visitor.visit_map(CompoundAccess { iter: map.iter(), value: None })
            }
            _ => Err(NbtError::Deserialize(format!(
                "无法将 {:?} 反序列化为映射",
                self.tag
            ))),
        }
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.tag {
            Tag::String(variant) => visitor.visit_enum((&**variant).into_deserializer()),  // 修复：使用 as_str() 是稳定的
            Tag::Compound(map) if map.len() == 1 => {
                let (variant, value) = map.iter().next().unwrap();
                visitor.visit_enum(EnumDeserializer {
                    variant: variant.as_str(),
                    value: Some(value),
                })
            }
            _ => Err(NbtError::Deserialize(format!(
                "无法将 {:?} 反序列化为枚举，期望字符串或单个键的Compound",
                self.tag
            ))),  // 修复：移除不支持的枚举反序列化
        }
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_string(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }
}

/// 用于反序列化 List 的访问器
struct ListAccess<'a> {
    iter: std::slice::Iter<'a, Tag>,
}

impl<'de> SeqAccess<'de> for ListAccess<'de> {
    type Error = NbtError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some(tag) => seed.deserialize(NbtDeserializer { tag }).map(Some),
            None => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }
}

/// 用于反序列化 IntArray 的访问器
struct IntArrayAccess<'a> {
    iter: std::slice::Iter<'a, i32>,
    len: usize,
}

impl<'de> SeqAccess<'de> for IntArrayAccess<'de> {
    type Error = NbtError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some(&v) => seed.deserialize(de::value::I32Deserializer::new(v)).map(Some),
            None => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.len)
    }
}

/// 用于反序列化 LongArray 的访问器
struct LongArrayAccess<'a> {
    iter: std::slice::Iter<'a, i64>,
    len: usize,
}

impl<'de> SeqAccess<'de> for LongArrayAccess<'de> {
    type Error = NbtError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some(&v) => seed.deserialize(de::value::I64Deserializer::new(v)).map(Some),
            None => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.len)
    }
}

/// 用于反序列化 ByteArray 的访问器
struct ByteArrayAccess<'a> {
    iter: std::slice::Iter<'a, i8>,
    len: usize,
}

impl<'de> SeqAccess<'de> for ByteArrayAccess<'de> {
    type Error = NbtError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some(&v) => seed.deserialize(de::value::I8Deserializer::new(v)).map(Some),
            None => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.len)
    }
}

/// 用于反序列化 Compound 的访问器
struct CompoundAccess<'a> {
    iter: std::collections::hash_map::Iter<'a, String, Tag>,
    value: Option<&'a Tag>,
}

impl<'de> MapAccess<'de> for CompoundAccess<'de> {
    type Error = NbtError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some((key, value)) => {
                self.value = Some(value);
                seed.deserialize(de::value::StrDeserializer::<NbtError>::new(key.as_str()))  // 修复：添加类型注解
                    .map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        match self.value.take() {
            Some(value) => seed.deserialize(NbtDeserializer { tag: value }),
            None => Err(NbtError::Deserialize("内部错误：没有对应的值".to_string())),
        }
    }
}

/// 用于反序列化枚举的助手
struct EnumDeserializer<'a> {
    variant: &'a str,
    value: Option<&'a Tag>,
}

impl<'de> de::EnumAccess<'de> for EnumDeserializer<'de> {
    type Error = NbtError;
    type Variant = EnumVariantDeserializer<'de>;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        let variant = seed.deserialize(de::value::StrDeserializer::<NbtError>::new(self.variant))?;  // 修复：添加类型注解
        let deserializer = EnumVariantDeserializer { value: self.value };
        Ok((variant, deserializer))
    }
}

/// 用于反序列化枚举变体的助手
struct EnumVariantDeserializer<'a> {
    value: Option<&'a Tag>,
}

impl<'de> de::VariantAccess<'de> for EnumVariantDeserializer<'de> {
    type Error = NbtError;

    fn unit_variant(self) -> Result<(), Self::Error> {
        match self.value {
            None | Some(Tag::End) => Ok(()),
            _ => Err(NbtError::Deserialize(
                "期望空枚举变体，但找到了值".to_string(),
            )),
        }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.value {
            Some(value) => seed.deserialize(NbtDeserializer { tag: value }),
            None => Err(NbtError::Deserialize(
                "期望新类型变体，但缺少值".to_string(),
            )),
        }
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        match self.value {
            Some(Tag::List(_, v)) if v.len() == len => {
                visitor.visit_seq(ListAccess { iter: v.iter() })
            }
            Some(value) => Err(NbtError::Deserialize(format!(
                "期望长度为 {} 的元组，但找到了 {:?}",
                len, value
            ))),
            None => Err(NbtError::Deserialize("期望元组变体，但缺少值".to_string())),
        }
    }

    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        match self.value {
            Some(Tag::Compound(map)) => {
                visitor.visit_map(CompoundAccess { iter: map.iter(), value: None })
            }
            Some(value) => Err(NbtError::Deserialize(format!(
                "期望结构体变体，但找到了 {:?}",
                value
            ))),
            None => Err(NbtError::Deserialize("期望结构体变体，但缺少值".to_string())),
        }
    }
}

// 便利函数
impl Tag {
    /// 从实现了 Serialize 的类型创建 Tag
    pub fn from_serializable<T: Serialize>(value: &T) -> Result<Self, NbtError> {
        nbt_serde::to_tag(value)
    }
    
    /// 将 Tag 转换为实现了 Deserialize 的类型
    pub fn to_deserializable<'a, T: Deserialize<'a>>(&'a self) -> Result<T, NbtError> {
        nbt_serde::from_tag(self)
    }
}
