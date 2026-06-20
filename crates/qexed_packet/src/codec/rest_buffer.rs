use bytes::BufMut;

use crate::{PacketCodec, PacketReader, PacketWriter, net_types::RestBuffer, read_exact_vec_into};

impl PacketCodec for RestBuffer {
    fn serialize(&self, w: &mut PacketWriter) -> anyhow::Result<()> {
        Ok(w.buf.put_slice(&self.0))
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> anyhow::Result<()> {
        let len = r.buf.remaining();
        read_exact_vec_into(r.buf, len, &mut self.0)?;
        Ok(())
    }
}
