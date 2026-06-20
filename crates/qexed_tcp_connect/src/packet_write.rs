use bytes::{Bytes, BytesMut};
use flate2::Compression;
use flate2::write::ZlibEncoder;
use openssl::symm::{Cipher, Crypter, Mode};
use qexed_packet::PacketCodec;
use std::future::poll_fn;
use std::io::{Error, ErrorKind, IoSlice, Write};
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use tokio::io::{AsyncWrite, AsyncWriteExt, WriteHalf};
use tokio::net::TcpStream;

use crate::packet_read::{varint_len, write_varint};

const MAX_PACKET_SIZE: usize = 8 * 1024 * 1024;
const MAX_IO_SLICES: usize = 64;

pub struct PacketSink<W> {
    writer: W,
    compression_threshold: Option<i32>,
    encrypter: Option<Crypter>,
    max_packet_size: usize,
}

pub struct PacketSend {
    inner: PacketSink<WriteHalf<TcpStream>>,
    compression_threshold: i32,
}

pub enum FramePart {
    Bytes(Bytes),
    Shared(&'static Bytes),
}

impl FramePart {
    fn as_slice(&self) -> &[u8] {
        match self {
            Self::Bytes(bytes) => bytes.as_ref(),
            Self::Shared(bytes) => bytes.as_ref(),
        }
    }
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
            return Err(PacketWriteError::InvalidEncryptionKeyLength(
                shared_secret.len(),
            ));
        }

        let mut encrypter = Crypter::new(
            Cipher::aes_128_cfb8(),
            Mode::Encrypt,
            shared_secret,
            Some(shared_secret),
        )
        .map_err(|err| PacketWriteError::CryptoError(err.to_string()))?;
        encrypter.pad(false);
        self.encrypter = Some(encrypter);
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

    pub fn max_packet_size(&self) -> usize {
        self.max_packet_size
    }

    pub async fn send<T: qexed_packet::Packet>(
        &mut self,
        packet: T,
    ) -> Result<(), PacketWriteError> {
        self.send_ref(&packet).await
    }

    pub async fn send_ref<T: qexed_packet::Packet>(
        &mut self,
        packet: &T,
    ) -> Result<(), PacketWriteError> {
        let mut frame = BytesMut::new();
        self.append_packet_frame_ref(packet, &mut frame)?;
        self.send_encoded_frame(frame).await
    }

    pub fn build_send_packet<T: qexed_packet::Packet>(
        packet: T,
    ) -> Result<Bytes, PacketWriteError> {
        Self::build_send_packet_ref(&packet)
    }

    pub fn build_send_packet_ref<T: qexed_packet::Packet>(
        packet: &T,
    ) -> Result<Bytes, PacketWriteError> {
        let mut buf = BytesMut::new();
        let mut writer = qexed_packet::PacketWriter::new(&mut buf);
        qexed_packet::net_types::VarInt(T::ID).serialize(&mut writer)?;
        packet.serialize(&mut writer)?;
        Ok(buf.freeze())
    }

    pub fn append_packet_frame_ref<T: qexed_packet::Packet>(
        &self,
        packet: &T,
        frames: &mut BytesMut,
    ) -> Result<(), PacketWriteError> {
        if self.compression_threshold.is_none() {
            return self.append_uncompressed_packet_frame_ref(packet, frames);
        }

        let payload = Self::build_send_packet_ref(packet)?;
        self.append_payload_frame(payload, frames)
    }

    pub fn append_payload_frame<B: AsRef<[u8]>>(
        &self,
        payload: B,
        frames: &mut BytesMut,
    ) -> Result<(), PacketWriteError> {
        let frame = self.encode_frame(payload.as_ref())?;
        frames.extend_from_slice(&frame);
        Ok(())
    }

    pub async fn send_raw<B: AsRef<[u8]>>(&mut self, payload: B) -> Result<(), PacketWriteError> {
        let frame = self.encode_frame(payload.as_ref())?;
        self.send_encoded_frame(frame).await
    }

    pub async fn send_encoded_frame<B: AsRef<[u8]>>(
        &mut self,
        frame: B,
    ) -> Result<(), PacketWriteError> {
        let frame = frame.as_ref();
        if self.encrypter.is_some() {
            let frame = self.encrypt_frame(frame)?;
            self.writer
                .write_all(&frame)
                .await
                .map_err(PacketWriteError::OtherError)?;
        } else {
            self.writer
                .write_all(frame)
                .await
                .map_err(PacketWriteError::OtherError)?;
        }
        Ok(())
    }

