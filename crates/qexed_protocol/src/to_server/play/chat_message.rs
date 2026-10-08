use qexed_packet::{Packet, PacketCodec, PacketReader, PacketWriter, net_types::VarInt};
use qexed_packet::error::PacketError;

#[derive(Debug, Default, PartialEq, Clone)]
pub struct ChatMessage {
    pub message: String,
    pub timestamp: i64,
    pub salt: i64,
    pub signature: Option<crate::types::MessageSignature>,
    pub offset: VarInt,
    pub acknowledged: [u8; 3],
    pub checksum: u8,
}

impl Packet for ChatMessage {
    const ID: i32 = 0x09;

    fn serialize(&self, w: &mut PacketWriter) -> Result<(), PacketError> {
        self.message.serialize(w)?;
        self.timestamp.serialize(w)?;
        self.salt.serialize(w)?;
        self.signature.is_some().serialize(w)?;
        if let Some(signature) = &self.signature {
            signature.serialize(w)?;
        }
        self.offset.serialize(w)?;
        self.acknowledged.serialize(w)?;
        self.checksum.serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> Result<(), PacketError> {
        self.message.deserialize(r)?;
        self.timestamp.deserialize(r)?;
        self.salt.deserialize(r)?;

        let mut has_signature = false;
        has_signature.deserialize(r)?;
        self.signature = if has_signature {
            let mut signature = crate::types::MessageSignature::default();
            signature.deserialize(r)?;
            Some(signature)
        } else {
            None
        };

        self.offset.deserialize(r)?;
        self.acknowledged.deserialize(r)?;
        self.checksum.deserialize(r)
    }
}
