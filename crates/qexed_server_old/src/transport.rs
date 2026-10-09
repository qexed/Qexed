//! 异步包分帧：MC 协议的 长度前缀(varint) + [数据长度(varint) + zlib] 帧。

use bytes::{Buf, BufMut, BytesMut};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use std::io::Read as _;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::error::ServerError;

const READ_CHUNK: usize = 4096;
const MAX_PACKET_SIZE: usize = 8 * 1024 * 1024;

/// 连接的读侧：解帧（+ 解压）出单个包的字节。
pub struct PacketReader<R> {
    inner: R,
    buffer: BytesMut,
    compression_threshold: Option<i32>,
    max_packet_size: usize,
}

impl<R: AsyncRead + Unpin> PacketReader<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            buffer: BytesMut::with_capacity(READ_CHUNK),
            compression_threshold: None,
            max_packet_size: MAX_PACKET_SIZE,
        }
    }

    pub fn set_compression_threshold(&mut self, threshold: i32) {
        self.compression_threshold = (threshold >= 0).then_some(threshold);
    }

    /// 读一个包；连接正常关闭且无残包时返回 None。
    pub async fn read_packet(&mut self) -> Result<Option<BytesMut>, ServerError> {
        loop {
            if let Some(packet) = self.try_parse()? {
                return Ok(Some(packet));
            }
            let mut tmp = [0u8; READ_CHUNK];
            let n = self.inner.read(&mut tmp).await?;
            if n == 0 {
                if self.buffer.is_empty() {
                    return Ok(None);
                }
                return Err(ServerError::ConnectionClosed { what: "incomplete packet" });
            }
            self.buffer.extend_from_slice(&tmp[..n]);
        }
    }

    fn try_parse(&mut self) -> Result<Option<BytesMut>, ServerError> {
        let Some((packet_len, header_len)) = read_varint_prefix(&self.buffer)? else {
            return Ok(None);
        };
        if packet_len < 0 {
            return Err(ServerError::InvalidPacketLength(packet_len));
        }
        let packet_len = packet_len as usize;
        if packet_len > self.max_packet_size {
            return Err(ServerError::PacketTooLarge { len: packet_len, max: self.max_packet_size });
        }
        let frame_len = header_len + packet_len;
        if self.buffer.len() < frame_len {
            return Ok(None);
        }
        let mut frame = self.buffer.split_to(frame_len);
        frame.advance(header_len);
        self.decode_frame(frame).map(Some)
    }

    fn decode_frame(&self, frame: BytesMut) -> Result<BytesMut, ServerError> {
        let Some(_threshold) = self.compression_threshold else {
            return Ok(frame);
        };
        // 压缩帧：数据长度 varint + zlib 数据（0 = 未压缩）
        let mut cursor = std::io::Cursor::new(frame.as_ref());
        let data_len = read_varint_sync(&mut cursor)
            .map_err(|e| ServerError::Compression(e.to_string()))?;
        let header_len = cursor.position() as usize;
        if data_len == 0 {
            return Ok(BytesMut::from(&frame[header_len..]));
        }
        if data_len < 0 {
            return Err(ServerError::Compression(format!("negative data length {data_len}")));
        }
        if (data_len as usize) > self.max_packet_size {
            return Err(ServerError::PacketTooLarge { len: data_len as usize, max: self.max_packet_size });
        }
        let mut decoder = ZlibDecoder::new(&frame[header_len..]);
        let mut out = Vec::with_capacity(data_len as usize);
        decoder.read_to_end(&mut out)
            .map_err(|e| ServerError::Compression(e.to_string()))?;
        if out.len() != data_len as usize {
            return Err(ServerError::Compression(format!("decompressed size mismatch: {} != {}", out.len(), data_len)));
        }
        Ok(BytesMut::from(&out[..]))
    }
}

fn read_varint_prefix(buf: &[u8]) -> Result<Option<(i32, usize)>, ServerError> {
    let mut value = 0i32;
    for position in 0..5 {
        let Some(&byte) = buf.get(position) else { return Ok(None) };
        value |= ((byte & 0x7F) as i32) << (7 * position);
        if byte & 0x80 == 0 {
            return Ok(Some((value, position + 1)));
        }
    }
    Err(ServerError::Compression("varint too long".into()))
}

fn read_varint_sync(r: &mut impl std::io::Read) -> std::io::Result<i32> {
    let mut value = 0i32;
    for position in 0..5 {
        let mut byte = [0u8; 1];
        r.read_exact(&mut byte)?;
        value |= ((byte[0] & 0x7F) as i32) << (7 * position);
        if byte[0] & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "varint too long"))
}

/// 连接的写侧：封帧（+ 压缩）。
pub struct PacketWriter<W> {
    inner: W,
    compression_threshold: Option<i32>,
    max_packet_size: usize,
}

