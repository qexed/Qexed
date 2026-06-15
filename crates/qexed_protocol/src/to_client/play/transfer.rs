use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x81)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Transfer {
    pub host: String,
    pub port: VarInt,
}

impl Transfer {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port: VarInt(i32::from(port)),
        }
    }
}

#[cfg(test)]
mod tests {
    use qexed_packet::Packet;

    use super::Transfer;

    #[test]
    fn transfer_round_trips() {
        let packet = Transfer::new("127.0.0.1", 25566);
        let mut buf = bytes::BytesMut::new();
        let mut writer = qexed_packet::PacketWriter::new(&mut buf);
        packet.serialize(&mut writer).unwrap();

        let mut bytes = buf.freeze();
        let mut reader = qexed_packet::PacketReader::new(&mut bytes);
        let mut decoded = Transfer::default();
        decoded.deserialize(&mut reader).unwrap();

        assert_eq!(decoded, packet);
    }
}
