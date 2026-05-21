use bytes::BufMut;
use bytes::{Buf, BytesMut};
use flate2::Compression;
use flate2::bufread::{ZlibDecoder, ZlibEncoder};
use qexed_packet::PacketCodec;
use std::io::Cursor;
use std::io::ErrorKind;
use std::io::Read;
use std::io::{Error, Result};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use tokio::io::{AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::{io::AsyncReadExt, net::TcpListener};
use openssl::symm::{Cipher, Crypter, Mode};
use bytes::{Bytes};
use anyhow::anyhow;
mod bind;
mod packet_read;
pub use packet_read::PacketStream;
pub use packet_read::PacketReadError;
pub use packet_read::PacketReadVarIntParseError;
pub use bind::BindError;
pub use bind::bind;

rust_i18n::i18n!("../../locales");
// 压缩阈值：当数据包长度超过此值时启用压缩
// pub struct PacketListener {
//     pub socket_read: ReadHalf<TcpStream>,
//     pub socket_write: WriteHalf<TcpStream>,
//     compression_threshold: isize,
//     compression_enabled: Arc<AtomicBool>, // 是否启用压缩
// }

// impl PacketListener {
//     pub fn new(
//         socket_read: ReadHalf<TcpStream>,
//         socket_write: WriteHalf<TcpStream>,
//         compression_threshold: isize,
//     ) -> Self {
//         Self {
//             socket_read,
//             socket_write,
//             compression_enabled: Arc::new(AtomicBool::new(false)),
//             compression_threshold: compression_threshold,
//         }
//     }
//     pub fn from_socket(socket: TcpStream, compression_threshold: isize) -> Self {
//         let (r, w) = tokio::io::split(socket);
//         Self::new(r, w, compression_threshold)
//     }
//     pub fn split(self) -> (PacketRead, PacketSend) {
//         let encryption_enabled =Arc::new(AtomicBool::new(false));
//         return (
//             PacketRead {
//                 buffer: BytesMut::with_capacity(4096),
//                 socket_read: self.socket_read,
//                 compression_enabled: Arc::clone(&self.compression_enabled),
//                 encryption_enabled: encryption_enabled.clone(),
//                 decrypter: None,
//                 encryption_buffer: BytesMut::new(),
//             },
//             PacketSend {
//                 socket_write: self.socket_write,
//                 compression_threshold: Arc::new(AtomicIsize::new(self.compression_threshold)),
//                 compression_enabled: Arc::clone(&self.compression_enabled),
//                 encryption_enabled: encryption_enabled,
//                 encrypter: None,
//             },
//         );
//     }
//     // 启用或禁用压缩
//     pub fn set_compression(&self, enabled: bool) {
//         self.compression_enabled.store(enabled, Ordering::Relaxed);
//     }
// }

pub struct PacketSend {
    pub socket_write: WriteHalf<TcpStream>,
    compression_threshold: Arc<AtomicIsize>,
    compression_enabled: Arc<AtomicBool>,
    encryption_enabled: Arc<AtomicBool>,
    encrypter: Option<Crypter>,
}

impl PacketSend {
    pub fn new(socket_write: WriteHalf<TcpStream>, compression_threshold: isize) -> Self {
        Self {
            socket_write,
            compression_threshold: Arc::new(AtomicIsize::new(compression_threshold)),
            compression_enabled: Arc::new(AtomicBool::new(false)),
            encryption_enabled: Arc::new(AtomicBool::new(false)),
            encrypter: None,
        }
    }

    pub async fn send<T: qexed_packet::Packet>(&mut self, packet: T) -> anyhow::Result<()> {
        self.send_raw(Self::build_send_packet(packet).await?).await
    }
    pub async fn build_send_packet<T: qexed_packet::Packet>(packet: T) -> anyhow::Result<Bytes>{
        let mut buf = BytesMut::new();
        let mut writer = qexed_packet::PacketWriter::new(&mut buf);
        qexed_packet::net_types::VarInt(T::ID as i32).serialize(&mut writer)?;
        packet.serialize(&mut writer)?;
        Ok(buf.freeze())
    }
    pub async fn send_raw(&mut self, data: Bytes) -> anyhow::Result<()> {
        let mut processed_data = BytesMut::new();

        // 1. 处理压缩
        if self.compression_enabled.load(Ordering::Relaxed) {
            self.compress_data(&data, &mut processed_data)?;
        } else {
            // 写入数据包长度
            write_varint(data.len() as i32, &mut processed_data);
            processed_data.put_slice(&data);
        }

        // 2. 处理加密
        let data_to_send = if self.encryption_enabled.load(Ordering::Relaxed) {
            if let Some(encrypter) = &mut self.encrypter {
                // 加密数据
                Self::encrypt_data_with(encrypter, &processed_data)?
            } else {
                // 加密已启用但没有加密器，不应该发生这种情况
                return Err(anyhow!("Encryption enabled but no encrypter found"));
            }
        } else {
            processed_data
        };

        // 3. 发送数据
        self.socket_write.write_all(&data_to_send).await?;
        Ok(())
    }

    /// 使用给定的加密器加密数据
    fn encrypt_data_with(
        encrypter: &mut Crypter,
        data: &BytesMut,
    ) -> anyhow::Result<BytesMut> {
        let input_len = data.len();
        let output_len = input_len; // AES/CFB8 输出大小与输入相同
        let mut output = BytesMut::with_capacity(output_len);
        output.resize(output_len, 0);

        // 加密数据
        let encrypted_len = encrypter.update(&data, &mut output[..])
            .map_err(|e| anyhow!("Encryption error: {}", e))?;

        let final_len = encrypter.finalize(&mut output[encrypted_len..])
            .map_err(|e| anyhow!("Encryption finalize error: {}", e))?;

        let total_len = encrypted_len + final_len;
        output.truncate(total_len);

        Ok(output)
    }

    /// 压缩数据
    fn compress_data(&self, data: &Bytes, output: &mut BytesMut) -> anyhow::Result<()> {
        let threshold = self.compression_threshold.load(Ordering::Relaxed) as i32;

        if data.len() >= threshold as usize && threshold >= 0 {
            // 压缩数据
            let mut encoder = ZlibEncoder::new(&data[..], Compression::default());
            let mut compressed = Vec::new();
            encoder.read_to_end(&mut compressed)?;

            // 计算总长度：未压缩长度 + 压缩数据
            let total_len = compressed.len() + varint_length(data.len() as i32);

            // 写入总长度
            write_varint(total_len as i32, output);
            // 写入未压缩数据长度
            write_varint(data.len() as i32, output);
            output.put_slice(&compressed);
        } else {
            // 小数据包不压缩
            // 计算总长度：0 + 数据长度
            let total_len = 1 + data.len(); // 0 的 varint 长度为 1

            // 写入总长度
            write_varint(total_len as i32, output);
            write_varint(0, output); // 0 表示未压缩
            output.put_slice(data);
        }

        Ok(())
    }

    /// 启用加密
    pub fn set_encryption(&mut self, shared_secret: &[u8]) -> anyhow::Result<()> {
        if shared_secret.len() != 16 {
            return Err(anyhow!("Shared secret must be 16 bytes for AES-128"));
        }

        // Minecraft 使用 AES-128/CFB8 模式
        let cipher = Cipher::aes_128_cfb8();

        // 创建加密器
        let encrypter = Crypter::new(
            cipher,
            Mode::Encrypt,
            shared_secret,
            Some(shared_secret)  // IV 为共享密钥
        ).map_err(|e| anyhow!("Failed to create encrypter: {}", e))?;

        // CFB8 模式不需要填充
        let mut encrypter = encrypter;
        encrypter.pad(false);

        self.encrypter = Some(encrypter);
        self.encryption_enabled.store(true, Ordering::Relaxed);

        Ok(())
    }

    /// 禁用加密
    pub fn disable_encryption(&mut self) {
        self.encryption_enabled.store(false, Ordering::Relaxed);
        self.encrypter = None;
    }

    /// 同步刷新
    pub async fn flush(&mut self) -> anyhow::Result<()> {
        self.socket_write.flush().await?;
        Ok(())
    }

    /// 获取底层 TCP 流
    pub fn into_inner(self) -> WriteHalf<TcpStream> {
        self.socket_write
    }

    /// 获取可写引用
    pub fn get_mut(&mut self) -> &mut WriteHalf<TcpStream> {
        &mut self.socket_write
    }

    /// 获取不可变引用
    pub fn get_ref(&self) -> &WriteHalf<TcpStream> {
        &self.socket_write
    }

    pub async fn shutdown(&mut self) -> anyhow::Result<()> {
        self.socket_write.shutdown().await?;
        Ok(())
    }

    /// 启用或禁用压缩
    pub fn set_compression(&self, enabled: bool) {
        self.compression_enabled.store(enabled, Ordering::Relaxed);
    }

    /// 设置压缩阈值
    pub fn set_compression_threshold(&self, compression_threshold: isize) {
        self.compression_threshold
            .store(compression_threshold, Ordering::Relaxed);
    }

    /// 检查加密是否启用
    pub fn is_encryption_enabled(&self) -> bool {
        self.encryption_enabled.load(Ordering::Relaxed)
    }

    /// 检查压缩是否启用
    pub fn is_compression_enabled(&self) -> bool {
        self.compression_enabled.load(Ordering::Relaxed)
    }
}
/// 读取 Minecraft 协议的变长整数 (VarInt)
fn read_varint<B: Buf>(buf: &mut B) -> Result<i32> {
    let mut value = 0;
    let mut position = 0;
    let mut current_byte;

    while position < 5 {
        if buf.remaining() == 0 {
            return Err(Error::new(ErrorKind::UnexpectedEof, "VarInt incomplete"));
        }

        current_byte = buf.get_u8();
        value |= (current_byte as i32 & 0x7F) << (7 * position);

        if (current_byte & 0x80) == 0 {
            return Ok(value);
        }

        position += 1;
    }

    Err(Error::new(ErrorKind::InvalidData, "VarInt too big"))
}
/// 写入 Minecraft 协议的变长整数 (VarInt)
fn write_varint(value: i32, buf: &mut BytesMut) {
    let mut val = value as u32;
    loop {
        let mut temp = (val & 0x7F) as u8;
        val >>= 7;
        if val != 0 {
            temp |= 0x80;
        }
        buf.put_u8(temp);
        if val == 0 {
            break;
        }
    }
}
/// 计算 VarInt 的字节长度
fn varint_length(value: i32) -> usize {
    let mut val = value as u32;
    let mut len = 0;

    loop {
        len += 1;
        if val < 128 {
            break;
        }
        val >>= 7;
    }

    len
}

// pub async fn read_one_packet<T: qexed_packet::Packet>(
//     packet_read: &mut PacketStream,
// ) -> anyhow::Result<T> {
//     let data = packet_read.read().await?;
//     let mut buf = BytesMut::new();
//     buf.extend_from_slice(&data);
//     let mut reader = qexed_packet::PacketReader::new(Box::new(&mut buf));
//     let mut id: qexed_packet::net_types::VarInt = Default::default();
//     id.deserialize(&mut reader)?;
//     if (id.0 as u32) != T::ID {
//         return Err(
//             qexed_protocol::error::ProtocolDecodeError::PacketIdMismatch {
//                 expected: T::ID,
//                 got: id.0 as u32,
//             }
//                 .into(),
//         );
//     }
//     let mut decoded: T = Default::default();
//     decoded.deserialize(&mut reader)?;
//     Ok(decoded)
// }
// pub fn decode_packet<T: qexed_packet::Packet>(
//     reader: &mut qexed_packet::PacketReader,
// ) -> anyhow::Result<T> {
//     let mut decoded: T = Default::default();
//     decoded.deserialize(reader)?;
//     Ok(decoded)
// }
