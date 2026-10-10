use bytes::{Buf, BytesMut};
use flate2::read::ZlibDecoder;
use openssl::symm::{Cipher, Crypter, Mode};
use std::collections::VecDeque;
use std::io::{Cursor, Read};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, ReadBuf};
use tokio_stream::Stream;

use crate::error::{PacketReadVarIntParseError, TcpConnectError};

const READ_BUFFER_SIZE: usize = 4096;
const MAX_PACKET_SIZE: usize = 8 * 1024 * 1024;

pub struct PacketStream<R> {
    reader: R,
    buffer: BytesMut,
    pending: VecDeque<BytesMut>,
    compression_threshold: Option<i32>,
    decrypter: Option<Crypter>,
    max_packet_size: usize,
}

impl<R: AsyncRead + Unpin> PacketStream<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            buffer: BytesMut::with_capacity(READ_BUFFER_SIZE),
            pending: VecDeque::new(),
            compression_threshold: None,
            decrypter: None,
            max_packet_size: MAX_PACKET_SIZE,
        }
    }

    pub fn set_compression_threshold(&mut self, threshold: i32) {
        self.compression_threshold = (threshold >= 0).then_some(threshold);
    }

    pub fn set_compression(&mut self, enabled: bool) {
        self.compression_threshold = enabled.then_some(0);
    }

    pub fn disable_compression(&mut self) {
        self.compression_threshold = None;
    }

    pub fn is_compression_enabled(&self) -> bool {
        self.compression_threshold.is_some()
    }

    pub fn enable_encryption(&mut self, shared_secret: &[u8]) -> Result<(), TcpConnectError> {
        if shared_secret.len() != 16 {
            return Err(TcpConnectError::InvalidEncryptionKeyLength {
                length: shared_secret.len(),
            });
        }

        let mut decrypter = Crypter::new(
            Cipher::aes_128_cfb8(),
            Mode::Decrypt,
            shared_secret,
            Some(shared_secret),
        )
        .map_err(|err| TcpConnectError::CryptoError(err.to_string()))?;
        decrypter.pad(false);
        self.decrypter = Some(decrypter);
        Ok(())
    }

    pub fn set_encryption(&mut self, shared_secret: &[u8]) -> Result<(), TcpConnectError> {
        self.enable_encryption(shared_secret)
    }

    pub fn disable_encryption(&mut self) {
        self.decrypter = None;
    }

    pub fn is_encryption_enabled(&self) -> bool {
        self.decrypter.is_some()
    }

    pub fn set_max_packet_size(&mut self, max_packet_size: usize) {
        self.max_packet_size = max_packet_size;
    }

    pub async fn read_packet(&mut self) -> Result<Option<BytesMut>, TcpConnectError> {
        use tokio::io::AsyncReadExt;

        loop {
            if let Some(packet) = self.pending.pop_front() {
                return Ok(Some(packet));
            }

            if let Some(packet) = self.try_parse_packet()? {
                return Ok(Some(packet));
            }

            let mut tmp = [0_u8; READ_BUFFER_SIZE];
            let n = self.reader.read(&mut tmp).await?;

            if n == 0 {
                if self.buffer.is_empty() {
                    return Ok(None);
                }

                return Err(TcpConnectError::ConnectionClosedWithIncompletePacket);
            }

            self.push_read_bytes(&tmp[..n])?;
        }
    }

    fn push_read_bytes(&mut self, bytes: &[u8]) -> Result<(), TcpConnectError> {
        if let Some(decrypter) = self.decrypter.as_mut() {
            let mut decrypted = vec![0_u8; bytes.len()];
            let len = decrypter
                .update(bytes, &mut decrypted)
                .map_err(|err| TcpConnectError::CryptoError(err.to_string()))?;
            self.buffer.extend_from_slice(&decrypted[..len]);
        } else {
            self.buffer.extend_from_slice(bytes);
        }

        Ok(())
    }

    fn try_parse_packet(&mut self) -> Result<Option<BytesMut>, TcpConnectError> {
        let Some((packet_len, header_len)) = read_varint_prefix(&self.buffer)? else {
            return Ok(None);
        };

        if packet_len < 0 {
            return Err(TcpConnectError::InvalidPacketLength(packet_len));
        }

        let packet_len = packet_len as usize;
        if packet_len > self.max_packet_size {
            return Err(TcpConnectError::PacketTooLarge {
                size: packet_len,
                max: self.max_packet_size,
            });
        }

        let frame_len = header_len + packet_len;
        if self.buffer.len() < frame_len {
            return Ok(None);
        }

        let mut frame = self.buffer.split_to(frame_len);
        frame.advance(header_len);
        self.decode_frame(frame).map(Some)
    }

    fn decode_frame(&self, frame: BytesMut) -> Result<BytesMut, TcpConnectError> {
        let Some(threshold) = self.compression_threshold else {
            return Ok(frame);
        };

        let mut cursor = Cursor::new(frame.as_ref());
        let data_len = read_varint(&mut cursor)?;
        let header_len = cursor.position() as usize;

        if data_len < 0 {
            return Err(TcpConnectError::InvalidCompressedDataLength(data_len));
        }

        if data_len == 0 {
            return Ok(BytesMut::from(&frame[header_len..]));
        }

        if data_len < threshold {
            return Err(TcpConnectError::CompressedDataLengthTooSmall {
                size: data_len,
                threshold,
            });
        }

        let expected_len = data_len as usize;
        if expected_len > self.max_packet_size {
            return Err(TcpConnectError::PacketTooLarge {
                size: expected_len,
                max: self.max_packet_size,
            });
        }

        let mut decoder = ZlibDecoder::new(&frame[header_len..]).take(expected_len as u64 + 1);
        let mut decompressed = Vec::with_capacity(expected_len);
        decoder
            .read_to_end(&mut decompressed)
            .map_err(TcpConnectError::DecompressionError)?;

        if decompressed.len() != expected_len {
            return Err(TcpConnectError::DecompressionSizeMismatch {
                expected: expected_len,
                actual: decompressed.len(),
            });
        }

        Ok(BytesMut::from(decompressed.as_slice()))
    }
}

