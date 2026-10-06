use qexed_packet::{
    Packet, PacketCodec, PacketReader, PacketWriter,
    net_types::{AnyNbt, VarInt},
};

#[derive(Debug, PartialEq, Clone)]
pub struct BossEvent {
    pub id: uuid::Uuid,
    pub operation: BossEventOperation,
}

impl Default for BossEvent {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::nil(),
            operation: BossEventOperation::Remove,
        }
    }
}

impl Packet for BossEvent {
    const ID: i32 = 0x8;

    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        self.id.serialize(w)?;
        self.operation.serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        self.id.deserialize(r)?;
        self.operation.deserialize(r)
    }
}

/// ClientboundBossEventPacket$Operation：变体 id（readEnum 的 VarInt 序数）后跟变体负载。
#[derive(Debug, PartialEq, Clone)]
pub enum BossEventOperation {
    Add {
        name: AnyNbt,
        progress: f32,
        color: BossBarColor,
        overlay: BossBarOverlay,
        darken_screen: bool,
        play_music: bool,
        create_world_fog: bool,
    },
    Remove,
    UpdateProgress {
        progress: f32,
    },
    UpdateName {
        name: AnyNbt,
    },
    UpdateStyle {
        color: BossBarColor,
        overlay: BossBarOverlay,
    },
    UpdateProperties {
        darken_screen: bool,
        play_music: bool,
        create_world_fog: bool,
    },
}

impl Default for BossEventOperation {
    fn default() -> Self {
        Self::Remove
    }
}

impl PacketCodec for BossEventOperation {
    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        match self {
            Self::Add {
                name,
                progress,
                color,
                overlay,
                darken_screen,
                play_music,
                create_world_fog,
            } => {
                VarInt(0).serialize(w)?;
                name.serialize(w)?;
                progress.serialize(w)?;
                color.serialize(w)?;
                overlay.serialize(w)?;
                boss_bar_flags(*darken_screen, *play_music, *create_world_fog).serialize(w)
            }
            Self::Remove => VarInt(1).serialize(w),
            Self::UpdateProgress { progress } => {
                VarInt(2).serialize(w)?;
                progress.serialize(w)
            }
            Self::UpdateName { name } => {
                VarInt(3).serialize(w)?;
                name.serialize(w)
            }
            Self::UpdateStyle { color, overlay } => {
                VarInt(4).serialize(w)?;
                color.serialize(w)?;
                overlay.serialize(w)
            }
            Self::UpdateProperties {
                darken_screen,
                play_music,
                create_world_fog,
            } => {
                VarInt(5).serialize(w)?;
                boss_bar_flags(*darken_screen, *play_music, *create_world_fog).serialize(w)
            }
        }
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        let mut operation_id = VarInt::default();
        operation_id.deserialize(r)?;
        *self = match operation_id.0 {
            0 => {
                let name = r.deserialize()?;
                let progress = r.deserialize()?;
                let color = r.deserialize()?;
                let overlay = r.deserialize()?;
                let (darken_screen, play_music, create_world_fog) = read_boss_bar_flags(r)?;
                Self::Add {
                    name,
                    progress,
                    color,
                    overlay,
                    darken_screen,
                    play_music,
                    create_world_fog,
                }
            }
            1 => Self::Remove,
            2 => Self::UpdateProgress {
                progress: r.deserialize()?,
            },
            3 => Self::UpdateName {
                name: r.deserialize()?,
            },
            4 => Self::UpdateStyle {
                color: r.deserialize()?,
                overlay: r.deserialize()?,
            },
            5 => {
                let (darken_screen, play_music, create_world_fog) = read_boss_bar_flags(r)?;
                Self::UpdateProperties {
                    darken_screen,
                    play_music,
                    create_world_fog,
                }
            }
            other => {
                return Err(qexed_packet::PacketError::msg(format!(
                    "unsupported boss event operation: {other}"
                )))
            }
        };
        Ok(())
    }
}

/// BossEvent$BossBarColor：readEnum VarInt 序数。
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub enum BossBarColor {
    Pink,
    Blue,
    Red,
    #[default]
    Green,
    Yellow,
    Purple,
    White,
}

impl PacketCodec for BossBarColor {
    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        VarInt(*self as i32).serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        let mut value = VarInt::default();
        value.deserialize(r)?;
        *self = match value.0 {
            0 => Self::Pink,
            1 => Self::Blue,
            2 => Self::Red,
            3 => Self::Green,
            4 => Self::Yellow,
            5 => Self::Purple,
            6 => Self::White,
            other => {
                return Err(qexed_packet::PacketError::msg(format!(
                    "unsupported boss bar color: {other}"
                )))
            }
        };
        Ok(())
    }
}

/// BossEvent$BossBarOverlay：readEnum VarInt 序数。
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub enum BossBarOverlay {
    #[default]
    Progress,
    Notched6,
    Notched10,
    Notched12,
    Notched20,
}

impl PacketCodec for BossBarOverlay {
    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        VarInt(*self as i32).serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        let mut value = VarInt::default();
        value.deserialize(r)?;
        *self = match value.0 {
            0 => Self::Progress,
            1 => Self::Notched6,
            2 => Self::Notched10,
            3 => Self::Notched12,
            4 => Self::Notched20,
            other => {
                return Err(qexed_packet::PacketError::msg(format!(
                    "unsupported boss bar overlay: {other}"
                )))
            }
        };
        Ok(())
    }
}

fn boss_bar_flags(darken_screen: bool, play_music: bool, create_world_fog: bool) -> u8 {
    u8::from(darken_screen) | (u8::from(play_music) << 1) | (u8::from(create_world_fog) << 2)
}

fn read_boss_bar_flags(r: &mut PacketReader) -> qexed_packet::Result<(bool, bool, bool)> {
    let flags = r.buf.get_u8();
    Ok((flags & 0x01 != 0, flags & 0x02 != 0, flags & 0x04 != 0))
}
