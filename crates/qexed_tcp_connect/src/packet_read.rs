use bytes::{Buf, BufMut as _, BytesMut};
use flate2::read::ZlibDecoder;
use openssl::symm::{Cipher, Crypter, Mode};
use std::collections::VecDeque;
use std::io::{Cursor, Read};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, ReadBuf};
use tokio_stream::Stream;

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

    pub fn enable_encryption(&mut self, shared_secret: &[u8]) -> Result<(), PacketReadError> {
        if shared_secret.len() != 16 {
            return Err(PacketReadError::InvalidEncryptionKeyLength(
                shared_secret.len(),
            ));
        }

        let mut decrypter = Crypter::new(
            Cipher::aes_128_cfb8(),
            Mode::Decrypt,
            shared_secret,
            Some(shared_secret),
        )
        .map_err(|err| PacketReadError::CryptoError(err.to_string()))?;
        decrypter.pad(false);
        self.decrypter = Some(decrypter);
        Ok(())
    }

    pub fn set_encryption(&mut self, shared_secret: &[u8]) -> Result<(), PacketReadError> {
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

    pub async fn read_packet(&mut self) -> Result<Option<BytesMut>, PacketReadError> {
        loop {
            if let Some(packet) = self.pending.pop_front() {
                return Ok(Some(packet));
            }

            if let Some(packet) = self.try_parse_packet()? {
                return Ok(Some(packet));
            }

            let n = self.read_more().await?;

            if n == 0 {
                if self.buffer.is_empty() {
                    return Ok(None);
                }

                return Err(PacketReadError::ConnectionClosedWithIncompletePacket);
            }
        }
    }

    async fn read_more(&mut self) -> Result<usize, PacketReadError> {
        use tokio::io::AsyncReadExt;

        if self.decrypter.is_none() {
            self.buffer.reserve(READ_BUFFER_SIZE);
            return self
                .reader
                .read_buf(&mut self.buffer)
                .await
                .map_err(PacketReadError::OtherError);
        }

        let mut tmp = [0_u8; READ_BUFFER_SIZE];
        let n = self
            .reader
            .read(&mut tmp)
            .await
            .map_err(PacketReadError::OtherError)?;
        self.push_read_bytes(&tmp[..n])?;
        Ok(n)
    }

    fn push_read_bytes(&mut self, bytes: &[u8]) -> Result<(), PacketReadError> {
        if let Some(decrypter) = self.decrypter.as_mut() {
            let old_len = self.buffer.len();
            self.buffer.resize(old_len + bytes.len(), 0);
            let output = &mut self.buffer[old_len..];
            let len = match decrypter.update(bytes, output) {
                Ok(len) => len,
                Err(err) => {
                    self.buffer.truncate(old_len);
                    return Err(PacketReadError::CryptoError(err.to_string()));
                }
            };
            self.buffer.truncate(old_len + len);
        } else {
            self.buffer.extend_from_slice(bytes);
        }

        Ok(())
    }

    fn try_parse_packet(&mut self) -> Result<Option<BytesMut>, PacketReadError> {
        let Some((packet_len, header_len)) = read_varint_prefix(&self.buffer)? else {
            return Ok(None);
        };

        if packet_len < 0 {
            return Err(PacketReadError::InvalidPacketLength(packet_len));
        }

        let packet_len = packet_len as usize;
        if packet_len > self.max_packet_size {
            return Err(PacketReadError::PacketTooLarge(
                packet_len,
                self.max_packet_size,
            ));
        }

        let frame_len = header_len + packet_len;
        if self.buffer.len() < frame_len {
            return Ok(None);
        }

        let mut frame = self.buffer.split_to(frame_len);
        frame.advance(header_len);
        self.decode_frame(frame).map(Some)
    }

    fn decode_frame(&self, mut frame: BytesMut) -> Result<BytesMut, PacketReadError> {
        let Some(threshold) = self.compression_threshold else {
            return Ok(frame);
        };

        let mut cursor = Cursor::new(frame.as_ref());
        let data_len =
            read_varint(&mut cursor).map_err(PacketReadError::PacketReadVarIntParseError)?;
        let header_len = cursor.position() as usize;

        if data_len < 0 {
            return Err(PacketReadError::InvalidCompressedDataLength(data_len));
        }

        if data_len == 0 {
            frame.advance(header_len);
            return Ok(frame);
        }

        if data_len < threshold {
            return Err(PacketReadError::CompressedDataLengthTooSmall(
                data_len, threshold,
            ));
        }

        let expected_len = data_len as usize;
        if expected_len > self.max_packet_size {
            return Err(PacketReadError::PacketTooLarge(
                expected_len,
                self.max_packet_size,
            ));
        }

        let mut decoder = ZlibDecoder::new(&frame[header_len..]).take(expected_len as u64 + 1);
        let mut decompressed = BytesMut::with_capacity(expected_len);
        decompressed.resize(expected_len, 0);
        decoder
            .read_exact(decompressed.as_mut())
            .map_err(PacketReadError::DecompressionError)?;

        let mut extra = [0_u8; 1];
        if decoder
            .read(&mut extra)
            .map_err(PacketReadError::DecompressionError)?
            != 0
        {
            return Err(PacketReadError::DecompressionSizeMismatch(
                expected_len,
                expected_len + 1,
            ));
        }

        Ok(decompressed)
    }
}

