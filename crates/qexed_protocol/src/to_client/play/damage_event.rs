use qexed_packet::{Packet, PacketCodec, PacketReader, PacketWriter, net_types::VarInt};
use qexed_packet::error::PacketError;

#[derive(Debug, Default, PartialEq, Clone)]
pub struct DamageEvent {
    pub entity_id: VarInt,
    pub source_type_id: VarInt,
    pub source_cause_id: VarInt,
    pub source_direct_id: VarInt,
    pub has_source_position: bool,
    pub source_position: Option<DamageSourcePosition>,
}

impl Packet for DamageEvent {
    const ID: i32 = 0x19;

    fn serialize(&self, w: &mut PacketWriter) -> Result<(), PacketError> {
        self.entity_id.serialize(w)?;
        self.source_type_id.serialize(w)?;
        self.source_cause_id.serialize(w)?;
        self.source_direct_id.serialize(w)?;
        self.has_source_position.serialize(w)?;
        if self.has_source_position {
            let position = self.source_position.as_ref().ok_or(PacketError::MissingRequired {
                what: "source position (required when has_source_position is set)",
            })?;
            position.serialize(w)?;
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> Result<(), PacketError> {
        self.entity_id.deserialize(r)?;
        self.source_type_id.deserialize(r)?;
        self.source_cause_id.deserialize(r)?;
        self.source_direct_id.deserialize(r)?;
        self.has_source_position.deserialize(r)?;
        self.source_position = if self.has_source_position {
            let mut position = DamageSourcePosition::default();
            position.deserialize(r)?;
            Some(position)
        } else {
            None
        };
        Ok(())
    }
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct DamageSourcePosition {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[cfg(test)]
mod tests {
    use bytes::Buf as _;
    use qexed_packet::{Packet, PacketReader, PacketWriter, net_types::VarInt};

    use super::{DamageEvent, DamageSourcePosition};

    #[test]
    fn damage_event_without_source_position_has_no_extra_option_flag() {
        let packet = DamageEvent {
            entity_id: VarInt(1),
            source_type_id: VarInt(0),
            source_cause_id: VarInt(2),
            source_direct_id: VarInt(2),
            has_source_position: false,
            source_position: None,
        };
        let mut bytes = bytes::BytesMut::new();

        packet
            .serialize(&mut PacketWriter::new(&mut bytes))
            .unwrap();

        assert_eq!(bytes.len(), 5);
        assert_eq!(bytes[4], 0);
        let mut frozen = bytes.freeze();
        let mut decoded = DamageEvent::default();
        decoded
            .deserialize(&mut PacketReader::new(&mut frozen))
            .unwrap();
        assert_eq!(decoded, packet);
        assert_eq!(frozen.remaining(), 0);
    }

    #[test]
    fn damage_event_with_source_position_round_trips() {
        let packet = DamageEvent {
            entity_id: VarInt(1),
            source_type_id: VarInt(0),
            source_cause_id: VarInt(2),
            source_direct_id: VarInt(2),
            has_source_position: true,
            source_position: Some(DamageSourcePosition {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            }),
        };
        let mut bytes = bytes::BytesMut::new();

        packet
            .serialize(&mut PacketWriter::new(&mut bytes))
            .unwrap();

        let mut frozen = bytes.freeze();
        let mut decoded = DamageEvent::default();
        decoded
            .deserialize(&mut PacketReader::new(&mut frozen))
            .unwrap();
        assert_eq!(decoded, packet);
        assert_eq!(frozen.remaining(), 0);
    }
}
