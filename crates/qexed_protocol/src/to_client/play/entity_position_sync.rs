use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x23)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct EntityPositionSync {
    pub id: VarInt,
    pub position: PositionPath,
    pub y_rot: f32,
    pub x_rot: f32,
    pub on_ground: bool,
}

/// net.minecraft.world.entity.PositionPath：VarInt 类型 id 分发（0 = LINEAR，1 = STEPPED）。
#[derive(Debug, PartialEq, Clone)]
pub enum PositionPath {
    /// PositionPath$Linear：Vec3 的 STREAM_CODEC（3 个 double）。
    Linear { x: f64, y: f64, z: f64 },
    /// PositionPath$Stepped：PositionStep 列表（每步 Vec3 + VarInt）。
    Stepped { steps: Vec<PositionStep> },
}

impl PacketCodec for PositionPath {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> qexed_packet::Result<()> {
        match self {
            Self::Linear { x, y, z } => {
                VarInt(0).serialize(w)?;
                x.serialize(w)?;
                y.serialize(w)?;
                z.serialize(w)
            }
            Self::Stepped { steps } => {
                VarInt(1).serialize(w)?;
                steps.serialize(w)
            }
        }
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> qexed_packet::Result<()> {
        let mut kind = VarInt::default();
        kind.deserialize(r)?;
        *self = match kind.0 {
            0 => Self::Linear {
                x: r.deserialize()?,
                y: r.deserialize()?,
                z: r.deserialize()?,
            },
            1 => Self::Stepped {
                steps: r.deserialize()?,
            },
            other => {
                return Err(qexed_packet::PacketError::msg(format!(
                    "unsupported position path type: {other}"
                )))
            }
        };
        Ok(())
    }
}

impl Default for PositionPath {
    fn default() -> Self {
        Self::Linear { x: 0.0, y: 0.0, z: 0.0 }
    }
}

/// net.minecraft.world.entity.PositionStep：Vec3（3 个 double）+ VarInt。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PositionStep {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub tick_offset: VarInt,
}

impl PacketCodec for PositionStep {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> qexed_packet::Result<()> {
        self.x.serialize(w)?;
        self.y.serialize(w)?;
        self.z.serialize(w)?;
        self.tick_offset.serialize(w)
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> qexed_packet::Result<()> {
        self.x.deserialize(r)?;
        self.y.deserialize(r)?;
        self.z.deserialize(r)?;
        self.tick_offset.deserialize(r)
    }
}
