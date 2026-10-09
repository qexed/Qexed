use bytes::{Buf as _, BufMut as _, BytesMut};
use qexed_packet::PacketCodec;
use qexed_packet::net_types::VarInt;
use crate::error::ForgeError;

pub const HANDSHAKE_CHANNEL: &str = "forge:handshake";
pub const DISCRIMINATOR_SERVER_HELLO: u8 = 0x00;
pub const DISCRIMINATOR_CLIENT_MOD_LIST: u8 = 0x01;
pub const DISCRIMINATOR_SERVER_MOD_LIST: u8 = 0x02;
pub const DISCRIMINATOR_ACK: u8 = 0x03;
pub const FML_NETWORK_VERSION: i32 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModEntry {
    pub mod_id: String,
    pub version: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModList {
    pub entries: Vec<ModEntry>,
}

impl ModList {
    pub fn new(entries: Vec<ModEntry>) -> Self { Self { entries } }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
    pub fn mod_ids(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|e| e.mod_id.as_str())
    }
    pub fn check_compatibility(&self, server: &ModList) -> Result<(), Vec<String>> {
        let missing: Vec<String> = server.mod_ids()
            .filter(|id| !self.mod_ids().any(|c| c == *id))
            .map(str::to_string).collect();
        if missing.is_empty() { Ok(()) } else { Err(missing) }
    }
}

fn write_str(buf: &mut BytesMut, s: &str) {
    let mut w = qexed_packet::PacketWriter::new(buf);
    VarInt(s.len() as i32).serialize(&mut w).expect("length fits varint");
    w.buf.put_slice(s.as_bytes());
}

fn read_string(r: &mut qexed_packet::PacketReader) -> Result<String, ForgeError> {
    let mut len = VarInt::default();
    len.deserialize(r)?;
    let len = len.0;
    if len < 0 || len > 1_048_576 {
        return Err(ForgeError::ModListRejected(format!("unreasonable string length")));
    }
    if r.buf.remaining() < len as usize {
        return Err(ForgeError::ModListRejected("string truncated".into()));
    }
    let mut bytes = vec![0u8; len as usize];
    r.buf.copy_to_slice(&mut bytes);
    String::from_utf8(bytes).map_err(|_| ForgeError::ModListRejected("invalid utf-8".into()))
}

pub fn encode_server_hello(fml_version: i32) -> Vec<u8> {
    let mut buf = BytesMut::with_capacity(8);
    buf.put_u8(DISCRIMINATOR_SERVER_HELLO);
    let mut w = qexed_packet::PacketWriter::new(&mut buf);
    VarInt(fml_version).serialize(&mut w).expect("varint");
    buf.to_vec()
}

pub fn encode_server_mod_list(list: &ModList) -> Vec<u8> {
    let mut buf = BytesMut::with_capacity(16 + list.entries.len() * 24);
    buf.put_u8(DISCRIMINATOR_SERVER_MOD_LIST);
    let mut w = qexed_packet::PacketWriter::new(&mut buf);
    VarInt(list.entries.len() as i32).serialize(&mut w).expect("varint");
    for e in &list.entries {
        write_str(&mut buf, &e.mod_id);
        write_str(&mut buf, &e.version);
    }
    buf.to_vec()
}

pub fn encode_ack() -> Vec<u8> {
    vec![DISCRIMINATOR_ACK]
}

#[derive(Debug, PartialEq)]
pub enum HandshakeMessage {
    ServerHello { fml_version: i32 },
    ClientModList(ModList),
    ServerModList(ModList),
    Ack,
    Unknown { discriminator: u8, payload: Vec<u8> },
}

pub fn decode_handshake(data: &[u8]) -> Result<HandshakeMessage, ForgeError> {
    let mut buf = BytesMut::from(data);
    if !buf.has_remaining() {
        return Err(ForgeError::UnexpectedDiscriminator(0));
    }
    let discriminator = buf.get_u8();
    let mut r = qexed_packet::PacketReader::new(&mut buf);
    match discriminator {
        DISCRIMINATOR_SERVER_HELLO => {
            let mut v = VarInt::default();
            v.deserialize(&mut r)?;
            Ok(HandshakeMessage::ServerHello { fml_version: v.0 })
        }
        DISCRIMINATOR_CLIENT_MOD_LIST | DISCRIMINATOR_SERVER_MOD_LIST => {
            let list = read_mod_list(&mut r)?;
            Ok(if discriminator == DISCRIMINATOR_CLIENT_MOD_LIST {
                HandshakeMessage::ClientModList(list)
            } else {
                HandshakeMessage::ServerModList(list)
            })
        }
        DISCRIMINATOR_ACK => Ok(HandshakeMessage::Ack),
        other => Ok(HandshakeMessage::Unknown {
            discriminator: other,
            payload: data[1..].to_vec(),
        }),
    }
}

fn read_mod_list(r: &mut qexed_packet::PacketReader) -> Result<ModList, ForgeError> {
    let mut count = VarInt::default();
    count.deserialize(r)?;
    let count = count.0;
    if count < 0 || count > 65536 {
        return Err(ForgeError::ModListRejected(format!("unreasonable mod count")));
    }
    let mut entries = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let mod_id = read_string(r)?;
        let version = read_string(r)?;
        entries.push(ModEntry { mod_id, version });
    }
    Ok(ModList { entries })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeState {
    AwaitingModList,
    AwaitingClientAck,
    Complete,
}

#[derive(Debug)]
pub struct ServerHandshake {
    state: HandshakeState,
    server_mods: ModList,
    client_mods: Option<ModList>,
}

impl ServerHandshake {
    pub fn new(server_mods: ModList) -> Self {
        Self { state: HandshakeState::AwaitingModList, server_mods, client_mods: None }
    }
    pub fn state(&self) -> HandshakeState { self.state }
    pub fn client_mods(&self) -> Option<&ModList> { self.client_mods.as_ref() }
    pub fn server_mods(&self) -> &ModList { &self.server_mods }

    pub fn on_client_response(&mut self, data: &[u8]) -> Result<Option<Vec<u8>>, ForgeError> {
        match self.state {
            HandshakeState::AwaitingModList => {
                let msg = decode_handshake(data)?;
                let HandshakeMessage::ClientModList(list) = msg else {
                    return Err(ForgeError::UnexpectedDiscriminator(1));
                };
                self.client_mods = Some(list);
                self.state = HandshakeState::AwaitingClientAck;
                Ok(Some(encode_server_mod_list(&self.server_mods)))
            }
            HandshakeState::AwaitingClientAck => {
                let msg = decode_handshake(data)?;
                match msg {
                    HandshakeMessage::Ack => {
                        self.state = HandshakeState::Complete;
                        Ok(None)
                    }
                    _ => Err(ForgeError::UnexpectedDiscriminator(3)),
                }
            }
            HandshakeState::Complete => Ok(None),
        }
    }
}