impl<R: AsyncRead + Unpin> Stream for PacketStream<R> {
    type Item = Result<BytesMut, TcpConnectError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            if let Some(packet) = self.pending.pop_front() {
                return Poll::Ready(Some(Ok(packet)));
            }

            match self.try_parse_packet() {
                Ok(Some(packet)) => return Poll::Ready(Some(Ok(packet))),
                Ok(None) => {}
                Err(err) => return Poll::Ready(Some(Err(err))),
            }

            let mut tmp = [0_u8; READ_BUFFER_SIZE];
            let mut read_buf = ReadBuf::new(&mut tmp);
            match Pin::new(&mut self.reader).poll_read(cx, &mut read_buf) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Ok(())) => {
                    let n = read_buf.filled().len();
                    if n == 0 {
                        if self.buffer.is_empty() {
                            return Poll::Ready(None);
                        }

                        return Poll::Ready(Some(Err(
                            TcpConnectError::ConnectionClosedWithIncompletePacket,
                        )));
                    }

                    if let Err(err) = self.push_read_bytes(&tmp[..n]) {
                        return Poll::Ready(Some(Err(err)));
                    }
                }
                Poll::Ready(Err(err)) => {
                    return Poll::Ready(Some(Err(TcpConnectError::Io(err))));
                }
            }
        }
    }
}

// ---------------- varint helpers ----------------

pub(crate) fn read_varint<R: Read>(reader: &mut R) -> Result<i32, PacketReadVarIntParseError> {
    let mut value = 0_i32;

    for position in 0..5 {
        let mut byte = [0_u8; 1];
        let read = reader
            .read(&mut byte)
            .map_err(PacketReadVarIntParseError::ReadError)?;

        if read == 0 {
            return Err(PacketReadVarIntParseError::IncompleteError);
        }

        value |= ((byte[0] & 0x7f) as i32) << (7 * position);
        if (byte[0] & 0x80) == 0 {
            return Ok(value);
        }
    }

    Err(PacketReadVarIntParseError::TooLargeError)
}

pub(crate) fn read_varint_prefix(bytes: &[u8]) -> Result<Option<(i32, usize)>, TcpConnectError> {
    let mut value = 0_i32;

    for (position, byte) in bytes.iter().take(5).enumerate() {
        value |= ((*byte & 0x7f) as i32) << (7 * position);
        if (*byte & 0x80) == 0 {
            return Ok(Some((value, position + 1)));
        }
    }

    if bytes.len() < 5 {
        Ok(None)
    } else {
        Err(TcpConnectError::PacketReadVarIntParseError(
            PacketReadVarIntParseError::TooLargeError,
        ))
    }
}

pub(crate) fn write_varint(value: i32, buf: &mut BytesMut) {
    let mut value = value as u32;

    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        buf.extend_from_slice(&[byte]);

        if value == 0 {
            break;
        }
    }
}

pub(crate) fn varint_len(value: i32) -> usize {
    let mut value = value as u32;
    let mut len = 1;

    while value >= 0x80 {
        value >>= 7;
        len += 1;
    }

    len
}