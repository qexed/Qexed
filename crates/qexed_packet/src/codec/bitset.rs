use crate::{PacketCodec, PacketReader, PacketWriter, net_types::Bitset};

impl PacketCodec for Bitset {
    fn serialize(&self, w: &mut PacketWriter) -> Result<(), crate::error::PacketError> {
        w.serialize(&self.0)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> Result<(), crate::error::PacketError> {
        self.0.deserialize(r)
    }
}
