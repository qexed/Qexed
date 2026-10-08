use bytes::{Buf as _, BufMut as _};

use crate::{PacketCodec, error::PacketError, net_types::VarInt};

impl PacketCodec for String {
    fn serialize(&self, w: &mut crate::PacketWriter) -> Result<(), PacketError> {
        let len = self.len();
        if len > i32::MAX as usize {
            return Err(PacketError::LengthExceedsVarIntMax { name: "string", len });
        }

        VarInt(len as i32).serialize(w)?;
        w.buf.put_slice(self.as_bytes());
        Ok(())
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> Result<(), PacketError> {
        let mut len = VarInt::default();
        len.deserialize(r)?;
        if len.0 < 0 {
            return Err(PacketError::NegativeLength {
                name: "string",
                value: len.0,
            });
        }

        let len = len.0 as usize;
        if r.buf.remaining() < len {
            return Err(PacketError::LengthExceedsRemaining {
                name: "string",
                len,
                remaining: r.buf.remaining(),
            });
        }

        let bytes = r.buf.copy_to_bytes(len);
        *self = String::from_utf8(bytes.to_vec())?;
        Ok(())
    }
}