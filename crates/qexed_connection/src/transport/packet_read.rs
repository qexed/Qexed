//! Minecraft TCP 帧读取流（v4 qexed_tcp_connect::packet_read 迁移）.
//!
//! 差异：v4 用 openssl Crypter 做 AES-128-CFB8 解密，v6 workspace 只有 RustCrypto
//! 的 aes + cfb8 crate，这里用 cfb8::AsyncStreamCipher 逐段解密；错误枚举改为
//! thiserror（v4 的 I18nErrorDisplay 不存在），语义变体一一对应。

use aes::Aes128;
use bytes::{Buf, BytesMut};
use cfb8::cipher::{AsyncStreamCipher as _, KeyIvInit as _};
use cfb8::Decryptor;
use flate2::read::ZlibDecoder;
use std::collections::VecDeque;
use std::io::{Cursor, Read};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, ReadBuf};
use tokio_stream::Stream;

const READ_BUFFER_SIZE: usize = 4096;
const MAX_PACKET_SIZE: usize = 8 * 1024 * 1024;

type AesCfb8 = Decryptor<Aes128>;

pub struct PacketStream<R> {
    reader: R,
    buffer: BytesMut,
    pending: VecDeque<BytesMut>,
    compression_threshold: Option<i32>,
    decrypter: Option<AesCfb8>,
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
            return Err(PacketReadError::InvalidEncryptionKeyLength {
                length: shared_secret.len(),
            });
        }

        let key: [u8; 16] = shared_secret
            .try_into()
            .expect("length checked above");
        let iv = key; // Minecraft 协议：CFB8 IV 与密钥相同
        let mut decrypter = AesCfb8::new(&key.into(), &iv.into());
        // 关键修复：客户端常把 Key 包与后续加密包放在同一 TCP 段发出。
        // 启用解密时必须立即解密 buffer 里已存在的密文残留，
        // 否则残留密文会被当明文帧解析（表现为后续包 id 乱码/长度爆炸）。
        if !self.buffer.is_empty() {
            let mut pending_plain = std::mem::take(&mut self.buffer);
            decrypter.decrypt(&mut pending_plain);
            self.buffer = pending_plain;
        }
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

    pub fn into_inner(self) -> R {
        self.reader
    }

    /// 从任意读取半构造（组装层 boxed 流用）。
    pub fn from_reader(reader: R) -> Self {
        Self::new(reader)
    }

    pub async fn read_packet(&mut self) -> Result<Option<BytesMut>, PacketReadError> {
        use tokio::io::AsyncReadExt;

        loop {
            if let Some(packet) = self.pending.pop_front() {
                return Ok(Some(packet));
            }

            if let Some(packet) = self.try_parse_packet()? {
                return Ok(Some(packet));
            }

            let mut tmp = [0_u8; READ_BUFFER_SIZE];
            let n = self
                .reader
                .read(&mut tmp)
                .await
                .map_err(|err| PacketReadError::OtherError { err })?;

            if n == 0 {
                if self.buffer.is_empty() {
                    return Ok(None);
                }

                return Err(PacketReadError::ConnectionClosedWithIncompletePacket);
            }

                        self.push_read_bytes(&tmp[..n]);
        }
    }

    fn push_read_bytes(&mut self, bytes: &[u8]) {
        if let Some(decrypter) = self.decrypter.clone() {
            let mut decrypted = bytes.to_vec();
            decrypter.decrypt(&mut decrypted);
            self.buffer.extend_from_slice(&decrypted);
            if log::log_enabled!(log::Level::Info) && decrypted.len() < 100 {
                log::info!("[diag-rx] {}B dec hex={}", decrypted.len(), decrypted.iter().map(|b| format!("{b:02x}")).collect::<String>());
            }
        } else {
            self.buffer.extend_from_slice(bytes);
        }
    }

    fn try_parse_packet(&mut self) -> Result<Option<BytesMut>, PacketReadError> {
        let Some((packet_len, header_len)) = read_varint_prefix(&self.buffer)? else {
            return Ok(None);
        };

        if packet_len < 0 {
            return Err(PacketReadError::InvalidPacketLength { length: packet_len });
        }

        let packet_len = packet_len as usize;
        if packet_len > self.max_packet_size {
            return Err(PacketReadError::PacketTooLarge {
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

    fn decode_frame(&self, frame: BytesMut) -> Result<BytesMut, PacketReadError> {
        let Some(threshold) = self.compression_threshold else {
            return Ok(frame);
        };

        let mut cursor = Cursor::new(frame.as_ref());
        let data_len =
            read_varint(&mut cursor).map_err(|err| PacketReadError::PacketReadVarIntParseError { err })?;
        let header_len = cursor.position() as usize;

        if data_len < 0 {
            return Err(PacketReadError::InvalidCompressedDataLength { length: data_len });
        }

        if data_len == 0 {
            return Ok(BytesMut::from(&frame[header_len..]));
        }

        // MC 协议接收端规则：data_length > 0 即解压，阈值只影响发送侧决策。
        // （v4 沿袭的"小于阈值拒绝"违反协议，导致拒收真压缩小包。）
        let _ = threshold;

        let expected_len = data_len as usize;
        if expected_len > self.max_packet_size {
            return Err(PacketReadError::PacketTooLarge {
                size: expected_len,
                max: self.max_packet_size,
            });
        }

        let mut decoder = ZlibDecoder::new(&frame[header_len..]).take(expected_len as u64 + 1);
        let mut decompressed = Vec::with_capacity(expected_len);
        decoder
            .read_to_end(&mut decompressed)
            .map_err(|err| PacketReadError::DecompressionError { err })?;

        if decompressed.len() != expected_len {
            return Err(PacketReadError::DecompressionSizeMismatch {
                expected: expected_len,
                actual: decompressed.len(),
            });
        }

        Ok(BytesMut::from(decompressed.as_slice()))
    }
}


/// 传输层读取错误（v4 PacketReadError 的 thiserror 版）。
#[derive(Debug, thiserror::Error)]
pub enum PacketReadError {
    #[error("{}", t("qexed.connection.packet_read.packet_read_varint_error").replace("%{error}", &err.to_string()))]
    PacketReadVarIntParseError { #[from] err: PacketReadVarIntParseError },
    #[error("{}", t("qexed.connection.packet_read.connection_closed_with_incomplete_packet"))]
    ConnectionClosedWithIncompletePacket,
    #[error("{}", t("qexed.connection.packet_read.io_error").replace("%{error}", &err.to_string()))]
    OtherError { #[from] err: std::io::Error },
    #[error("{}", t("qexed.connection.packet_read.invalid_packet_length").replace("%{length}", &length.to_string()))]
    InvalidPacketLength { length: i32 },
    #[error("{}", t("qexed.connection.packet_read.packet_too_large").replace("%{size}", &size.to_string()).replace("%{max}", &max.to_string()))]
    PacketTooLarge { size: usize, max: usize },
    #[error("{}", t("qexed.connection.packet_read.invalid_compressed_data_length").replace("%{length}", &length.to_string()))]
    InvalidCompressedDataLength { length: i32 },
    #[error("{}", t("qexed.connection.packet_read.compressed_data_length_too_small").replace("%{size}", &size.to_string()).replace("%{threshold}", &threshold.to_string()))]
    CompressedDataLengthTooSmall { size: i32, threshold: i32 },
    #[error("{}", t("qexed.connection.packet_read.decompression_error").replace("%{error}", &err.to_string()))]
    DecompressionError { #[source] err: std::io::Error },
    #[error("{}", t("qexed.connection.packet_read.decompression_size_mismatch").replace("%{expected}", &expected.to_string()).replace("%{actual}", &actual.to_string()))]
    DecompressionSizeMismatch { expected: usize, actual: usize },
    #[error("{}", t("qexed.connection.packet_read.invalid_encryption_key_length").replace("%{length}", &length.to_string()))]
    InvalidEncryptionKeyLength { length: usize },
}

fn t(key: &str) -> String {
    qexed_language::t(key)
}

/// VarInt 解析错误（v4 同名枚举）。
#[derive(Debug, thiserror::Error)]
pub enum PacketReadVarIntParseError {
    #[error("{}", t("qexed.connection.packet_read_varint.incomplete"))]
    IncompleteError,
    #[error("{}", t("qexed.connection.packet_read_varint.too_large"))]
    TooLargeError,
    #[error("{}", t("qexed.connection.packet_read_varint.read_error").replace("%{error}", &err.to_string()))]
    ReadError { #[from] err: std::io::Error },
}

pub(crate) fn read_varint<R: Read>(reader: &mut R) -> Result<i32, PacketReadVarIntParseError> {
    let mut value = 0_i32;

    for position in 0..5 {
        let mut byte = [0_u8; 1];
        let read = reader
            .read(&mut byte)
            .map_err(|err| PacketReadVarIntParseError::ReadError { err })?;

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
        Err(PacketReadError::PacketReadVarIntParseError {
            err: PacketReadVarIntParseError::TooLargeError,
        })
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

                    self.push_read_bytes(&tmp[..n]);
                }
                Poll::Ready(Err(err)) => {
                    return Poll::Ready(Some(Err(PacketReadError::OtherError { err })));
                }
            }
        }
    }
}
