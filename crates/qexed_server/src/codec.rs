//! 收发包辅助：ID 校验、带超时读取、序列化小工具。

use std::time::Duration;

use bytes::BytesMut;
use qexed_packet::net_types::VarInt;
use qexed_packet::{Packet, PacketCodec, PacketReader, PacketWriter};

use crate::error::ServerError;

/// 带超时的 `packets.read_packet()`：
/// - `Ok(Some(payload))` 正常收到
/// - `Ok(None)` 连接已关闭
/// - `Err(_)` 超时或底层 IO 错误
pub(crate) async fn read_packet_or_timeout<R>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
    timeout_dur: Duration,
) -> Result<Option<BytesMut>, ServerError>
where
    R: tokio::io::AsyncRead + Unpin,
{
    tokio::time::timeout(timeout_dur, packets.read_packet())
        .await
        .map_err(|_| ServerError::Protocol(format!("read timeout after {timeout_dur:?}")))?
        .map_err(Into::into)
}

/// 读取并校验预期包 ID，然后反序列化。
pub(crate) async fn read_expected_packet<T, R>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
    timeout_dur: Duration,
) -> Result<T, ServerError>
where
    T: Packet + Default,
    R: tokio::io::AsyncRead + Unpin,
{
    let Some(mut payload) = read_packet_or_timeout(packets, timeout_dur).await? else {
        return Err(ServerError::Protocol(
            "connection closed while reading packet".into(),
        ));
    };
    let id = read_packet_id(&mut payload)?;
    if id != T::ID {
        return Err(ServerError::Protocol(format!(
            "packet ID mismatch: expected {}, actual {}",
            T::ID,
            id
        )));
    }
    decode_payload::<T>(&mut payload)
}

pub(crate) fn read_packet_id(payload: &mut BytesMut) -> Result<i32, ServerError> {
    let mut reader = PacketReader::new(payload);
    let mut id = VarInt::default();
    id.deserialize(&mut reader)
        .map_err(|err| ServerError::Protocol(err.to_string()))?;
    Ok(id.0)
}

pub(crate) fn decode_payload<T>(payload: &mut BytesMut) -> Result<T, ServerError>
where
    T: Packet + Default,
{
    let mut reader = PacketReader::new(payload);
    let mut packet = T::default();
    packet
        .deserialize(&mut reader)
        .map_err(|err| ServerError::Protocol(err.to_string()))?;
    Ok(packet)
}

/// 把一个 String 序列化为裸字节（用于自定义 payload 的 RestBuffer）。
pub(crate) fn string_payload(value: &str) -> Result<Vec<u8>, ServerError> {
    let mut payload = BytesMut::new();
    let mut writer = PacketWriter::new(&mut payload);
    value
        .to_string()
        .serialize(&mut writer)
        .map_err(|err| ServerError::Protocol(err.to_string()))?;
    Ok(payload.to_vec())
}
