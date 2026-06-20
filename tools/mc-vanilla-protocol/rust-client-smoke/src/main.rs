use std::{
    env,
    io::{Read, Write},
    net::TcpStream,
};

use anyhow::{Context, Result};
use bytes::{BufMut as _, BytesMut};
use qexed_packet::{Packet, PacketCodec, PacketWriter, net_types::VarInt};
use qexed_protocol::to_server::{
    configuration::{
        finish_configuration::FinishConfiguration, select_known_packs::SelectKnownPacks,
        settings::Settings,
    },
    handshaking::set_protocol::SetProtocol,
    login::{login_acknowledged::LoginAcknowledged, login_start::LoginStart},
    play::{
        chunk_batch_received::ChunkBatchReceived, keep_alive::KeepAlive,
        move_player_pos::MovePlayerPos, player_loaded::PlayerLoaded,
    },
};

const PROTOCOL_VERSION: i32 = 776;

fn main() -> Result<()> {
    let port = env::args()
        .nth(1)
        .context("missing server port argument")?
        .parse::<u16>()
        .context("invalid server port")?;
    let mut stream = TcpStream::connect(("127.0.0.1", port)).context("connect Java protocol server")?;

    write_packet(
        &mut stream,
        &SetProtocol {
            protocol_version: VarInt(PROTOCOL_VERSION),
            server_host: "127.0.0.1".to_string(),
            server_port: port,
            next_state: VarInt(2),
        },
    )?;
    write_packet(
        &mut stream,
        &LoginStart {
            username: "RustCodec".to_string(),
            player_uuid: uuid::Uuid::new_v3(
                &uuid::Uuid::NAMESPACE_OID,
                b"OfflinePlayer:RustCodec",
            ),
        },
    )?;
    write_packet(&mut stream, &LoginAcknowledged {})?;

    write_packet(
        &mut stream,
        &Settings {
            locale: "en_us".to_string(),
            view_distance: 2,
            chat_mode: VarInt(0),
            chat_colors: true,
            displayed_skin_parts: 0,
            main_hand: VarInt(1),
            enable_text_filtering: false,
            allow_server_listings: false,
            particle_status: VarInt(0),
        },
    )?;
    write_packet(&mut stream, &SelectKnownPacks { entries: Vec::new() })?;
    write_packet(&mut stream, &FinishConfiguration {})?;

    write_packet(&mut stream, &PlayerLoaded)?;
    write_packet(
        &mut stream,
        &MovePlayerPos {
            x: 0.5,
            y: 64.0,
            z: 0.5,
            flags: 1,
        },
    )?;
    write_packet(
        &mut stream,
        &ChunkBatchReceived {
            desired_chunks_per_tick: 20.0,
        },
    )?;
    write_packet(
        &mut stream,
        &KeepAlive {
            keep_alive_id: 123456789,
        },
    )?;

    let mut ack = [0_u8; 1];
    stream.read_exact(&mut ack).context("read Java server ack")?;
    anyhow::ensure!(ack[0] == 1, "unexpected Java server ack: {}", ack[0]);
    Ok(())
}

fn write_packet<T>(stream: &mut TcpStream, packet: &T) -> Result<()>
where
    T: Packet,
{
    let payload = build_payload(T::ID, packet)?;
    write_frame(stream, &payload)
}

fn build_payload<T>(packet_id: i32, packet: &T) -> Result<BytesMut>
where
    T: Packet,
{
    let mut payload = BytesMut::new();
    {
        let mut writer = PacketWriter::new(&mut payload);
        VarInt(packet_id).serialize(&mut writer)?;
        packet.serialize(&mut writer)?;
    }
    Ok(payload)
}

fn write_frame(stream: &mut TcpStream, payload: &[u8]) -> Result<()> {
    let mut frame = BytesMut::new();
    write_varint(&mut frame, payload.len() as i32);
    frame.put_slice(payload);
    stream.write_all(&frame).context("write frame")
}

fn write_varint(buf: &mut BytesMut, value: i32) {
    let mut value = value as u32;
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        buf.put_u8(byte);
        if value == 0 {
            break;
        }
    }
}