impl<R: AsyncRead + Unpin> Stream for PacketStream<R> {
    type Item = Result<BytesMut, PacketReadError>;

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
                            PacketReadError::ConnectionClosedWithIncompletePacket,
                        )));
                    }

                    if let Err(err) = self.push_read_bytes(&tmp[..n]) {
                        return Poll::Ready(Some(Err(err)));
                    }
                }
                Poll::Ready(Err(err)) => {
                    return Poll::Ready(Some(Err(PacketReadError::OtherError(err))));
                }
            }
        }
    }
}

#[derive(Debug, qexed_error_macros::I18nErrorDisplay)]
pub enum PacketReadError {
    #[error("qexed_tcp_connect.packet_read.packet_read_varint_error", error = field_0.to_string())]
    PacketReadVarIntParseError(PacketReadVarIntParseError),
    #[error("qexed_tcp_connect.packet_read.connection_closed_with_incomplete_packet")]
    ConnectionClosedWithIncompletePacket,
    #[error("qexed_tcp_connect.packet_read.io_error", error = field_0.to_string())]
    OtherError(std::io::Error),
    #[error("qexed_tcp_connect.packet_read.invalid_packet_length", length = field_0)]
    InvalidPacketLength(i32),
    #[error("qexed_tcp_connect.packet_read.packet_too_large", size = field_0, max = field_1)]
    PacketTooLarge(usize, usize),
    #[error("qexed_tcp_connect.packet_read.invalid_compressed_data_length", length = field_0)]
    InvalidCompressedDataLength(i32),
    #[error("qexed_tcp_connect.packet_read.compressed_data_length_too_small", size = field_0, threshold = field_1)]
    CompressedDataLengthTooSmall(i32, i32),
    #[error("qexed_tcp_connect.packet_read.decompression_error", error = field_0.to_string())]
    DecompressionError(std::io::Error),
    #[error("qexed_tcp_connect.packet_read.decompression_size_mismatch", expected = field_0, actual = field_1)]
    DecompressionSizeMismatch(usize, usize),
    #[error("qexed_tcp_connect.packet_read.crypto_error", error = field_0)]
    CryptoError(String),
    #[error("qexed_tcp_connect.packet_read.invalid_encryption_key_length", length = field_0)]
    InvalidEncryptionKeyLength(usize),
}

impl std::error::Error for PacketReadError {}

#[derive(Debug, qexed_error_macros::I18nErrorDisplay)]
pub enum PacketReadVarIntParseError {
    #[error("qexed_tcp_connect.packet_read_varint.incomplete")]
    IncompleteError,
    #[error("qexed_tcp_connect.packet_read_varint.too_large")]
    TooLargeError,
    #[error("qexed_tcp_connect.packet_read_varint.read_error", error = field_0.to_string())]
    ReadError(std::io::Error),
}

impl std::error::Error for PacketReadVarIntParseError {}

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

pub(crate) fn read_varint_prefix(bytes: &[u8]) -> Result<Option<(i32, usize)>, PacketReadError> {
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
        Err(PacketReadError::PacketReadVarIntParseError(
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
        buf.put_u8(byte);

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
