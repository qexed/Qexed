use qexed_packet::{PacketCodec, PacketReader, PacketWriter, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x1f)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct DeleteChat {
    pub message_signature: MessageSignaturePacked,
}

/// net.minecraft.network.chat.MessageSignature$Packed：
/// 先读 VarInt (id + 1)；为 0 表示完整签名（后跟 256 字节），否则是缓存 id。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MessageSignaturePacked {
    pub id: i32,
    pub full_signature: Option<Vec<u8>>,
}

impl PacketCodec for MessageSignaturePacked {
    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        match &self.full_signature {
            Some(signature) => {
                VarInt(0).serialize(w)?;
                if signature.len() != MessageSignaturePacked::SIGNATURE_BYTES {
                    return Err(qexed_packet::PacketError::msg(format!(
                        "message signature length must be {}, got {}",
                        MessageSignaturePacked::SIGNATURE_BYTES,
                        signature.len()
                    )));
                }
                w.buf.extend_from_slice(signature);
                Ok(())
            }
            None => VarInt(self.id + 1).serialize(w),
        }
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        let mut packed = VarInt::default();
        packed.deserialize(r)?;
        let id = packed.0 - 1;
        if id == -1 {
            if r.buf.remaining() < MessageSignaturePacked::SIGNATURE_BYTES {
                return Err(qexed_packet::PacketError::msg(format!(
                    "message signature length {} exceeds remaining {}",
                    MessageSignaturePacked::SIGNATURE_BYTES,
                    r.buf.remaining()
                )));
            }
            self.full_signature =
                Some(r.buf.copy_to_bytes(MessageSignaturePacked::SIGNATURE_BYTES).to_vec());
            self.id = -1;
        } else {
            self.id = id;
            self.full_signature = None;
        }
        Ok(())
    }
}

impl MessageSignaturePacked {
    pub const SIGNATURE_BYTES: usize = 256;
}
