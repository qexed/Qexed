//! CONFIGURATION / PLAY 阶段的 Forge 自定义通道编解码。

pub const FORGE_LOGIN_CHANNEL: &str = "forge:login";
pub const FORGE_CHANNEL_DATA: &str = "forge:channeldata";

pub const LOGIN_DISCRIMINATOR_READY: i32 = 0x00;
pub const LOGIN_DISCRIMINATOR_REGISTRY: i32 = 0x01;
pub const LOGIN_DISCRIMINATOR_ACK: i32 = 0x02;

/// forge:login 通道内部帧（discriminator varint + payload）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForgeLoginMessage {
    Ready { channels: Vec<String> },
    RegistryData { payload: Vec<u8> },
    Ack,
    Unknown { discriminator: i32, payload: Vec<u8> },
}

impl ForgeLoginMessage {
    pub fn decode(data: &[u8]) -> Result<Self, crate::ForgeError> {
        use bytes::Buf as _;
        use qexed_packet::PacketCodec as _;
        use qexed_packet::net_types::VarInt;

        let mut buf = bytes::BytesMut::from(data);
        if !buf.has_remaining() {
            return Err(crate::ForgeError::UnexpectedDiscriminator(0));
        }
        let mut r = qexed_packet::PacketReader::new(&mut buf);
        let mut disc = VarInt::default();
        disc.deserialize(&mut r)?;
        let consumed = r.buf.remaining();
        let rest = data[data.len() - consumed..].to_vec();

        match disc.0 {
            LOGIN_DISCRIMINATOR_READY => {
                let mut count = VarInt::default();
                count.deserialize(&mut r)?;
                let mut channels = Vec::with_capacity(count.0.max(0) as usize);
                for _ in 0..count.0 {
                    channels.push(read_login_string(&mut r)?);
                }
                Ok(Self::Ready { channels })
            }
            LOGIN_DISCRIMINATOR_REGISTRY => Ok(Self::RegistryData { payload: rest }),
            LOGIN_DISCRIMINATOR_ACK => Ok(Self::Ack),
            other => Ok(Self::Unknown { discriminator: other, payload: rest }),
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        use bytes::BufMut as _;
        use qexed_packet::PacketCodec as _;
        use qexed_packet::net_types::VarInt;

        let mut buf = bytes::BytesMut::new();
        let mut w = qexed_packet::PacketWriter::new(&mut buf);
        match self {
            Self::Ready { channels } => {
                VarInt(LOGIN_DISCRIMINATOR_READY).serialize(&mut w).expect("varint");
                VarInt(channels.len() as i32).serialize(&mut w).expect("varint");
                for c in channels {
                    VarInt(c.len() as i32).serialize(&mut w).expect("varint");
                    w.buf.put_slice(c.as_bytes());
                }
            }
            Self::RegistryData { payload } => {
                VarInt(LOGIN_DISCRIMINATOR_REGISTRY).serialize(&mut w).expect("varint");
                w.buf.extend_from_slice(payload);
            }
            Self::Ack => {
                VarInt(LOGIN_DISCRIMINATOR_ACK).serialize(&mut w).expect("varint");
            }
            Self::Unknown { discriminator, payload } => {
                VarInt(*discriminator).serialize(&mut w).expect("varint");
                w.buf.extend_from_slice(payload);
            }
        }
        buf.to_vec()
    }
}

fn read_login_string(r: &mut qexed_packet::PacketReader) -> Result<String, crate::ForgeError> {
    use qexed_packet::PacketCodec as _;
    use qexed_packet::net_types::VarInt;
    let mut len = VarInt::default();
    len.deserialize(r)?;
    let len = len.0;
    if len < 0 || r.buf.remaining() < len as usize {
        return Err(crate::ForgeError::UnknownChannel("string truncated".into()));
    }
    let mut bytes = vec![0u8; len as usize];
    r.buf.copy_to_slice(&mut bytes);
    String::from_utf8(bytes).map_err(|_| crate::ForgeError::UnknownChannel("invalid utf-8".into()))
}
