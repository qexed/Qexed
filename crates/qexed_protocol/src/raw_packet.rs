use qexed_packet::{Packet, PacketCodec, PacketReader, PacketWriter, net_types::RestBuffer};

#[derive(Debug, Default, PartialEq, Clone)]
pub struct RawPacket<const ID_VALUE: i32> {
    pub data: RestBuffer,
}

impl<const ID_VALUE: i32> Packet for RawPacket<ID_VALUE> {
    const ID: i32 = ID_VALUE;

    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        self.data.serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        self.data.deserialize(r)
    }
}
