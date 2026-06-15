use bytes::{Buf as _, BufMut as _};

use crate::{PacketCodec, net_types::VarInt};

impl PacketCodec for String {
    fn serialize(&self, w: &mut crate::PacketWriter) -> anyhow::Result<()> {
        let len = self.len();
        if len > i32::MAX as usize {
            return Err(anyhow::anyhow!("string length {} exceeds VarInt max", len));
        }

        VarInt(len as i32).serialize(w)?;
        w.buf.put_slice(self.as_bytes());
        Ok(())
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> anyhow::Result<()> {
        let mut len = VarInt::default();
        len.deserialize(r)?;
        if len.0 < 0 {
            return Err(anyhow::anyhow!("negative string length: {}", len.0));
        }

        let len = len.0 as usize;
        if r.buf.remaining() < len {
            return Err(anyhow::anyhow!(
                "string length {} exceeds remaining {}",
                len,
                r.buf.remaining()
            ));
        }

        let bytes = r.buf.copy_to_bytes(len);
        *self = String::from_utf8(bytes.to_vec())?;
        Ok(())
    }
}
