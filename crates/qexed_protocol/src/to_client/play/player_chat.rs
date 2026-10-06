use qexed_packet::{
    Packet, PacketCodec, PacketReader, PacketWriter,
    net_types::{ByteArray, VarInt},
};

/// FilterMask$Type id（idMapper -> VarInt）。
const FILTER_MASK_PASS_THROUGH: i32 = 0;
const FILTER_MASK_FULLY_FILTERED: i32 = 1;
const FILTER_MASK_PARTIALLY_FILTERED: i32 = 2;

/// ClientboundPlayerChatPacket（Function8 composite）线序：
///   globalIndex VarInt, sender UUID, index VarInt,
///   Optional<MessageSignature>,
///   SignedMessageBody$Packed { content String, timeStamp INSTANT(i64 毫秒), salt LONG(i64), lastSeen: list(<=20) of MessageSignature$Packed },
///   Optional<Component> unsignedContent,
///   FilterMask { type VarInt; type==2 时追加 BIT_SET(ByteArray) },
///   ChatType$Bound { Holder VarInt, Component name, Optional<Component> targetName }
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PlayerChat {
    pub global_index: VarInt,
    pub sender: uuid::Uuid,
    pub index: VarInt,
    pub signature: Option<crate::types::MessageSignature>,
    pub plain_message: String,
    pub timestamp: i64,
    pub salt: i64,
    pub previous_messages: Vec<PackedMessageSignature>,
    pub unsigned_chat_content: Option<crate::types::TextComponent>,
    pub filter_type: VarInt,
    /// 仅 filter_type == 2 时读写（BIT_SET = ByteArray：VarInt 长度 + 小端位字节）。
    pub filter_mask: ByteArray,
    pub chat_type: ChatTypeHolder,
    pub network_name: crate::types::TextComponent,
    pub network_target_name: Option<crate::types::TextComponent>,
}

impl PlayerChat {
    pub fn pass_through(
        global_index: i32,
        sender: uuid::Uuid,
        index: i32,
        signature: crate::types::MessageSignature,
        previous_messages: Vec<PackedMessageSignature>,
        plain_message: impl Into<String>,
        timestamp: i64,
        salt: i64,
        network_name: crate::types::TextComponent,
    ) -> Self {
        Self {
            global_index: VarInt(global_index),
            sender,
            index: VarInt(index),
            signature: Some(signature),
            plain_message: plain_message.into(),
            timestamp,
            salt,
            previous_messages,
            unsigned_chat_content: None,
            filter_type: VarInt(FILTER_MASK_PASS_THROUGH),
            filter_mask: ByteArray::default(),
            chat_type: ChatTypeHolder::default(),
            network_name,
            network_target_name: None,
        }
    }
}

impl Packet for PlayerChat {
    const ID: i32 = 0x42;

    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        self.global_index.serialize(w)?;
        self.sender.serialize(w)?;
        self.index.serialize(w)?;
        self.signature.serialize(w)?;
        self.plain_message.serialize(w)?;
        self.timestamp.serialize(w)?;
        self.salt.serialize(w)?;
        self.previous_messages.serialize(w)?;
        self.unsigned_chat_content.serialize(w)?;
        self.filter_type.serialize(w)?;
        if self.filter_type.0 == FILTER_MASK_PARTIALLY_FILTERED {
            self.filter_mask.serialize(w)?;
        }
        // ChatType$Bound：Holder VarInt + name + Optional targetName
        self.chat_type.serialize(w)?;
        self.network_name.serialize(w)?;
        self.network_target_name.serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        self.global_index.deserialize(r)?;
        self.sender.deserialize(r)?;
        self.index.deserialize(r)?;
        self.signature.deserialize(r)?;
        self.plain_message.deserialize(r)?;
        self.timestamp.deserialize(r)?;
        self.salt.deserialize(r)?;
        self.previous_messages.deserialize(r)?;
        self.unsigned_chat_content.deserialize(r)?;
        self.filter_type.deserialize(r)?;
        if self.filter_type.0 == FILTER_MASK_PARTIALLY_FILTERED {
            self.filter_mask.deserialize(r)?;
        } else {
            self.filter_mask = ByteArray::default();
        }
        self.chat_type.deserialize(r)?;
        self.network_name.deserialize(r)?;
        self.network_target_name.deserialize(r)
    }
}

/// MessageSignature$Packed：VarInt(id+1)；0 表示携带完整 256 字节签名。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PackedMessageSignature {
    pub id: VarInt,
    pub full_signature: Option<crate::types::MessageSignature>,
}

impl PackedMessageSignature {
    pub fn full(signature: crate::types::MessageSignature) -> Self {
        Self {
            id: VarInt(0),
            full_signature: Some(signature),
        }
    }
}

impl PacketCodec for PackedMessageSignature {
    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        self.id.serialize(w)?;
        if self.id.0 == 0 {
            let Some(signature) = &self.full_signature else {
                return Err(qexed_packet::PacketError::msg(format!("full packed message signature requires signature bytes")));
            };
            signature.serialize(w)?;
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        self.id.deserialize(r)?;
        self.full_signature = if self.id.0 == 0 {
            let mut signature = crate::types::MessageSignature::default();
            signature.deserialize(r)?;
            Some(signature)
        } else {
            None
        };
        Ok(())
    }
}

/// ChatType$Bound 的 Holder 部分（name/targetName 由包级字段承载，线序一致）。
#[derive(Debug, PartialEq, Clone)]
pub struct ChatTypeHolder {
    pub holder_id: VarInt,
}

impl Default for ChatTypeHolder {
    fn default() -> Self {
        Self {
            holder_id: VarInt(1),
        }
    }
}

impl PacketCodec for ChatTypeHolder {
    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        self.holder_id.serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        self.holder_id.deserialize(r)
    }
}