    pub async fn send_encoded_frames_vectored(
        &mut self,
        frames: &[FramePart],
    ) -> Result<(), PacketWriteError> {
        if self.encrypter.is_some() {
            let len = frames.iter().map(|frame| frame.as_slice().len()).sum();
            let mut joined = BytesMut::with_capacity(len);
            for frame in frames {
                joined.extend_from_slice(frame.as_slice());
            }
            let encrypted = self.encrypt_frame(&joined)?;
            self.writer
                .write_all(&encrypted)
                .await
                .map_err(PacketWriteError::OtherError)?;
        } else {
            write_all_vectored(&mut self.writer, frames).await?;
        }
        Ok(())
    }

    pub fn encode_payload_frame<B: AsRef<[u8]>>(
        &self,
        payload: B,
    ) -> Result<Bytes, PacketWriteError> {
        Ok(self.encode_frame(payload.as_ref())?.freeze())
    }

    pub async fn flush(&mut self) -> Result<(), PacketWriteError> {
        self.writer
            .flush()
            .await
            .map_err(PacketWriteError::OtherError)
    }

    pub async fn shutdown(&mut self) -> Result<(), PacketWriteError> {
        self.writer
            .shutdown()
            .await
            .map_err(PacketWriteError::OtherError)
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
            return Err(PacketWriteError::PacketTooLarge(
                payload.len(),
                self.max_packet_size,
            ));
        }

        if self.compression_threshold.is_none() {
            return self.encode_uncompressed_frame(payload);
        }

        let mut body = BytesMut::new();
        match self.compression_threshold {
            Some(threshold) => self.write_compressed_body(payload, threshold, &mut body)?,
            None => unreachable!("uncompressed path returns early"),
        }

        if body.len() > self.max_packet_size {
            return Err(PacketWriteError::PacketTooLarge(
                body.len(),
                self.max_packet_size,
            ));
        }

        let mut frame = BytesMut::with_capacity(varint_len(body.len() as i32) + body.len());
        write_varint(body.len() as i32, &mut frame);
        frame.extend_from_slice(&body);
        Ok(frame)
    }

    fn encode_uncompressed_frame(&self, payload: &[u8]) -> Result<BytesMut, PacketWriteError> {
        let mut frame = BytesMut::with_capacity(varint_len(payload.len() as i32) + payload.len());
        write_varint(payload.len() as i32, &mut frame);
        frame.extend_from_slice(payload);
        Ok(frame)
    }

    fn append_uncompressed_packet_frame_ref<T: qexed_packet::Packet>(
        &self,
        packet: &T,
        frames: &mut BytesMut,
    ) -> Result<(), PacketWriteError> {
        const MAX_VARINT_BYTES: usize = 5;

        let frame_start = frames.len();
        frames.extend_from_slice(&[0; MAX_VARINT_BYTES]);

        let encode_result = (|| {
            let mut writer = qexed_packet::PacketWriter::new(frames);
            qexed_packet::net_types::VarInt(T::ID).serialize(&mut writer)?;
            packet.serialize(&mut writer)?;
            Ok::<(), anyhow::Error>(())
        })();
        if let Err(err) = encode_result {
            frames.truncate(frame_start);
            return Err(err.into());
        }

        let payload_len = frames.len() - frame_start - MAX_VARINT_BYTES;
        if payload_len > self.max_packet_size {
            frames.truncate(frame_start);
            return Err(PacketWriteError::PacketTooLarge(
                payload_len,
                self.max_packet_size,
            ));
        }

        let header_len = varint_len(payload_len as i32);
        if header_len != MAX_VARINT_BYTES {
            frames.copy_within(frame_start + MAX_VARINT_BYTES.., frame_start + header_len);
            frames.truncate(frame_start + header_len + payload_len);
        }

        let mut header = BytesMut::with_capacity(header_len);
        write_varint(payload_len as i32, &mut header);
        frames[frame_start..frame_start + header_len].copy_from_slice(&header);
        Ok(())
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
                .map_err(PacketWriteError::CompressionError)?;
            let compressed = encoder
                .finish()
                .map_err(PacketWriteError::CompressionError)?;

            write_varint(payload.len() as i32, body);
            body.extend_from_slice(&compressed);
        } else {
            write_varint(0, body);
            body.extend_from_slice(payload);
        }

        Ok(())
    }

    fn encrypt_frame(&mut self, frame: &[u8]) -> Result<BytesMut, PacketWriteError> {
        let Some(encrypter) = self.encrypter.as_mut() else {
            return Ok(BytesMut::from(frame));
        };

        let mut encrypted = BytesMut::zeroed(frame.len());
        let len = encrypter
            .update(frame, encrypted.as_mut())
            .map_err(|err| PacketWriteError::CryptoError(err.to_string()))?;
        encrypted.truncate(len);
        Ok(encrypted)
    }
}

