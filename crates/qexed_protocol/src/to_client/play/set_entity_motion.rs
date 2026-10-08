use qexed_packet::{Packet, PacketCodec, PacketReader, PacketWriter, net_types::VarInt};
use qexed_packet::error::PacketError;

use super::add_entity::{read_lp_vec3, write_lp_vec3};

#[derive(Debug, Default, PartialEq, Clone)]
pub struct SetEntityMotion {
    pub entity_id: VarInt,
    pub velocity_x: f64,
    pub velocity_y: f64,
    pub velocity_z: f64,
}

impl SetEntityMotion {
    pub fn from_velocity(entity_id: i32, x: f64, y: f64, z: f64) -> Self {
        Self {
            entity_id: VarInt(entity_id),
            velocity_x: x,
            velocity_y: y,
            velocity_z: z,
        }
    }
}

impl Packet for SetEntityMotion {
    const ID: i32 = 0x65;

    fn serialize(&self, w: &mut PacketWriter) -> Result<(), PacketError> {
        self.entity_id.serialize(w)?;
        write_lp_vec3(w, self.velocity_x, self.velocity_y, self.velocity_z)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> Result<(), PacketError> {
        self.entity_id.deserialize(r)?;
        let (velocity_x, velocity_y, velocity_z) = read_lp_vec3(r)?;
        self.velocity_x = velocity_x;
        self.velocity_y = velocity_y;
        self.velocity_z = velocity_z;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use qexed_packet::Packet;

    use super::SetEntityMotion;

    #[test]
    fn set_entity_motion_uses_lp_vec3() {
        let packet = SetEntityMotion::from_velocity(7, 0.0, 0.0, 0.0);
        let mut zero_buf = bytes::BytesMut::new();
        let mut zero_writer = qexed_packet::PacketWriter::new(&mut zero_buf);
        packet.serialize(&mut zero_writer).unwrap();
        assert_eq!(
            zero_buf.len(),
            2,
            "zero motion must be encoded as entity id plus one LpVec3 marker"
        );

        let packet = SetEntityMotion::from_velocity(300, 0.12, 0.42, -0.06);
        let mut buf = bytes::BytesMut::new();
        let mut writer = qexed_packet::PacketWriter::new(&mut buf);
        packet.serialize(&mut writer).unwrap();

        let mut bytes = buf.freeze();
        let mut reader = qexed_packet::PacketReader::new(&mut bytes);
        let mut decoded = SetEntityMotion::default();
        decoded.deserialize(&mut reader).unwrap();

        assert_eq!(decoded.entity_id, packet.entity_id);
        assert!((decoded.velocity_x - packet.velocity_x).abs() < 0.001);
        assert!((decoded.velocity_y - packet.velocity_y).abs() < 0.001);
        assert!((decoded.velocity_z - packet.velocity_z).abs() < 0.001);
    }
}
