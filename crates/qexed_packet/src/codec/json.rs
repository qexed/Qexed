use crate::{PacketCodec, write_varint_len};
use bytes::{Buf as _, BufMut as _};

use crate::net_types::VarInt;

impl PacketCodec for serde_json::Value {
    fn serialize(&self, w: &mut crate::PacketWriter) -> anyhow::Result<()> {
        let bytes = serde_json::to_vec(self)?;
        write_varint_len(bytes.len(), "json", w)?;
        w.buf.put_slice(&bytes);
        Ok(())
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> anyhow::Result<()> {
        let mut len = VarInt::default();
        len.deserialize(r)?;
        if len.0 < 0 {
            return Err(anyhow::anyhow!("negative json length: {}", len.0));
        }

        let len = len.0 as usize;
        if r.buf.remaining() < len {
            return Err(anyhow::anyhow!(
                "json length {} exceeds remaining {}",
                len,
                r.buf.remaining()
            ));
        }

        let bytes = r.buf.copy_to_bytes(len);
        *self = serde_json::from_slice(&bytes)?;
        Ok(())
    }
}
