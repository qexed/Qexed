use bytes::BufMut as _;
use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x1A)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Interact {
    pub entity_id: VarInt,
    pub hand: VarInt,
    pub location: Vec3,
    pub using_secondary_action: bool,
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    const DATA_BITS_MASK: u64 = 32_767;
    const MAX_QUANTIZED_VALUE: f64 = 32_766.0;
    const CONTINUATION_FLAG: u8 = 4;
    const ABS_MAX_VALUE: f64 = 17_179_869_183.0;
    const ABS_MIN_VALUE: f64 = 0.000_030_519_440_883_843_01;

    fn has_continuation_bit(value: u8) -> bool {
        (value & Self::CONTINUATION_FLAG) == Self::CONTINUATION_FLAG
    }

    fn sanitize(value: f64) -> f64 {
        if value.is_nan() {
            0.0
        } else {
            value.clamp(-Self::ABS_MAX_VALUE, Self::ABS_MAX_VALUE)
        }
    }

    fn pack(value: f64) -> u64 {
        ((value * 0.5 + 0.5) * Self::MAX_QUANTIZED_VALUE).round() as u64
    }

    fn unpack(value: u64) -> f64 {
        ((value & Self::DATA_BITS_MASK).min(Self::MAX_QUANTIZED_VALUE as u64) as f64) * 2.0
            / Self::MAX_QUANTIZED_VALUE
            - 1.0
    }
}

impl PacketCodec for Vec3 {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> qexed_packet::Result<()> {
        let x = Self::sanitize(self.x);
        let y = Self::sanitize(self.y);
        let z = Self::sanitize(self.z);
        let chessboard_length = x.abs().max(y.abs()).max(z.abs());
        if chessboard_length < Self::ABS_MIN_VALUE {
            w.buf.put_u8(0);
            return Ok(());
        }

        let scale = chessboard_length.ceil() as u64;
        let is_partial = (scale & 3) != scale;
        let markers = if is_partial {
            (scale & 3) | u64::from(Self::CONTINUATION_FLAG)
        } else {
            scale
        };
        let buffer = markers
            | (Self::pack(x / scale as f64) << 3)
            | (Self::pack(y / scale as f64) << 18)
            | (Self::pack(z / scale as f64) << 33);
        w.buf.put_u8(buffer as u8);
        w.buf.put_u8((buffer >> 8) as u8);
        w.buf.put_u32((buffer >> 16) as u32);
        if is_partial {
            VarInt((scale >> 2) as i32).serialize(w)?;
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> qexed_packet::Result<()> {
        let lowest = r.buf.get_u8();
        if lowest == 0 {
            *self = Vec3::default();
            return Ok(());
        }

        let middle = r.buf.get_u8();
        let highest = u64::from(r.buf.get_u32());
        let buffer = (highest << 16) | (u64::from(middle) << 8) | u64::from(lowest);
        let mut scale = u64::from(lowest & 3);
        if Self::has_continuation_bit(lowest) {
            let mut extra = VarInt::default();
            extra.deserialize(r)?;
            scale |= u64::from(extra.0 as u32) << 2;
        }

        self.x = Self::unpack(buffer >> 3) * scale as f64;
        self.y = Self::unpack(buffer >> 18) * scale as f64;
        self.z = Self::unpack(buffer >> 33) * scale as f64;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use qexed_packet::Packet;

    use super::{Interact, Vec3};

    #[test]
    fn interact_round_trips_with_lp_vec3_location() {
        let packet = Interact {
            entity_id: qexed_packet::net_types::VarInt(42),
            hand: qexed_packet::net_types::VarInt(0),
            location: Vec3 {
                x: 0.25,
                y: 1.0,
                z: -0.5,
            },
            using_secondary_action: false,
        };
        let mut buf = bytes::BytesMut::new();
        let mut writer = qexed_packet::PacketWriter::new(&mut buf);
        packet.serialize(&mut writer).unwrap();

        let mut bytes = buf.freeze();
        let mut reader = qexed_packet::PacketReader::new(&mut bytes);
        let mut decoded = Interact::default();
        decoded.deserialize(&mut reader).unwrap();

        assert_eq!(decoded.entity_id, packet.entity_id);
        assert_eq!(decoded.hand, packet.hand);
        assert_eq!(
            decoded.using_secondary_action,
            packet.using_secondary_action
        );
        assert!((decoded.location.x - packet.location.x).abs() < 0.001);
        assert!((decoded.location.y - packet.location.y).abs() < 0.001);
        assert!((decoded.location.z - packet.location.z).abs() < 0.001);
    }
}