impl<W: AsyncWrite + Unpin> PacketWriter<W> {
    pub fn new(inner: W) -> Self {
        Self { inner, compression_threshold: None, max_packet_size: MAX_PACKET_SIZE }
    }

    pub fn set_compression_threshold(&mut self, threshold: i32) {
        self.compression_threshold = (threshold >= 0).then_some(threshold);
    }

    /// 发送一个原始包（已含包 ID varint 的负载）。
    pub async fn send_raw(&mut self, mut payload: BytesMut) -> Result<(), ServerError> {
        let frame = self.encode_frame(&mut payload);
        self.inner.write_all(&frame).await?;
        self.inner.flush().await?;
        Ok(())
    }

    fn encode_frame(&self, payload: &[u8]) -> BytesMut {
        let mut frame = BytesMut::new();
        match self.compression_threshold {
            Some(threshold) if payload.len() >= threshold as usize => {
                let mut encoder = ZlibEncoder::new(Vec::new(), flate2::Compression::default());
                use std::io::Write as _;
                encoder.write_all(payload).expect("zlib encode");
                let compressed = encoder.finish().expect("zlib finish");
                let data_len_varint_size = varint_len(payload.len() as i32);
                write_varint_buf(&mut frame, (compressed.len() + data_len_varint_size) as i32);
                write_varint_buf(&mut frame, payload.len() as i32);
                frame.put_slice(&compressed);
            }
            Some(_) => {
                // 未压缩：数据长度 = 0
                write_varint_buf(&mut frame, (payload.len() + 1) as i32);
                write_varint_buf(&mut frame, 0);
                frame.put_slice(payload);
            }
            None => {
                write_varint_buf(&mut frame, payload.len() as i32);
                frame.put_slice(payload);
            }
        }
        frame
    }
}

fn varint_len(value: i32) -> usize {
    let mut val = value as u32;
    let mut len = 1;
    loop {
        val >>= 7;
        if val == 0 { return len; }
        len += 1;
    }
}

fn write_varint_buf(buf: &mut BytesMut, value: i32) {
    let mut val = value as u32;
    loop {
        let mut temp = (val & 0x7F) as u8;
        val >>= 7;
        if val != 0 { temp |= 0x80; }
        buf.put_u8(temp);
        if val == 0 { break; }
    }
}

/// 一条客户端连接的读写对。
pub struct Connection<R, W> {
    pub reader: PacketReader<R>,
    pub writer: PacketWriter<W>,
}

impl<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> Connection<R, W> {
    pub fn new(reader: R, writer: W) -> Self {
        Self { reader: PacketReader::new(reader), writer: PacketWriter::new(writer) }
    }

    pub fn set_compression(&mut self, threshold: i32) {
        self.reader.set_compression_threshold(threshold);
        self.writer.set_compression_threshold(threshold);
    }

    /// 读一个包并解析为指定类型（校验包 ID）。
    pub async fn read_packet<T: qexed_packet::Packet>(&mut self, what: &'static str) -> Result<T, ServerError> {
        let Some(mut payload) = self.reader.read_packet().await? else {
            return Err(ServerError::ConnectionClosed { what });
        };
        let actual = read_packet_id(&mut payload)?;
        if actual != T::ID {
            return Err(ServerError::PacketIdMismatch { expected: T::ID, actual });
        }
        decode_payload::<T>(&mut payload)
    }

    /// 读一个包，返回 (包 ID, 负载)（状态机内分发用）。
    pub async fn read_any(&mut self, what: &'static str) -> Result<Option<(i32, BytesMut)>, ServerError> {
        match self.reader.read_packet().await? {
            None => Err(ServerError::ConnectionClosed { what }),
            Some(mut payload) => {
                let id = read_packet_id(&mut payload)?;
                Ok(Some((id, payload)))
            }
        }
    }

    pub async fn send<T: qexed_packet::Packet>(&mut self, packet: &T) -> Result<(), ServerError> {
        use qexed_packet::PacketCodec as _;
        let mut payload = BytesMut::new();
        write_varint_buf(&mut payload, T::ID);
        {
            let mut w = qexed_packet::PacketWriter::new(&mut payload);
            packet.serialize(&mut w)?;
        }
        self.writer.send_raw(payload).await
    }
}

pub fn read_packet_id(payload: &mut BytesMut) -> Result<i32, ServerError> {
    let mut reader = qexed_packet::PacketReader::new(payload);
    use qexed_packet::PacketCodec as _;
    let mut id = qexed_packet::net_types::VarInt::default();
    id.deserialize(&mut reader)?;
    Ok(id.0)
}

pub fn decode_payload<T: qexed_packet::Packet>(payload: &mut BytesMut) -> Result<T, ServerError> {
    let mut reader = qexed_packet::PacketReader::new(payload);
    use qexed_packet::PacketCodec as _;
    let mut packet = T::default();
    packet.deserialize(&mut reader)?;
    Ok(packet)
}