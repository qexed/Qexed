use bytes::BufMut;
use qexed_packet::{Packet, PacketCodec, PacketReader, PacketWriter, net_types::VarInt};

#[derive(Debug, Default, PartialEq, Clone)]
pub struct AddEntity {
    pub entity_id: VarInt,
    pub uuid: uuid::Uuid,
    pub entity_type: VarInt,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub velocity_x: f64,
    pub velocity_y: f64,
    pub velocity_z: f64,
    pub pitch: u8,
    pub yaw: u8,
    pub head_yaw: u8,
    pub data: VarInt,
}

impl AddEntity {
    pub fn new(
        entity_id: i32,
        uuid: uuid::Uuid,
        entity_type: i32,
        position: EntityPosition,
        data: i32,
    ) -> Self {
        Self {
            entity_id: VarInt(entity_id),
            uuid,
            entity_type: VarInt(entity_type),
            x: position.x,
            y: position.y,
            z: position.z,
            velocity_x: 0.0,
            velocity_y: 0.0,
            velocity_z: 0.0,
            pitch: pack_degrees(position.pitch),
            yaw: pack_degrees(position.yaw),
            head_yaw: pack_degrees(position.yaw),
            data: VarInt(data),
        }
    }

    pub fn player(
        entity_id: i32,
        uuid: uuid::Uuid,
        entity_type: i32,
        position: EntityPosition,
    ) -> Self {
        Self::new(entity_id, uuid, entity_type, position, 0)
    }
}

impl Packet for AddEntity {
    const ID: i32 = 0x01;

    fn serialize(&self, w: &mut PacketWriter) -> anyhow::Result<()> {
        self.entity_id.serialize(w)?;
        self.uuid.serialize(w)?;
        self.entity_type.serialize(w)?;
        self.x.serialize(w)?;
        self.y.serialize(w)?;
        self.z.serialize(w)?;
        write_lp_vec3(w, self.velocity_x, self.velocity_y, self.velocity_z)?;
        self.pitch.serialize(w)?;
        self.yaw.serialize(w)?;
        self.head_yaw.serialize(w)?;
        self.data.serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> anyhow::Result<()> {
        self.entity_id.deserialize(r)?;
        self.uuid.deserialize(r)?;
        self.entity_type.deserialize(r)?;
        self.x.deserialize(r)?;
        self.y.deserialize(r)?;
        self.z.deserialize(r)?;
        let (velocity_x, velocity_y, velocity_z) = read_lp_vec3(r)?;
        self.velocity_x = velocity_x;
        self.velocity_y = velocity_y;
        self.velocity_z = velocity_z;
        self.pitch.deserialize(r)?;
        self.yaw.deserialize(r)?;
        self.head_yaw.deserialize(r)?;
        self.data.deserialize(r)
    }
}

#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub struct EntityPosition {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct TeleportEntity {
    pub entity_id: VarInt,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub velocity_x: f64,
    pub velocity_y: f64,
    pub velocity_z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub relative_flags: i32,
    pub on_ground: bool,
}

impl TeleportEntity {
    pub fn from_position(entity_id: i32, position: EntityPosition) -> Self {
        Self {
            entity_id: VarInt(entity_id),
            x: position.x,
            y: position.y,
            z: position.z,
            velocity_x: 0.0,
            velocity_y: 0.0,
            velocity_z: 0.0,
            yaw: position.yaw,
            pitch: position.pitch,
            relative_flags: 0,
            on_ground: position.on_ground,
        }
    }
}

impl Packet for TeleportEntity {
    const ID: i32 = 0x7D;

