//! Minecraft TCP 帧写出（v4 qexed_tcp_connect::packet_write 迁移）。
//!
//! 差异：v4 用 openssl Crypter 加密 + anyhow 错误，v6 用 RustCrypto cfb8
//! 加密 + thiserror（PacketEncodeError 由 qexed_packet::PacketError 取代）。

use aes::Aes128;
use bytes::{Bytes, BytesMut};
use cfb8::cipher::{AsyncStreamCipher as _, KeyIvInit as _};
use cfb8::Encryptor;
use flate2::Compression;
use flate2::write::ZlibEncoder;
use qexed_packet::{Packet, PacketCodec as _};
use std::io::Write;
use std::ops::{Deref, DerefMut};
use tokio::io::{AsyncWrite, AsyncWriteExt, WriteHalf};
use tokio::net::TcpStream;

use super::packet_read::{varint_len, write_varint};

const MAX_PACKET_SIZE: usize = 8 * 1024 * 1024;

type AesCfb8 = Encryptor<Aes128>;

pub struct PacketSink<W> {
    writer: W,
    compression_threshold: Option<i32>,
    encrypter: Option<AesCfb8>,
    max_packet_size: usize,
}

pub struct PacketSend {
    inner: PacketSink<WriteHalf<TcpStream>>,
    compression_threshold: i32,
}

impl PacketSink<tokio::io::Sink> {
    pub fn encode_payload_frame_with_threshold<B: AsRef<[u8]>>(
        payload: B,
        compression_threshold: Option<i32>,
    ) -> Result<Bytes, PacketWriteError> {
        let mut sink = PacketSink::new(tokio::io::sink());
        if let Some(threshold) = compression_threshold {
            sink.set_compression_threshold(threshold);
        }
        sink.encode_payload_frame(payload)
    }
}

