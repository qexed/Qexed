
use crate::{PacketCodec, PacketReader, PacketWriter, net_types::Bitset};

impl PacketCodec for Bitset {
    fn serialize(&self, w: &mut PacketWriter) -> Result<(), anyhow::Error> {
        w.serialize(&self.0)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> Result<(), anyhow::Error> {
        self.0.deserialize(r)
    }
}