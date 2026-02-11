use bytes::BufMut;

use crate::{PacketCodec, PacketReader, PacketWriter, net_types::RestBuffer};

impl PacketCodec for RestBuffer {
    fn serialize(&self, w: &mut PacketWriter) -> anyhow::Result<()>  {
        Ok(w.buf.put_slice(&self.0))
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> anyhow::Result<()>  {
        let len = r.buf.remaining();
        let bytes = r.buf.copy_to_bytes(len);
        Ok(*self = RestBuffer(bytes.to_vec()))
    }
}