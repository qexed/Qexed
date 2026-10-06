//! 连接域编解码辅助（v4 connection::codec 迁移）。

use bytes::BytesMut;
use qexed_packet::{Packet, PacketCodec};

use crate::error::{ConnectionError, Result};
use crate::transport::PacketStream;

pub(super) async fn read_expected_packet<T, R>(packets: &mut PacketStream<R>) -> Result<T>
where
    T: Packet + Default,
    R: tokio::io::AsyncRead + Unpin,
{
    let Some(mut payload) = packets.read_packet().await? else {
        return Err(ConnectionError::msg("connection closed while reading packet"));
    };

    let packet_id = read_packet_id(&mut payload)?;
    if packet_id != T::ID {
        return Err(ConnectionError::msg(format!(
            "packet ID mismatch: expected {}, actual {}",
            T::ID,
            packet_id
        )));
    }

    decode_payload::<T>(&mut payload)
}

pub(crate) fn read_packet_id(payload: &mut BytesMut) -> Result<i32> {
    let mut reader = qexed_packet::PacketReader::new(payload);
    let mut packet_id = qexed_packet::net_types::VarInt::default();
    packet_id.deserialize(&mut reader)?;
    Ok(packet_id.0)
}

pub(crate) fn decode_payload<T>(payload: &mut BytesMut) -> Result<T>
where
    T: Packet + Default,
{
    let mut reader = qexed_packet::PacketReader::new(payload);
    let mut packet = T::default();
    packet.deserialize(&mut reader).map_err(|e| {
        ConnectionError::msg(format!(
            "failed to decode payload as {}: {e}",
            std::any::type_name::<T>()
        ))
    })?;
    Ok(packet)
}

pub(super) fn string_payload(value: &str) -> Result<Vec<u8>> {
    let mut payload = BytesMut::new();
    let mut writer = qexed_packet::PacketWriter::new(&mut payload);
    value.to_string().serialize(&mut writer)?;
    Ok(payload.to_vec())
}