async fn write_all_vectored<W: AsyncWrite + Unpin>(
    writer: &mut W,
    frames: &[FramePart],
) -> Result<(), PacketWriteError> {
    let mut frame_index = 0;
    let mut frame_offset = 0;

    while frame_index < frames.len() {
        while frame_index < frames.len() && frame_offset == frames[frame_index].as_slice().len() {
            frame_index += 1;
            frame_offset = 0;
        }

        if frame_index == frames.len() {
            return Ok(());
        }

        let slices: Vec<_> = frames[frame_index..]
            .iter()
            .enumerate()
            .take(MAX_IO_SLICES)
            .filter_map(|(index, frame)| {
                let bytes = frame.as_slice();
                let offset = if index == 0 { frame_offset } else { 0 };
                (offset < bytes.len()).then(|| IoSlice::new(&bytes[offset..]))
            })
            .collect();

        let written = poll_fn(|cx| Pin::new(&mut *writer).poll_write_vectored(cx, &slices))
            .await
            .map_err(PacketWriteError::OtherError)?;
        if written == 0 {
            return Err(PacketWriteError::OtherError(Error::from(
                ErrorKind::WriteZero,
            )));
        }

        let mut remaining = written;
        while frame_index < frames.len() {
            let frame_remaining = frames[frame_index].as_slice().len() - frame_offset;
            if remaining < frame_remaining {
                frame_offset += remaining;
                break;
            }

            remaining -= frame_remaining;
            frame_index += 1;
            frame_offset = 0;
            if remaining == 0 {
                break;
            }
        }
    }

    Ok(())
}

impl PacketSend {
    pub fn new(socket_write: WriteHalf<TcpStream>, compression_threshold: isize) -> Self {
        Self {
            inner: PacketSink::new(socket_write),
            compression_threshold: normalize_threshold(compression_threshold),
        }
    }

    pub async fn send<T: qexed_packet::Packet>(
        &mut self,
        packet: T,
    ) -> Result<(), PacketWriteError> {
        self.inner.send(packet).await
    }

    pub async fn build_send_packet<T: qexed_packet::Packet>(
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

#[derive(Debug, qexed_error_macros::I18nErrorDisplay)]
pub enum PacketWriteError {
    #[error("qexed_tcp_connect.packet_write.io_error", error = field_0.to_string())]
    OtherError(std::io::Error),
    #[error("qexed_tcp_connect.packet_write.packet_too_large", size = field_0, max = field_1)]
    PacketTooLarge(usize, usize),
    #[error("qexed_tcp_connect.packet_write.compression_error", error = field_0.to_string())]
    CompressionError(std::io::Error),
    #[error("qexed_tcp_connect.packet_write.crypto_error", error = field_0)]
    CryptoError(String),
    #[error("qexed_tcp_connect.packet_write.invalid_encryption_key_length", length = field_0)]
    InvalidEncryptionKeyLength(usize),
    #[error("qexed_tcp_connect.packet_write.packet_encode_error", error = field_0.to_string())]
    PacketEncodeError(anyhow::Error),
}

impl From<anyhow::Error> for PacketWriteError {
    fn from(value: anyhow::Error) -> Self {
        Self::PacketEncodeError(value)
    }
}

impl std::error::Error for PacketWriteError {}

fn normalize_threshold(compression_threshold: isize) -> i32 {
    compression_threshold
        .try_into()
        .unwrap_or(if compression_threshold < 0 {
            -1
        } else {
            i32::MAX
        })
}