impl<W: AsyncWrite + Unpin> PacketSink<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            compression_threshold: None,
            encrypter: None,
            max_packet_size: MAX_PACKET_SIZE,
        }
    }

    pub fn with_compression_threshold(writer: W, threshold: i32) -> Self {
        let mut sink = Self::new(writer);
        sink.set_compression_threshold(threshold);
        sink
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

    pub fn compression_threshold(&self) -> Option<i32> {
        self.compression_threshold
    }

    pub fn enable_encryption(&mut self, shared_secret: &[u8]) -> Result<(), PacketWriteError> {
        if shared_secret.len() != 16 {
            return Err(PacketWriteError::InvalidEncryptionKeyLength {
                length: shared_secret.len(),
            });
        }

        let key: [u8; 16] = shared_secret
            .try_into()
            .expect("length checked above");
        let iv = key; // Minecraft 协议：CFB8 IV 与密钥相同
        self.encrypter = Some(AesCfb8::new(&key.into(), &iv.into()));
        Ok(())
    }

    pub fn set_encryption(&mut self, shared_secret: &[u8]) -> Result<(), PacketWriteError> {
        self.enable_encryption(shared_secret)
    }

    pub fn disable_encryption(&mut self) {
        self.encrypter = None;
    }

    pub fn is_encryption_enabled(&self) -> bool {
        self.encrypter.is_some()
    }

    pub fn set_max_packet_size(&mut self, max_packet_size: usize) {
        self.max_packet_size = max_packet_size;
    }

    pub async fn send<T: Packet>(&mut self, packet: T) -> Result<(), PacketWriteError> {
        self.send_raw(Self::build_send_packet(packet)?).await
    }

    pub fn build_send_packet<T: Packet>(packet: T) -> Result<Bytes, PacketWriteError> {
        let mut buf = BytesMut::new();
        let mut writer = qexed_packet::PacketWriter::new(&mut buf);
        qexed_packet::net_types::VarInt(T::ID).serialize(&mut writer)?;
        packet.serialize(&mut writer)?;
        Ok(buf.freeze())
    }

    pub async fn send_raw<B: AsRef<[u8]>>(&mut self, payload: B) -> Result<(), PacketWriteError> {
        let frame = self.encode_frame(payload.as_ref())?;
        self.send_encoded_frame(frame).await
    }

    pub async fn send_encoded_frame<B: AsRef<[u8]>>(&mut self, frame: B) -> Result<(), PacketWriteError> {
        let frame = BytesMut::from(frame.as_ref());
        let frame = self.encrypt_frame(&frame);
        self.writer
            .write_all(&frame)
            .await
            .map_err(|err| PacketWriteError::OtherError { err })?;
        Ok(())
    }

    pub fn encode_payload_frame<B: AsRef<[u8]>>(&self, payload: B) -> Result<Bytes, PacketWriteError> {
        Ok(self.encode_frame(payload.as_ref())?.freeze())
    }

    pub async fn flush(&mut self) -> Result<(), PacketWriteError> {
        self.writer
            .flush()
            .await
            .map_err(|err| PacketWriteError::OtherError { err })
    }

    pub async fn shutdown(&mut self) -> Result<(), PacketWriteError> {
        self.writer
            .shutdown()
            .await
            .map_err(|err| PacketWriteError::OtherError { err })
    }

    pub fn get_ref(&self) -> &W {
        &self.writer
    }

    pub fn get_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    pub fn into_inner(self) -> W {
        self.writer
    }

    fn encode_frame(&self, payload: &[u8]) -> Result<BytesMut, PacketWriteError> {
        if payload.len() > self.max_packet_size {
            return Err(PacketWriteError::PacketTooLarge {
                size: payload.len(),
                max: self.max_packet_size,
            });
        }

        let mut body = BytesMut::new();

        match self.compression_threshold {
            Some(threshold) => self.write_compressed_body(payload, threshold, &mut body)?,
            None => body.extend_from_slice(payload),
        }

        if body.len() > self.max_packet_size {
            return Err(PacketWriteError::PacketTooLarge {
                size: body.len(),
                max: self.max_packet_size,
            });
        }

        let mut frame = BytesMut::with_capacity(varint_len(body.len() as i32) + body.len());
        write_varint(body.len() as i32, &mut frame);
        frame.extend_from_slice(&body);
        Ok(frame)
    }

    fn write_compressed_body(
        &self,
        payload: &[u8],
        threshold: i32,
        body: &mut BytesMut,
    ) -> Result<(), PacketWriteError> {
        if threshold >= 0 && payload.len() >= threshold as usize {
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
            encoder
                .write_all(payload)
                .map_err(|err| PacketWriteError::CompressionError { err })?;
            let compressed = encoder
                .finish()
                .map_err(|err| PacketWriteError::CompressionError { err })?;

            write_varint(payload.len() as i32, body);
            body.extend_from_slice(&compressed);
        } else {
            write_varint(0, body);
            body.extend_from_slice(payload);
        }

        Ok(())
    }

    fn encrypt_frame(&mut self, frame: &BytesMut) -> BytesMut {
        let Some(encrypter) = self.encrypter.clone() else {
            return frame.clone();
        };

        let mut encrypted = frame.to_vec();
        encrypter.encrypt(&mut encrypted);
        BytesMut::from(encrypted.as_slice())
    }
}

impl PacketSend {
    pub fn new(socket_write: WriteHalf<TcpStream>, compression_threshold: isize) -> Self {
        Self {
            inner: PacketSink::new(socket_write),
            compression_threshold: normalize_threshold(compression_threshold),
        }
    }

    pub async fn send<T: Packet>(&mut self, packet: T) -> Result<(), PacketWriteError> {
        self.inner.send(packet).await
    }

    pub async fn build_send_packet<T: Packet>(
        packet: T,
    ) -> Result<Bytes, PacketWriteError> {
        PacketSink::<WriteHalf<TcpStream>>::build_send_packet(packet)
    }

