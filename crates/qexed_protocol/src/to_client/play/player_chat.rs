use qexed_packet::{Packet, PacketCodec, PacketReader, PacketWriter, net_types::VarInt};
use qexed_packet::error::PacketError;

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
            chat_type: ChatTypeHolder::default(),
            network_name,
            network_target_name: None,
        }
    }
}

impl Packet for PlayerChat {
    const ID: i32 = 0x41;

    fn serialize(&self, w: &mut PacketWriter) -> Result<(), PacketError> {
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
            VarInt(0).serialize(w)?;
        }
        self.chat_type.serialize(w)?;
        self.network_name.serialize(w)?;
        self.network_target_name.serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> Result<(), PacketError> {
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
            let mut ignored_mask = Vec::<i64>::default();
            ignored_mask.deserialize(r)?;
        }
        self.chat_type.deserialize(r)?;
        self.network_name.deserialize(r)?;
        self.network_target_name.deserialize(r)
    }
}

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
    fn serialize(&self, w: &mut PacketWriter) -> Result<(), PacketError> {
        self.id.serialize(w)?;
        if self.id.0 == 0 {
            let Some(signature) = &self.full_signature else {
                return Err(PacketError::MissingRequired {
                    what: "signature bytes (required when packed message id is 0)",
                });
            };
            signature.serialize(w)?;
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> Result<(), PacketError> {
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
    fn serialize(&self, w: &mut PacketWriter) -> Result<(), PacketError> {
        self.holder_id.serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> Result<(), PacketError> {
        self.holder_id.deserialize(r)
    }
}

const FILTER_MASK_PASS_THROUGH: i32 = 0;
const FILTER_MASK_PARTIALLY_FILTERED: i32 = 2;
