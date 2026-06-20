use bytes::{Buf as _, BufMut as _};

use crate::{PacketCodec, net_types::VarInt, read_exact_vec, write_varint_len};

impl PacketCodec for String {
    fn serialize(&self, w: &mut crate::PacketWriter) -> anyhow::Result<()> {
        write_varint_len(self.len(), "string", w)?;
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

        *self = String::from_utf8(read_exact_vec(r.buf, len))?;
        Ok(())
    }
}