    fn serialize(&self, w: &mut PacketWriter) -> anyhow::Result<()> {
        self.entity_id.serialize(w)?;
        self.x.serialize(w)?;
        self.y.serialize(w)?;
        self.z.serialize(w)?;
        self.velocity_x.serialize(w)?;
        self.velocity_y.serialize(w)?;
        self.velocity_z.serialize(w)?;
        self.yaw.serialize(w)?;
        self.pitch.serialize(w)?;
        self.relative_flags.serialize(w)?;
        self.on_ground.serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> anyhow::Result<()> {
        self.entity_id.deserialize(r)?;
        self.x.deserialize(r)?;
        self.y.deserialize(r)?;
        self.z.deserialize(r)?;
        self.velocity_x.deserialize(r)?;
        self.velocity_y.deserialize(r)?;
        self.velocity_z.deserialize(r)?;
        self.yaw.deserialize(r)?;
        self.pitch.deserialize(r)?;
        self.relative_flags.deserialize(r)?;
        self.on_ground.deserialize(r)
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct EntityPositionSync {
    pub entity_id: VarInt,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub velocity_x: f64,
    pub velocity_y: f64,
    pub velocity_z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
}

impl EntityPositionSync {
    pub fn from_position(entity_id: i32, position: EntityPosition) -> Self {
        Self {
            entity_id: VarInt(entity_id),
            x: position.x,
            y: position.y,
            z: position.z,
            velocity_x: 0.0,
            velocity_y: 0.0,
            velocity_z: 0.0,
            yaw: position.yaw,
            pitch: position.pitch,
            on_ground: position.on_ground,
        }
    }
}

impl Packet for EntityPositionSync {
    const ID: i32 = 0x23;

    fn serialize(&self, w: &mut PacketWriter) -> anyhow::Result<()> {
        self.entity_id.serialize(w)?;
        self.x.serialize(w)?;
        self.y.serialize(w)?;
        self.z.serialize(w)?;
        self.velocity_x.serialize(w)?;
        self.velocity_y.serialize(w)?;
        self.velocity_z.serialize(w)?;
        self.yaw.serialize(w)?;
        self.pitch.serialize(w)?;
        self.on_ground.serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> anyhow::Result<()> {
        self.entity_id.deserialize(r)?;
        self.x.deserialize(r)?;
        self.y.deserialize(r)?;
        self.z.deserialize(r)?;
        self.velocity_x.deserialize(r)?;
        self.velocity_y.deserialize(r)?;
        self.velocity_z.deserialize(r)?;
        self.yaw.deserialize(r)?;
        self.pitch.deserialize(r)?;
        self.on_ground.deserialize(r)
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct RotateHead {
    pub entity_id: VarInt,
    pub head_yaw: u8,
}

impl RotateHead {
    pub fn new(entity_id: i32, yaw: f32) -> Self {
        Self {
            entity_id: VarInt(entity_id),
            head_yaw: pack_degrees(yaw),
        }
    }
}

impl Packet for RotateHead {
    const ID: i32 = 0x53;

    fn serialize(&self, w: &mut PacketWriter) -> anyhow::Result<()> {
        self.entity_id.serialize(w)?;
        self.head_yaw.serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> anyhow::Result<()> {
        self.entity_id.deserialize(r)?;
        self.head_yaw.deserialize(r)
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct RemoveEntities {
    pub entity_ids: Vec<VarInt>,
}

impl RemoveEntities {
    pub fn one(entity_id: i32) -> Self {
        Self {
            entity_ids: vec![VarInt(entity_id)],
        }
    }
}

impl Packet for RemoveEntities {
    const ID: i32 = 0x4D;

    fn serialize(&self, w: &mut PacketWriter) -> anyhow::Result<()> {
        self.entity_ids.serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> anyhow::Result<()> {
        self.entity_ids.deserialize(r)
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct PlayerInfoRemove {
    pub profile_ids: Vec<uuid::Uuid>,
}

impl PlayerInfoRemove {
    pub fn one(profile_id: uuid::Uuid) -> Self {
        Self {
            profile_ids: vec![profile_id],
        }
    }
}

impl Packet for PlayerInfoRemove {
    const ID: i32 = 0x45;

    fn serialize(&self, w: &mut PacketWriter) -> anyhow::Result<()> {
        self.profile_ids.serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> anyhow::Result<()> {
        self.profile_ids.deserialize(r)
    }
}

fn pack_degrees(degrees: f32) -> u8 {
    ((degrees * 256.0 / 360.0).floor() as i32 & 0xff) as u8
}

fn write_lp_vec3(w: &mut PacketWriter, x: f64, y: f64, z: f64) -> anyhow::Result<()> {
    const ABS_MIN_VALUE: f64 = 3.051944088384301E-5;
    const ABS_MAX_VALUE: f64 = 1.7179869183E10;
    const DATA_MASK: f64 = 32766.0;

    let x = sanitize_lp_value(x, ABS_MAX_VALUE);
    let y = sanitize_lp_value(y, ABS_MAX_VALUE);
    let z = sanitize_lp_value(z, ABS_MAX_VALUE);
    let chessboard_length = x.abs().max(y.abs()).max(z.abs());
    if chessboard_length < ABS_MIN_VALUE {
        w.buf.put_u8(0);
        return Ok(());
    }

    let scale = chessboard_length.ceil() as u64;
    let is_partial = (scale & 3) != scale;
    let markers = if is_partial { (scale & 3) | 4 } else { scale };
    let buffer = markers
        | (pack_lp_value(x / scale as f64, DATA_MASK) << 3)
        | (pack_lp_value(y / scale as f64, DATA_MASK) << 18)
        | (pack_lp_value(z / scale as f64, DATA_MASK) << 33);

    w.buf.put_u8(buffer as u8);
    w.buf.put_u8((buffer >> 8) as u8);
    w.buf.put_i32((buffer >> 16) as i32);
    if is_partial {
        VarInt((scale >> 2) as i32).serialize(w)?;
    }
    Ok(())
}

fn read_lp_vec3(r: &mut PacketReader) -> anyhow::Result<(f64, f64, f64)> {
    let lowest = r.buf.get_u8();
    if lowest == 0 {
        return Ok((0.0, 0.0, 0.0));
    }

    let middle = r.buf.get_u8();
    let highest = r.buf.get_u32();
    let buffer = ((highest as u64) << 16) | ((middle as u64) << 8) | lowest as u64;
    let mut scale = (lowest & 3) as u64;
    if lowest & 4 == 4 {
        let mut continuation = VarInt::default();
        continuation.deserialize(r)?;
        scale |= (continuation.0 as u32 as u64) << 2;
    }

    Ok((
        unpack_lp_value(buffer >> 3) * scale as f64,
        unpack_lp_value(buffer >> 18) * scale as f64,
        unpack_lp_value(buffer >> 33) * scale as f64,
    ))
}

fn sanitize_lp_value(value: f64, abs_max: f64) -> f64 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(-abs_max, abs_max)
    }
}

fn pack_lp_value(value: f64, data_mask: f64) -> u64 {
    ((value * 0.5 + 0.5) * data_mask).round() as u64
}

fn unpack_lp_value(value: u64) -> f64 {
    const DATA_BITS_MASK: u64 = 32767;
    const MAX_QUANTIZED_VALUE: f64 = 32766.0;

    ((value & DATA_BITS_MASK).min(MAX_QUANTIZED_VALUE as u64) as f64) * 2.0 / MAX_QUANTIZED_VALUE
        - 1.0
}

#[cfg(test)]
mod tests {
    use qexed_packet::Packet;

    use super::{AddEntity, EntityPosition, TeleportEntity};

    #[test]
    fn player_add_entity_roundtrips() {
        let packet = AddEntity::player(
            7,
            uuid::Uuid::from_u128(0x00112233445566778899aabbccddeeff),
            146,
            EntityPosition {
                x: 1.0,
                y: 2.0,
                z: 3.0,
                yaw: 90.0,
                pitch: 45.0,
                on_ground: true,
            },
        );
        let mut buf = bytes::BytesMut::new();
        let mut writer = qexed_packet::PacketWriter::new(&mut buf);
        packet.serialize(&mut writer).unwrap();

        let mut bytes = buf.freeze();
        let mut reader = qexed_packet::PacketReader::new(&mut bytes);
        let mut decoded = AddEntity::default();
        decoded.deserialize(&mut reader).unwrap();

        assert_eq!(decoded, packet);
    }

    #[test]
    fn teleport_entity_roundtrips() {
        let packet = TeleportEntity::from_position(
            7,
            EntityPosition {
                x: 1.0,
                y: 2.0,
                z: 3.0,
                yaw: 90.0,
                pitch: 45.0,
                on_ground: true,
            },
        );
        let mut buf = bytes::BytesMut::new();
        let mut writer = qexed_packet::PacketWriter::new(&mut buf);
        packet.serialize(&mut writer).unwrap();

        let mut bytes = buf.freeze();
        let mut reader = qexed_packet::PacketReader::new(&mut bytes);
        let mut decoded = TeleportEntity::default();
        decoded.deserialize(&mut reader).unwrap();

        assert_eq!(decoded, packet);
    }
}
