use qexed_packet::PacketCodec;

#[qexed_packet_macros::packet(id = 0x21)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct EntityEvent {
    pub entity_id: i32,
    pub event_id: u8,
}

#[cfg(test)]
mod tests {
    use qexed_packet::Packet;

    use super::EntityEvent;

    #[test]
    fn entity_event_uses_fixed_i32_entity_id() {
        let packet = EntityEvent {
            entity_id: 300,
            event_id: 3,
        };
        let mut buf = bytes::BytesMut::new();
        let mut writer = qexed_packet::PacketWriter::new(&mut buf);
        packet.serialize(&mut writer).unwrap();

        assert_eq!(buf.len(), 5);
        assert_eq!(&buf[..4], &300_i32.to_be_bytes());
        assert_eq!(buf[4], 3);

        let mut bytes = buf.freeze();
        let mut reader = qexed_packet::PacketReader::new(&mut bytes);
        let mut decoded = EntityEvent::default();
        decoded.deserialize(&mut reader).unwrap();
        assert_eq!(decoded, packet);
    }
}