    pub async fn send_raw<B: AsRef<[u8]>>(&mut self, payload: B) -> Result<(), PacketWriteError> {
        self.inner.send_raw(payload).await
    }

    pub fn set_compression(&mut self, enabled: bool) {
        if enabled {
            self.inner
                .set_compression_threshold(self.compression_threshold);
        } else {
            self.inner.disable_compression();
        }
    }

    pub fn set_compression_threshold(&mut self, compression_threshold: isize) {
        self.compression_threshold = normalize_threshold(compression_threshold);
        if self.inner.is_compression_enabled() {
            self.inner
                .set_compression_threshold(self.compression_threshold);
        }
    }

    pub fn enable_encryption(&mut self, shared_secret: &[u8]) -> Result<(), PacketWriteError> {
        self.inner.enable_encryption(shared_secret)
    }

    pub fn set_encryption(&mut self, shared_secret: &[u8]) -> Result<(), PacketWriteError> {
        self.enable_encryption(shared_secret)
    }

    pub fn disable_encryption(&mut self) {
        self.inner.disable_encryption();
    }

    pub fn is_encryption_enabled(&self) -> bool {
        self.inner.is_encryption_enabled()
    }

    pub fn is_compression_enabled(&self) -> bool {
        self.inner.is_compression_enabled()
    }

    pub async fn flush(&mut self) -> Result<(), PacketWriteError> {
        self.inner.flush().await
    }

    pub async fn shutdown(&mut self) -> Result<(), PacketWriteError> {
        self.inner.shutdown().await
    }

    pub fn get_ref(&self) -> &WriteHalf<TcpStream> {
        self.inner.get_ref()
    }

    pub fn get_mut(&mut self) -> &mut WriteHalf<TcpStream> {
        self.inner.get_mut()
    }

    pub fn into_inner(self) -> WriteHalf<TcpStream> {
        self.inner.into_inner()
    }
}

impl Deref for PacketSend {
    type Target = PacketSink<WriteHalf<TcpStream>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for PacketSend {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// 传输层写出错误（v4 PacketWriteError 的 thiserror 版，去掉 anyhow 变体）。
#[derive(Debug, thiserror::Error)]
pub enum PacketWriteError {
    #[error("{}", t("qexed.connection.packet_write.io_error").replace("%{error}", &err.to_string()))]
    OtherError { #[from] err: std::io::Error },
    #[error("{}", t("qexed.connection.packet_write.packet_too_large").replace("%{size}", &size.to_string()).replace("%{max}", &max.to_string()))]
    PacketTooLarge { size: usize, max: usize },
    #[error("{}", t("qexed.connection.packet_write.compression_error").replace("%{error}", &err.to_string()))]
    CompressionError { #[source] err: std::io::Error },
    #[error("{}", t("qexed.connection.packet_write.invalid_encryption_key_length").replace("%{length}", &length.to_string()))]
    InvalidEncryptionKeyLength { length: usize },
    /// v4 的 PacketEncodeError(anyhow::Error) 改为承载 qexed_packet::PacketError。
    #[error("{}", t("qexed.connection.packet_write.packet_encode_error").replace("%{error}", &err.to_string()))]
    PacketEncodeError { #[from] err: qexed_packet::PacketError },
}

fn t(key: &str) -> String {
    qexed_language::t(key)
}

fn normalize_threshold(compression_threshold: isize) -> i32 {
    compression_threshold
        .try_into()
        .unwrap_or(if compression_threshold < 0 {
            -1
        } else {
            i32::MAX
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;

    #[tokio::test]
    async fn roundtrip_plain() {
        let (client, server) = duplex(64 * 1024);
        let mut writer = PacketSink::new(client);
        let mut reader = crate::transport::PacketStream::new(server);
        writer.send_raw(b"\x01hello").await.unwrap();
        writer.shutdown().await.unwrap();
        let packet = reader.read_packet().await.unwrap().unwrap();
        assert_eq!(&packet[..], b"\x01hello");
    }
}
