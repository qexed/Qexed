use crate::{PacketCodec, PacketReader, PacketWriter, net_types::Bitset};

impl PacketCodec for Bitset {
    fn serialize(&self, w: &mut PacketWriter) -> crate::Result<()> {
        w.serialize(&self.0)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> crate::Result<()> {
        self.0.deserialize(r)
    }
}
