use qexed_packet::{PacketCodec, net_types::VarInt};

#[qexed_packet_macros::packet(id = 0x1A)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Interact {
    pub entity_id: VarInt,
    pub action: InteractAction,
    pub using_secondary_action: bool,
}

#[derive(Debug, PartialEq, Clone)]
pub enum InteractAction {
    Interact { hand: VarInt },
    Attack,
    InteractAt { target: Vec3, hand: VarInt },
}

impl Default for InteractAction {
    fn default() -> Self {
        Self::Interact { hand: VarInt(0) }
    }
}

impl PacketCodec for InteractAction {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
        match self {
            Self::Interact { hand } => {
                VarInt(0).serialize(w)?;
                hand.serialize(w)
            }
            Self::Attack => VarInt(1).serialize(w),
            Self::InteractAt { target, hand } => {
                VarInt(2).serialize(w)?;
                target.serialize(w)?;
                hand.serialize(w)
            }
        }
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
        let mut action_id = VarInt::default();
        action_id.deserialize(r)?;
        *self = match action_id.0 {
            0 => {
                let mut hand = VarInt::default();
                hand.deserialize(r)?;
                Self::Interact { hand }
            }
            1 => Self::Attack,
            2 => {
                let mut target = Vec3::default();
                target.deserialize(r)?;
                let mut hand = VarInt::default();
                hand.deserialize(r)?;
                Self::InteractAt { target, hand }
            }
            other => anyhow::bail!("unsupported interact action: {other}"),
        };
        Ok(())
    }
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[cfg(test)]
mod tests {
    use qexed_packet::Packet;

    use super::{Interact, InteractAction, Vec3};

    #[test]
    fn interact_at_round_trips() {
        let packet = Interact {
            entity_id: qexed_packet::net_types::VarInt(42),
            action: InteractAction::InteractAt {
                target: Vec3 {
                    x: 0.25,
                    y: 1.0,
                    z: -0.5,
                },
                hand: qexed_packet::net_types::VarInt(0),
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

        assert_eq!(decoded, packet);
    }
}
