use std::collections::HashMap;

use bytes::{Buf as _, BufMut as _};

use crate::{
    PacketCodec, PacketReader, PacketWriter,
    net_types::{RestBuffer, VarInt},
};

#[derive(Debug, Default, PartialEq, Clone)]
pub struct ByteArray(pub Vec<u8>);

impl PacketCodec for ByteArray {
    fn serialize(&self, w: &mut PacketWriter) -> anyhow::Result<()> {
        VarInt(self.0.len() as i32).serialize(w)?;
        w.buf.put_slice(&self.0);
        Ok(())
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> anyhow::Result<()> {
        let mut len = VarInt::default();
        len.deserialize(r)?;
        let len = checked_len(len.0, r.buf.remaining(), "byte array")?;
        self.0 = r.buf.copy_to_bytes(len).to_vec();
        Ok(())
    }
}

impl From<Vec<u8>> for ByteArray {
    fn from(value: Vec<u8>) -> Self {
        Self(value)
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct JsonValue(pub serde_json::Value);

impl PacketCodec for JsonValue {
    fn serialize(&self, w: &mut PacketWriter) -> anyhow::Result<()> {
        self.0.serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> anyhow::Result<()> {
        self.0.deserialize(r)
    }
}

pub type AnyNbt = qexed_nbt::Tag;

#[derive(Debug, Default, PartialEq, Clone)]
pub struct GameProfile {
    pub uuid: uuid::Uuid,
    pub username: String,
    pub properties: Vec<ProfileProperty>,
}

impl PacketCodec for GameProfile {
    fn serialize(&self, w: &mut PacketWriter) -> anyhow::Result<()> {
        self.uuid.serialize(w)?;
        self.username.serialize(w)?;
        write_bounded_count(self.properties.len(), 16, w)?;
        for property in &self.properties {
            property.serialize(w)?;
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> anyhow::Result<()> {
        self.uuid.deserialize(r)?;
        self.username.deserialize(r)?;
        let count = read_bounded_count(16, r)?;
        self.properties.clear();
        self.properties.reserve(count);
        for _ in 0..count {
            let mut property = ProfileProperty::default();
            property.deserialize(r)?;
            self.properties.push(property);
        }
        Ok(())
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct ProfileProperty {
    pub name: String,
    pub value: String,
    pub signature: Option<String>,
}

impl PacketCodec for ProfileProperty {
    fn serialize(&self, w: &mut PacketWriter) -> anyhow::Result<()> {
        self.name.serialize(w)?;
        self.value.serialize(w)?;
        self.signature.serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> anyhow::Result<()> {
        self.name.deserialize(r)?;
        self.value.deserialize(r)?;
        self.signature.deserialize(r)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Either<L, R> {
    Left(L),
    Right(R),
}

impl<L, R> Default for Either<L, R>
where
    L: Default,
{
    fn default() -> Self {
        Self::Left(L::default())
    }
}

impl<L, R> PacketCodec for Either<L, R>
where
    L: PacketCodec,
    R: PacketCodec,
{
    fn serialize(&self, w: &mut PacketWriter) -> anyhow::Result<()> {
        match self {
            Self::Left(value) => {
                true.serialize(w)?;
                value.serialize(w)
            }
            Self::Right(value) => {
                false.serialize(w)?;
                value.serialize(w)
            }
        }
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> anyhow::Result<()> {
        let mut is_left = false;
        is_left.deserialize(r)?;
        if is_left {
            let mut value = L::default();
            value.deserialize(r)?;
            *self = Self::Left(value);
        } else {
            let mut value = R::default();
            value.deserialize(r)?;
            *self = Self::Right(value);
        }
        Ok(())
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct BoundedStringMap<const MAX_COUNT: usize>(pub HashMap<String, String>);

pub type StringMap = BoundedStringMap<32>;

impl<const MAX_COUNT: usize> PacketCodec for BoundedStringMap<MAX_COUNT> {
    fn serialize(&self, w: &mut PacketWriter) -> anyhow::Result<()> {
        write_bounded_count(self.0.len(), MAX_COUNT, w)?;
        for (key, value) in &self.0 {
            key.serialize(w)?;
            value.serialize(w)?;
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> anyhow::Result<()> {
        let count = read_bounded_count(MAX_COUNT, r)?;
        self.0.clear();
        for _ in 0..count {
            let mut key = String::new();
            let mut value = String::new();
            key.deserialize(r)?;
            value.deserialize(r)?;
            self.0.insert(key, value);
        }
        Ok(())
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct ServerLink {
    pub link_type: Either<VarInt, AnyNbt>,
    pub link: String,
}

impl PacketCodec for ServerLink {
    fn serialize(&self, w: &mut PacketWriter) -> anyhow::Result<()> {
        self.link_type.serialize(w)?;
        self.link.serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> anyhow::Result<()> {
        self.link_type.deserialize(r)?;
        self.link.deserialize(r)
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct OptionalNbt(pub Option<qexed_nbt::Tag>);

impl PacketCodec for OptionalNbt {
    fn serialize(&self, w: &mut PacketWriter) -> anyhow::Result<()> {
        match &self.0 {
            Some(tag) => tag.serialize(w),
            None => qexed_nbt::Tag::End.serialize(w),
        }
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> anyhow::Result<()> {
        let mut tag = qexed_nbt::Tag::default();
        tag.deserialize(r)?;
        self.0 = (tag != qexed_nbt::Tag::End).then_some(tag);
        Ok(())
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct LengthPrefixed<T, const MAX_SIZE: usize> {
    pub value: T,
}

impl<T, const MAX_SIZE: usize> PacketCodec for LengthPrefixed<T, MAX_SIZE>
where
    T: PacketCodec,
{
    fn serialize(&self, w: &mut PacketWriter) -> anyhow::Result<()> {
        let mut payload = bytes::BytesMut::new();
        {
            let mut payload_writer = PacketWriter::new(&mut payload);
            self.value.serialize(&mut payload_writer)?;
        }

        if payload.len() > MAX_SIZE {
            return Err(anyhow::anyhow!(
                "length-prefixed payload size {} exceeds max {}",
                payload.len(),
                MAX_SIZE
            ));
        }

        VarInt(payload.len() as i32).serialize(w)?;
        w.buf.put_slice(&payload);
        Ok(())
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> anyhow::Result<()> {
        let mut len = VarInt::default();
        len.deserialize(r)?;
        let len = checked_len(len.0, r.buf.remaining(), "length-prefixed payload")?;
        if len > MAX_SIZE {
            return Err(anyhow::anyhow!(
                "length-prefixed payload size {} exceeds max {}",
                len,
                MAX_SIZE
            ));
        }

        let mut payload = r.buf.copy_to_bytes(len);
        let mut payload_reader = PacketReader::new(&mut payload);
        self.value.deserialize(&mut payload_reader)?;
        if payload_reader.buf.has_remaining() {
            return Err(anyhow::anyhow!(
                "length-prefixed payload has {} trailing bytes",
                payload_reader.buf.remaining()
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct CustomQueryPayload {
    pub channel: String,
    pub data: RestBuffer,
}

impl PacketCodec for CustomQueryPayload {
    fn serialize(&self, w: &mut PacketWriter) -> anyhow::Result<()> {
        self.channel.serialize(w)?;
        self.data.serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> anyhow::Result<()> {
        self.channel.deserialize(r)?;
        self.data.deserialize(r)
    }
}

fn read_bounded_count(max: usize, r: &mut PacketReader) -> anyhow::Result<usize> {
    let mut count = VarInt::default();
    count.deserialize(r)?;
    if count.0 < 0 {
        return Err(anyhow::anyhow!("negative collection length: {}", count.0));
    }

    let count = count.0 as usize;
    if count > max {
        return Err(anyhow::anyhow!(
            "collection length {} exceeds max {}",
            count,
            max
        ));
    }

    Ok(count)
}

fn write_bounded_count(count: usize, max: usize, w: &mut PacketWriter) -> anyhow::Result<()> {
    if count > max {
        return Err(anyhow::anyhow!(
            "collection length {} exceeds max {}",
            count,
            max
        ));
    }
    if count > i32::MAX as usize {
        return Err(anyhow::anyhow!(
            "collection length {} exceeds VarInt max",
            count
        ));
    }

    VarInt(count as i32).serialize(w)
}

fn checked_len(value: i32, remaining: usize, name: &str) -> anyhow::Result<usize> {
    if value < 0 {
        return Err(anyhow::anyhow!("negative {} length: {}", name, value));
    }

    let len = value as usize;
    if len > remaining {
        return Err(anyhow::anyhow!(
            "{} length {} exceeds remaining {}",
            name,
            len,
            remaining
        ));
    }

    Ok(len)
}
