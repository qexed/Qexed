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
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tokio::io::{AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::{io::AsyncReadExt, net::TcpListener};
use openssl::symm::{Cipher, Crypter, Mode};
use bytes::{Bytes}; 
use anyhow::anyhow;
// pub mod bridge;
// pub mod net_types;
// pub mod packet;
// pub mod player;
// 创建新的tcp服务器
pub async fn new_tcp_server(ip: &str, port: u16) -> Result<TcpListener> {
    let addr = format!("{}:{}", ip, port);
    let conn = TcpListener::bind(addr).await?;
    Ok(conn)
}
pub async fn new_tcp_server_by_addr(addr: String) -> Result<TcpListener> {
    let conn = TcpListener::bind(addr).await?;
    Ok(conn)
}
// 压缩阈值：当数据包长度超过此值时启用压缩
pub struct PacketListener {
    pub socket_read: ReadHalf<TcpStream>,
    pub socket_write: WriteHalf<TcpStream>,
    compression_threshold: usize,
    compression_enabled: Arc<AtomicBool>, // 是否启用压缩
}

impl PacketListener {
    pub fn new(
        socket_read: ReadHalf<TcpStream>,
        socket_write: WriteHalf<TcpStream>,
        compression_threshold: usize,
    ) -> Self {
        Self {
            socket_read,
            socket_write,
            compression_enabled: Arc::new(AtomicBool::new(false)),
            compression_threshold: compression_threshold,
        }
    }
    pub fn from_socket(socket: TcpStream, compression_threshold: usize) -> Self {
        let (r, w) = tokio::io::split(socket);
        Self::new(r, w, compression_threshold)
    }
    pub fn split(self) -> (PacketRead, PacketSend) {
        let encryption_enabled =Arc::new(AtomicBool::new(false));
        return (
            PacketRead {
                buffer: BytesMut::with_capacity(4096),
                socket_read: self.socket_read,
                compression_enabled: Arc::clone(&self.compression_enabled),
                encryption_enabled: encryption_enabled.clone(),
                decrypter: None,
                encryption_buffer: BytesMut::new(),
            },
            PacketSend {
                socket_write: self.socket_write,
                compression_threshold: Arc::new(AtomicUsize::new(self.compression_threshold)),
                compression_enabled: Arc::clone(&self.compression_enabled),
                encryption_enabled: encryption_enabled,
                encrypter: None,
            },
        );
    }
    // 启用或禁用压缩
    pub fn set_compression(&self, enabled: bool) {
        self.compression_enabled.store(enabled, Ordering::Relaxed);
    }
}

pub struct PacketSend {
    pub socket_write: WriteHalf<TcpStream>,
    compression_threshold: Arc<AtomicUsize>,
    compression_enabled: Arc<AtomicBool>,
    encryption_enabled: Arc<AtomicBool>,
    encrypter: Option<Crypter>,
}

impl PacketSend {
    pub fn new(socket_write: WriteHalf<TcpStream>, compression_threshold: usize) -> Self {
        Self {
            socket_write,
            compression_threshold: Arc::new(AtomicUsize::new(compression_threshold)),
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
    pub fn set_compression_threshold(&self, compression_threshold: usize) {
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
pub struct PacketRead {
    pub socket_read: ReadHalf<TcpStream>,
    buffer: BytesMut,
    compression_enabled: Arc<AtomicBool>,
    encryption_enabled: Arc<AtomicBool>,
    decrypter: Option<Crypter>,
    encryption_buffer: BytesMut,
}

impl PacketRead {
    pub async fn read(&mut self) -> Result<Vec<u8>> {
        loop {
            if let Some(packet) = self.try_parse_packet()? {
                return Ok(packet);
            }

            // 从套接字读取更多数据
            let mut temp_buf = [0u8; 1024];
            match self.socket_read.read(&mut temp_buf).await {
                Ok(0) => {
                    return Err(Error::new(
                        ErrorKind::ConnectionAborted,
                        "Connection closed",
                    ));
                }
                Ok(n) => {
                    let data = &temp_buf[..n];
                    
                    if self.encryption_enabled.load(Ordering::Relaxed) {
                        // 如果有加密，将加密数据存入缓冲区
                        self.encryption_buffer.extend_from_slice(data);
                        
                        // 尝试解密缓冲区中的数据
                        if let Some(_decrypter) = &mut self.decrypter {
                            self.decrypt_available_data()?;
                        }
                    } else {
                        // 无加密，直接存入缓冲区
                        self.buffer.extend_from_slice(data);
                    }
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => continue,
                Err(e) => return Err(e),
            }
        }
    }

    /// 尝试从缓冲区解析完整数据包
    fn try_parse_packet(&mut self) -> Result<Option<Vec<u8>>> {
        // 创建缓冲区视图（不消耗数据）
        let mut buf_view = self.buffer.clone().freeze();

        // 1. 读取数据包长度 (VarInt)
        let packet_len = match read_varint(&mut buf_view) {
            Ok(len) => len as usize,
            Err(_) => return Ok(None), // 长度不完整
        };

        // 检查整个数据包是否可用
        let varint_len = self.buffer.len() - buf_view.len();
        if self.buffer.len() < varint_len + packet_len {
            return Ok(None);
        }

        // 消耗缓冲区中的长度字段
        self.buffer.advance(varint_len);

        // 提取数据包部分
        let packet_data = self.buffer.split_to(packet_len);

        // 2. 处理压缩
        let raw_data = if self.compression_enabled.load(Ordering::Relaxed) {
            self.decompress_packet(packet_data)?
        } else {
            packet_data.to_vec()
        };

        Ok(Some(raw_data))
    }

    /// 解密可用的加密数据
    fn decrypt_available_data(&mut self) -> Result<()> {
        if self.encryption_buffer.is_empty() {
            return Ok(());
        }

        let decrypter = self.decrypter.as_mut().unwrap();
        let encrypted_data = &self.encryption_buffer;
        
        // 计算解密后数据的大小（AES/CFB8 模式，解密后大小不变）
        let output_len = encrypted_data.len();
        let mut output = vec![0u8; output_len];
        
        // 解密数据
        let decrypted_len = decrypter.update(encrypted_data, &mut output)
            .map_err(|e| Error::new(ErrorKind::InvalidData, e.to_string()))?;
        
        // 最终处理
        let final_len = decrypter.finalize(&mut output[decrypted_len..])
            .map_err(|e| Error::new(ErrorKind::InvalidData, e.to_string()))?;
        
        let total_len = decrypted_len + final_len;
        
        // 将解密数据添加到缓冲区
        self.buffer.extend_from_slice(&output[..total_len]);
        
        // 清空已处理的加密数据
        self.encryption_buffer.clear();
        
        Ok(())
    }

    /// 解压缩数据包
    fn decompress_packet(&self, data: BytesMut) -> Result<Vec<u8>> {
        let mut cursor = Cursor::new(&data);

        // 读取未压缩数据长度
        let uncompressed_size = read_varint(&mut cursor)? as usize;
        let header_len = cursor.position() as usize;

        if uncompressed_size == 0 {
            // 未压缩的数据包
            Ok(data[header_len..].to_vec())
        } else {
            // 解压缩数据
            let compressed_data = &data[header_len..];
            let mut decoder = ZlibDecoder::new(compressed_data);
            let mut decompressed = Vec::with_capacity(uncompressed_size);
            decoder.read_to_end(&mut decompressed)?;

            if decompressed.len() != uncompressed_size {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "Decompressed size mismatch: expected {}, got {}",
                        uncompressed_size,
                        decompressed.len()
                    ),
                ));
            }

            Ok(decompressed)
        }
    }

    /// 启用或禁用压缩
    pub fn set_compression(&self, enabled: bool) {
        self.compression_enabled.store(enabled, Ordering::Relaxed);
    }

    /// 启用加密
    pub fn set_encryption(&mut self, shared_secret: &[u8]) -> anyhow::Result<()> {
        // Minecraft 使用 AES/CFB8 模式，IV 为共享密钥
        let cipher = Cipher::aes_128_cfb8();
        
        // 创建解密器
        // 注意：CFB8 模式不需要填充
        let mut decrypter = Crypter::new(
            cipher,
            Mode::Decrypt,
            shared_secret,
            Some(shared_secret)  // IV 为共享密钥
        ).map_err(|e| anyhow::anyhow!("Failed to create decrypter: {}", e))?;
        
        decrypter.pad(false);
        
        self.decrypter = Some(decrypter);
        self.encryption_enabled.store(true, Ordering::Relaxed);
        
        Ok(())
    }
    
    /// 禁用加密
    pub fn disable_encryption(&mut self) {
        self.encryption_enabled.store(false, Ordering::Relaxed);
        self.decrypter = None;
        self.encryption_buffer.clear();
    }
    
    /// 创建新的 PacketRead 实例
    pub fn new(socket_read: ReadHalf<TcpStream>) -> Self {
        Self {
            socket_read,
            buffer: BytesMut::with_capacity(8192),
            compression_enabled: Arc::new(AtomicBool::new(false)),
            encryption_enabled: Arc::new(AtomicBool::new(false)),
            decrypter: None,
            encryption_buffer: BytesMut::with_capacity(8192),
        }
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

pub async fn read_one_packet<T: qexed_packet::Packet>(
    packet_read: &mut PacketRead,
) -> anyhow::Result<T> {
    let data = packet_read.read().await?;
    let mut buf = BytesMut::new();
    buf.extend_from_slice(&data);
    let mut reader = qexed_packet::PacketReader::new(Box::new(&mut buf));
    let mut id: qexed_packet::net_types::VarInt = Default::default();
    id.deserialize(&mut reader)?;
    if (id.0 as u32) != T::ID {
        return Err(
            qexed_protocol::error::ProtocolDecodeError::PacketIdMismatch {
                expected: T::ID,
                got: id.0 as u32,
            }
            .into(),
        );
    }
    let mut decoded: T = Default::default();
    decoded.deserialize(&mut reader)?;
    Ok(decoded)
}
pub fn decode_packet<T: qexed_packet::Packet>(
    reader: &mut qexed_packet::PacketReader,
) -> anyhow::Result<T> {
    let mut decoded: T = Default::default();
    decoded.deserialize(reader)?;
    Ok(decoded)
}
