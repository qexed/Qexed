use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x13)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ContainerClose {
    pub window_id: u8,
}

#[cfg(test)]
mod tests {
    use qexed_packet::Packet;

    use super::ContainerClose;

    #[test]
    fn container_close_round_trips_window_id() {
        let packet = ContainerClose { window_id: 1 };
        let mut buf = bytes::BytesMut::new();
        let mut writer = qexed_packet::PacketWriter::new(&mut buf);
        packet.serialize(&mut writer).unwrap();

        let mut bytes = buf.freeze();
        let mut reader = qexed_packet::PacketReader::new(&mut bytes);
        let mut decoded = ContainerClose::default();
        decoded.deserialize(&mut reader).unwrap();

        assert_eq!(decoded, packet);
    }
}
