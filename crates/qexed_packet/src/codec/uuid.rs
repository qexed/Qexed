use bytes::BufMut;

use crate::{PacketCodec, PacketReader, PacketWriter, error::PacketError};

impl PacketCodec for uuid::Uuid {
    fn serialize(&self, w: &mut PacketWriter) -> Result<(), PacketError> {
        Ok(w.buf.put_slice(self.as_bytes()))
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> Result<(), PacketError> {
        // 创建 16 字节数组
        let mut bytes = [0u8; 16];
        r.buf.copy_to_slice(&mut bytes);
        Ok(*self = uuid::Uuid::from_bytes(bytes))
    }
}
