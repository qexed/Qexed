use std::fs::File;
use std::io::{Read, Write, Result, Error, ErrorKind, Seek, SeekFrom, Cursor};
use std::path::{Path, PathBuf};
use byteorder::{ReadBytesExt, WriteBytesExt, BigEndian};
use flate2::read::{ZlibDecoder, GzDecoder};
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct ChunkLocation {
    pub offset: u32,
    pub sector_count: u8,
    pub is_external: bool,
}

#[derive(Debug, Clone)]
pub struct Header {
    pub locations: Vec<ChunkLocation>,
    pub timestamps: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct Anvil {
    pub header: Header,
    pub data: Vec<u8>,
    pub region_x: i32,
    pub region_z: i32,
    pub base_path: PathBuf,
    pub file_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ChunkData {
    pub length: u32,
    pub compression: u8,
    pub data: Vec<u8>,
    pub is_external: bool,
}

// 压缩类型常量
pub const COMPRESSION_GZIP: u8 = 1;
pub const COMPRESSION_ZLIB: u8 = 2;
pub const COMPRESSION_UNCOMPRESSED: u8 = 3;
pub const COMPRESSION_LZ4: u8 = 4;
pub const COMPRESSION_CUSTOM: u8 = 127;

// MCC文件头部大小（根据Wiki规范前5字节留空）
pub const MCC_HEADER_SIZE: u64 = 5;

impl Default for Header {
    fn default() -> Self {
        Self {
            locations: vec![ChunkLocation::default(); 1024],
            timestamps: vec![0; 1024],
        }
    }
}

impl Default for ChunkLocation {
    fn default() -> Self {
        Self {
            offset: 0,
            sector_count: 0,
            is_external: false,
        }
    }
}

impl ChunkLocation {
    pub fn from_bytes(bytes: [u8; 4]) -> Self {
        let offset = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], 0]) >> 8;
        let sector_count = bytes[3];
        
        let is_external = (sector_count & 0x80) != 0;
        let actual_sector_count = if is_external { sector_count & 0x7F } else { sector_count };
        
        Self {
            offset,
            sector_count: actual_sector_count,
            is_external,
        }
    }
    
    pub fn to_bytes(&self) -> [u8; 4] {
        let offset_bytes = (self.offset << 8).to_be_bytes();
        let sector_count = if self.is_external { 
            self.sector_count | 0x80 
        } else { 
            self.sector_count 
        };
        [offset_bytes[0], offset_bytes[1], offset_bytes[2], sector_count]
    }
    
    pub fn is_valid(&self) -> bool {
        self.offset != 0 && self.sector_count != 0
    }
    
    pub fn actual_offset(&self) -> u64 {
        (self.offset as u64) * 4096
    }
    
    pub fn actual_length(&self) -> u64 {
        (self.sector_count as u64) * 4096
    }
}

impl Header {
    pub fn from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let mut locations = Vec::with_capacity(1024);
        let mut timestamps = Vec::with_capacity(1024);
        
        // 读取位置表（4KB）
        for _ in 0..1024 {
            let mut bytes = [0u8; 4];
            reader.read_exact(&mut bytes)?;
            locations.push(ChunkLocation::from_bytes(bytes));
        }
        
        // 读取时间戳表（4KB）
        for _ in 0..1024 {
            timestamps.push(reader.read_u32::<BigEndian>()?);
        }
        
        Ok(Self {
            locations,
            timestamps,
        })
    }
    
    pub fn write_to_writer<W: Write>(&self, writer: &mut W) -> Result<()> {
        // 写入位置表
        for location in &self.locations {
            writer.write_all(&location.to_bytes())?;
        }
        
        // 写入时间戳表
        for timestamp in &self.timestamps {
            writer.write_u32::<BigEndian>(*timestamp)?;
        }
        
        Ok(())
    }
    
    pub fn get_chunk_index(x: i32, z: i32) -> usize {
        let x_mod = (x & 31) as usize;
        let z_mod = (z & 31) as usize;
        x_mod + z_mod * 32
    }
    
    pub fn get_chunk_location(&self, x: i32, z: i32) -> Option<&ChunkLocation> {
        let index = Self::get_chunk_index(x, z);
        self.locations.get(index)
    }
    
    pub fn get_chunk_location_mut(&mut self, x: i32, z: i32) -> Option<&mut ChunkLocation> {
        let index = Self::get_chunk_index(x, z);
        self.locations.get_mut(index)
    }
    
    pub fn get_valid_chunks(&self) -> Vec<(i32, i32, ChunkLocation)> {
        let mut chunks = Vec::new();
        
        for z in 0..32 {
            for x in 0..32 {
                let index = Self::get_chunk_index(x, z);
                if let Some(location) = self.locations.get(index) {
                    if location.is_valid() {
                        chunks.push((x, z, location.clone()));
                    }
                }
            }
        }
        
        chunks
    }
}

impl Anvil {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_ref = path.as_ref();
        let mut file = File::open(path_ref)?;
        
        // 从文件名解析区域坐标
        let (region_x, region_z) = Self::parse_region_coords(path_ref)
            .unwrap_or((0, 0));
        
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        
        let mut anvil = Self::from_bytes(&buffer)?;
        anvil.region_x = region_x;
        anvil.region_z = region_z;
        anvil.base_path = path_ref.parent().unwrap_or(Path::new(".")).to_path_buf();
        anvil.file_path = path_ref.to_path_buf();
        
        Ok(anvil)
    }
    
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < 8192 {
            return Err(Error::new(ErrorKind::UnexpectedEof, "Data too short for Anvil header"));
        }
        
        let mut cursor = std::io::Cursor::new(data);
        let header = Header::from_reader(&mut cursor)?;
        
        let remaining_data = data[8192..].to_vec();
        
        Ok(Self {
            header,
            data: remaining_data,
            region_x: 0,
            region_z: 0,
            base_path: PathBuf::from("."),
            file_path: PathBuf::from("unknown.mca"),
        })
    }
    
    pub fn new<P: AsRef<Path>>(path: P, region_x: i32, region_z: i32) -> Result<Self> {
        let path_ref = path.as_ref();
        
        Ok(Self {
            header: Header::default(),
            data: Vec::new(),
            region_x,
            region_z,
            base_path: path_ref.parent().unwrap_or(Path::new(".")).to_path_buf(),
            file_path: path_ref.to_path_buf(),
        })
    }
    
    /// 从文件名解析区域坐标
    fn parse_region_coords(path: &Path) -> Option<(i32, i32)> {
        let file_name = path.file_name()?.to_str()?;
        
        if file_name.starts_with("r.") && file_name.ends_with(".mca") {
            let parts: Vec<&str> = file_name[2..file_name.len()-4].split('.').collect();
            if parts.len() == 2 {
                if let (Ok(x), Ok(z)) = (parts[0].parse::<i32>(), parts[1].parse::<i32>()) {
                    return Some((x, z));
                }
            }
        }
        None
    }
    
    /// 获取区块的全局坐标
    fn get_global_coords(&self, x: i32, z: i32) -> (i32, i32) {
        (self.region_x * 32 + x, self.region_z * 32 + z)
    }
    
    /// 构建MCC文件名
    fn get_mcc_filename(&self, global_x: i32, global_z: i32) -> PathBuf {
        self.base_path.join(format!("c.{}.{}.mcc", global_x, global_z))
    }
    
    /// 从MCC文件读取区块数据
    fn read_chunk_from_mcc(&self, global_x: i32, global_z: i32) -> Result<ChunkData> {
        let mcc_path = self.get_mcc_filename(global_x, global_z);
        let mut file = File::open(&mcc_path)
            .map_err(|e| Error::new(ErrorKind::NotFound, 
                format!("MCC file not found: {}: {}", mcc_path.display(), e)))?;
        
        // 跳过前5个字节（根据Wiki规范留空）
        file.seek(SeekFrom::Start(MCC_HEADER_SIZE))?;
        
        // 读取数据长度
        let length = file.read_u32::<BigEndian>()?;
        let compression = file.read_u8()?;
        
        // 读取数据
        let mut data = vec![0u8; length as usize];
        file.read_exact(&mut data)?;
        
        Ok(ChunkData {
            length,
            compression,
            data,
            is_external: true,
        })
    }
    
    /// 写入区块数据到MCC文件
    fn write_chunk_to_mcc(&self, global_x: i32, global_z: i32, chunk_data: &ChunkData) -> Result<()> {
        let mcc_path = self.get_mcc_filename(global_x, global_z);
        let mut file = File::create(&mcc_path)?;
        
        // 写入前5个空字节（根据Wiki规范）
        file.write_all(&[0u8; MCC_HEADER_SIZE as usize])?;
        
        // 写入数据长度和压缩类型
        file.write_u32::<BigEndian>(chunk_data.length)?;
        file.write_u8(chunk_data.compression)?;
        
        // 写入数据
        file.write_all(&chunk_data.data)?;
        
        Ok(())
    }
    
    /// 检查是否需要外部存储（根据Wiki：超过1020KiB使用外部存储）
    fn needs_external_storage(chunk_data: &ChunkData) -> bool {
        // 总大小 = 数据长度(4字节) + 压缩类型(1字节) + 实际数据
        let total_size = 5 + chunk_data.data.len();
        total_size > 1020 * 1024 // 1020 KiB
    }
    
    pub fn get_chunk_data(&self, x: i32, z: i32) -> Result<Option<ChunkData>> {
        let location = match self.header.get_chunk_location(x, z) {
            Some(loc) if loc.is_valid() => loc,
            _ => return Ok(None),
        };
        
        // 处理外部存储的区块
        if location.is_external {
            let (global_x, global_z) = self.get_global_coords(x, z);
            match self.read_chunk_from_mcc(global_x, global_z) {
                Ok(chunk_data) => return Ok(Some(chunk_data)),
                Err(e) => {
                    eprintln!("Warning: Failed to read external chunk data from MCC file: {}", e);
                    return Ok(None);
                }
            }
        }
        
        // 处理内部存储的区块
        let offset = location.actual_offset() as usize;
        let length = location.actual_length() as usize;
        
        // 检查边界
        if offset < 8192 || offset + length > 8192 + self.data.len() {
            return Err(Error::new(ErrorKind::InvalidData, "Chunk data out of bounds"));
        }
        
        // 读取数据
        let chunk_data_start = offset - 8192;
        let chunk_data = &self.data[chunk_data_start..chunk_data_start + length];
        
        if chunk_data.len() < 5 {
            return Err(Error::new(ErrorKind::InvalidData, "Chunk data too short"));
        }
        
        let data_length = u32::from_be_bytes([
            chunk_data[0],
            chunk_data[1],
            chunk_data[2],
            chunk_data[3],
        ]) as usize;
        
        let compression = chunk_data[4];
        
        if data_length + 5 > chunk_data.len() {
            return Err(Error::new(ErrorKind::InvalidData, "Chunk data length mismatch"));
        }
        
        Ok(Some(ChunkData {
            length: data_length as u32,
            compression,
            data: chunk_data[5..5 + data_length].to_vec(),
            is_external: false,
        }))
    }
    
    /// 写入区块数据
    pub fn write_chunk_data(&mut self, x: i32, z: i32, mut chunk_data: ChunkData) -> Result<()> {
        let index = Header::get_chunk_index(x, z);
        
        // 检查是否需要外部存储
        let needs_external = Self::needs_external_storage(&chunk_data);
        
        if needs_external {
            // 外部存储：写入MCC文件
            let (global_x, global_z) = self.get_global_coords(x, z);
            self.write_chunk_to_mcc(global_x, global_z, &chunk_data)?;
            
            // 在MCA文件中创建占位符（根据Wiki规范）
            chunk_data.length = 1; // 数据长度设为1
            chunk_data.compression |= 0x80; // 设置最高位标记为外部存储
            chunk_data.data = vec![0]; // 最小数据
            
            // 更新位置表标记为外部存储
            if let Some(location) = self.header.locations.get_mut(index) {
                location.is_external = true;
            }
        } else {
            // 内部存储：清除外部存储标记
            if let Some(location) = self.header.locations.get_mut(index) {
                location.is_external = false;
            }
        }
        
        // 准备写入数据
        let mut data_to_write = Vec::new();
        data_to_write.write_u32::<BigEndian>(chunk_data.length)?;
        data_to_write.write_u8(chunk_data.compression)?;
        data_to_write.write_all(&chunk_data.data)?;
        
        // 计算需要的扇区数（向上取整到4KB）
        let total_size = data_to_write.len();
        let sector_count = ((total_size + 4095) / 4096) as u8;
        
        // 寻找空闲空间或追加到文件末尾
        let offset = self.find_free_space(sector_count)?;
        
        // 更新位置表
        if let Some(location) = self.header.locations.get_mut(index) {
            location.offset = offset;
            location.sector_count = sector_count;
        } else {
            return Err(Error::new(ErrorKind::InvalidInput, "Invalid chunk coordinates"));
        }
        
        // 更新时间戳
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;
        
        if let Some(ts) = self.header.timestamps.get_mut(index) {
            *ts = timestamp;
        }
        
        // 写入数据到内部存储
        self.write_data_to_offset(offset, &data_to_write, sector_count)?;
        
        Ok(())
    }
    
    /// 寻找空闲空间
    fn find_free_space(&self, required_sectors: u8) -> Result<u32> {
        // 简单实现：总是追加到文件末尾
        // 在实际实现中，应该查找已释放的空间
        
        let current_end = (self.data.len() + 8192) as u32;
        let offset = (current_end + 4095) / 4096; // 对齐到下一个扇区
        
        Ok(offset)
    }
    
    /// 在指定偏移写入数据
    fn write_data_to_offset(&mut self, offset: u32, data: &[u8], sector_count: u8) -> Result<()> {
        let actual_offset = (offset * 4096) as usize;
        let total_size = (sector_count as usize) * 4096;
        
        // 确保数据区域足够大
        if actual_offset + total_size > 8192 + self.data.len() {
            let needed_size = actual_offset + total_size - 8192;
            if needed_size > self.data.len() {
                self.data.resize(needed_size, 0);
            }
        }
        
        // 计算在data向量中的位置
        let data_offset = actual_offset - 8192;
        
        // 写入数据
        if data_offset + data.len() <= self.data.len() {
            self.data[data_offset..data_offset + data.len()].copy_from_slice(data);
            
            // 填充剩余空间为0（根据规范）
            let padding_start = data_offset + data.len();
            let padding_end = data_offset + total_size;
            if padding_start < padding_end && padding_end <= self.data.len() {
                for i in padding_start..padding_end {
                    self.data[i] = 0;
                }
            }
            
            Ok(())
        } else {
            Err(Error::new(ErrorKind::InvalidData, "Data exceeds allocated space"))
        }
    }
    
    /// 保存到文件
    pub fn save(&self) -> Result<()> {
        let mut file = File::create(&self.file_path)?;
        
        // 写入文件头
        self.header.write_to_writer(&mut file)?;
        
        // 写入数据部分
        file.write_all(&self.data)?;
        
        Ok(())
    }
    
    /// 保存到指定路径
    pub fn save_as<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let mut file = File::create(path.as_ref())?;
        
        // 写入文件头
        self.header.write_to_writer(&mut file)?;
        
        // 写入数据部分
        file.write_all(&self.data)?;
        
        Ok(())
    }
    
    /// 删除区块
    pub fn delete_chunk(&mut self, x: i32, z: i32) -> Result<()> {
        let index = Header::get_chunk_index(x, z);
        
        // 重置位置表项
        if let Some(location) = self.header.locations.get_mut(index) {
            location.offset = 0;
            location.sector_count = 0;
            location.is_external = false;
        }
        
        // 重置时间戳
        if let Some(timestamp) = self.header.timestamps.get_mut(index) {
            *timestamp = 0;
        }
        
        // 注意：这里没有实际释放数据空间，只是标记为可重用
        // 在实际实现中，应该维护空闲空间列表
        
        Ok(())
    }
    
    /// 解压缩区块数据
    pub fn decompress_chunk_data(chunk_data: &ChunkData) -> Result<Vec<u8>> {
        match chunk_data.compression & 0x7F { // 清除外部存储标记位
            COMPRESSION_GZIP => {
                let mut decoder = GzDecoder::new(&chunk_data.data[..]);
                let mut decompressed = Vec::new();
                decoder.read_to_end(&mut decompressed)?;
                Ok(decompressed)
            },
            COMPRESSION_ZLIB => {
                let mut decoder = ZlibDecoder::new(&chunk_data.data[..]);
                let mut decompressed = Vec::new();
                decoder.read_to_end(&mut decompressed)?;
                Ok(decompressed)
            },
            COMPRESSION_UNCOMPRESSED => {
                Ok(chunk_data.data.clone())
            },
            COMPRESSION_LZ4 => {
                // LZ4解压缩需要额外的crate，这里返回错误或原始数据
                Err(Error::new(ErrorKind::Unsupported, "LZ4 compression not yet supported"))
            },
            COMPRESSION_CUSTOM => {
                Err(Error::new(ErrorKind::Unsupported, "Custom compression requires external implementation"))
            },
            _ => {
                Err(Error::new(ErrorKind::InvalidData, 
                    format!("Unknown compression type: {}", chunk_data.compression)))
            }
        }
    }
    
    /// 压缩数据（用于写入）
    pub fn compress_data(data: &[u8], compression_type: u8) -> Result<Vec<u8>> {
        match compression_type {
            COMPRESSION_GZIP => {
                // GZip压缩需要额外的实现
                Err(Error::new(ErrorKind::Unsupported, "GZip compression not yet supported"))
            },
            COMPRESSION_ZLIB => {
                let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
                encoder.write_all(data)?;
                Ok(encoder.finish()?)
            },
            COMPRESSION_UNCOMPRESSED => {
                Ok(data.to_vec())
            },
            COMPRESSION_LZ4 => {
                Err(Error::new(ErrorKind::Unsupported, "LZ4 compression not yet supported"))
            },
            COMPRESSION_CUSTOM => {
                Err(Error::new(ErrorKind::Unsupported, "Custom compression requires external implementation"))
            },
            _ => {
                Err(Error::new(ErrorKind::InvalidData, 
                    format!("Unknown compression type: {}", compression_type)))
            }
        }
    }
    
    pub fn analyze_file(&self) {
        println!("=== Anvil 文件分析 ===");
        println!("区域文件坐标: r.{}.{}.mca", self.region_x, self.region_z);
        println!("文件总大小: {} 字节", 8192 + self.data.len());
        println!("数据部分大小: {} 字节", self.data.len());
        
        let valid_chunks = self.header.get_valid_chunks();
        println!("有效区块数量: {}", valid_chunks.len());
        
        // 分析区块分布
        let mut sector_usage = 0;
        let mut total_chunk_size = 0;
        let mut external_chunks = 0;
        
        for (x, z, location) in &valid_chunks {
            sector_usage += location.sector_count as u32;
            
            if location.is_external {
                external_chunks += 1;
                let (global_x, global_z) = self.get_global_coords(*x, *z);
                println!("  外部存储区块: 区域坐标 ({}, {}) -> 全局坐标 ({}, {})", 
                         x, z, global_x, global_z);
            } else if let Ok(Some(chunk_data)) = self.get_chunk_data(*x, *z) {
                total_chunk_size += chunk_data.length + 5; // 包括长度和压缩类型字段
            }
        }
        
        println!("使用的磁盘扇区: {} ({} 字节)", sector_usage, sector_usage * 4096);
        println!("实际区块数据大小: {} 字节", total_chunk_size);
        println!("外部存储区块数量: {}", external_chunks);
        
        if sector_usage > 0 {
            println!("空间利用率: {:.2}%", (total_chunk_size as f64) / (sector_usage as f64 * 4096.0) * 100.0);
        }
        
        // 显示前几个有效区块的信息
        println!("\n前10个有效区块:");
        for (i, (x, z, location)) in valid_chunks.iter().take(10).enumerate() {
            let storage_type = if location.is_external { "外部(MCC)" } else { "内部" };
            println!("  {}. 坐标 ({}, {}): 偏移={}, 扇区={}, 存储={}", 
                     i + 1, x, z, location.offset, location.sector_count, storage_type);
            
            if let Ok(Some(chunk_data)) = self.get_chunk_data(*x, *z) {
                let format_name = match chunk_data.compression {
                    COMPRESSION_GZIP => "Gzip",
                    COMPRESSION_ZLIB => "Zlib", 
                    COMPRESSION_UNCOMPRESSED => "Uncompressed",
                    COMPRESSION_LZ4 => "LZ4",
                    COMPRESSION_CUSTOM => "Custom",
                    _ => "Unknown",
                };
                println!("     长度: {} 字节, 压缩格式: {} ({})", 
                         chunk_data.length, chunk_data.compression, format_name);
            }
        }
        
        // 分析压缩格式分布
        let mut compression_stats = HashMap::new();
        for (x, z, _) in &valid_chunks {
            if let Ok(Some(chunk_data)) = self.get_chunk_data(*x, *z) {
                *compression_stats.entry(chunk_data.compression).or_insert(0) += 1;
            }
        }
        
        println!("\n压缩格式分布:");
        for (compression, count) in compression_stats {
            let format_name = match compression {
                COMPRESSION_GZIP => "Gzip",
                COMPRESSION_ZLIB => "Zlib",
                COMPRESSION_UNCOMPRESSED => "Uncompressed",
                COMPRESSION_LZ4 => "LZ4",
                COMPRESSION_CUSTOM => "Custom",
                _ => "Unknown",
            };
            println!("  {} ({}): {} 个区块", format_name, compression, count);
        }
    }
    
    pub fn find_largest_chunks(&self, count: usize) -> Vec<(i32, i32, u32)> {
        let mut chunks_with_size = Vec::new();
        
        for (x, z, location) in self.header.get_valid_chunks() {
            if let Ok(Some(chunk_data)) = self.get_chunk_data(x, z) {
                chunks_with_size.push((x, z, chunk_data.length));
            }
        }
        
        chunks_with_size.sort_by(|a, b| b.2.cmp(&a.2));
        chunks_with_size.into_iter().take(count).collect()
    }
    
    /// 检查MCC文件是否存在
    pub fn check_mcc_files(&self) -> Vec<(i32, i32, PathBuf, bool)> {
        let mut results = Vec::new();
        
        for (x, z, location) in self.header.get_valid_chunks() {
            if location.is_external {
                let (global_x, global_z) = self.get_global_coords(x, z);
                let mcc_path = self.get_mcc_filename(global_x, global_z);
                let exists = mcc_path.exists();
                results.push((x, z, mcc_path, exists));
            }
        }
        
        results
    }
}

fn main() -> Result<()> {
    let filename = "r.0.0.mca";
    
    println!("正在分析文件: {}", filename);
    
    match Anvil::from_file(filename) {
        Ok(anvil) => {
            // 基本文件分析
            anvil.analyze_file();
            
            // 检查MCC文件
            let mcc_files = anvil.check_mcc_files();
            if !mcc_files.is_empty() {
                println!("\nMCC外部文件检查:");
                for (x, z, path, exists) in mcc_files {
                    let status = if exists { "存在" } else { "缺失" };
                    println!("  区块 ({}, {}): {} -> {}", x, z, path.display(), status);
                }
            }
            
            // 查找最大的区块
            let largest_chunks = anvil.find_largest_chunks(5);
            println!("\n最大的5个区块:");
            for (i, (x, z, size)) in largest_chunks.iter().enumerate() {
                println!("  {}. 坐标 ({}, {}): {} 字节", i + 1, x, z, size);
            }
            
            // 演示写入功能
            println!("\n=== 写入功能演示 ===");
            
            // 创建新的区块数据示例
            let test_data = b"Test chunk data";
            let compressed_data = Anvil::compress_data(test_data, COMPRESSION_ZLIB)?;
            
            let test_chunk = ChunkData {
                length: compressed_data.len() as u32,
                compression: COMPRESSION_ZLIB,
                data: compressed_data,
                is_external: false,
            };
            
            println!("创建测试区块数据，大小: {} 字节", test_data.len());
            println!("压缩后大小: {} 字节", test_chunk.data.len());
            
            // 注意：在实际使用中，你需要创建一个可变的Anvil实例来写入
            // let mut anvil_mut = Anvil::from_file(filename)?;
            // anvil_mut.write_chunk_data(0, 0, test_chunk)?;
            // anvil_mut.save()?;
            
            println!("写入功能已实现，但在此演示中未实际执行写入操作");
        },
        Err(e) => {
            eprintln!("无法读取文件 {}: {}", filename, e);
            eprintln!("可能的原因:");
            eprintln!("  1. 文件不存在");
            eprintln!("  2. 文件格式不正确");
            eprintln!("  3. 文件已损坏");
            return Err(e);
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_chunk_location_external() {
        // 测试外部存储标记
        let bytes = [0x00, 0x00, 0x00, 0x81]; // sector_count = 129 (0x81)
        let location = ChunkLocation::from_bytes(bytes);
        assert!(location.is_external);
        assert_eq!(location.sector_count, 1); // 清除最高位后
        
        // 测试内部存储
        let bytes = [0x00, 0x00, 0x00, 0x01]; // sector_count = 1
        let location = ChunkLocation::from_bytes(bytes);
        assert!(!location.is_external);
        assert_eq!(location.sector_count, 1);
    }
    
    #[test]
    fn test_region_coords_parsing() {
        let path = Path::new("r.-1.2.mca");
        let coords = Anvil::parse_region_coords(path);
        assert_eq!(coords, Some((-1, 2)));
        
        let path = Path::new("invalid_name.mca");
        let coords = Anvil::parse_region_coords(path);
        assert_eq!(coords, None);
    }
    
    #[test]
    fn test_global_coords() {
        let anvil = Anvil {
            header: Header::default(),
            data: Vec::new(),
            region_x: 2,
            region_z: -3,
            base_path: PathBuf::from("."),
            file_path: PathBuf::from("test.mca"),
        };
        
        assert_eq!(anvil.get_global_coords(5, 10), (2 * 32 + 5, -3 * 32 + 10));
        assert_eq!(anvil.get_global_coords(0, 0), (64, -96));
    }
    
    #[test]
    fn test_compress_decompress() -> Result<()> {
        let test_data = b"Hello, Minecraft!";
        let compressed = Anvil::compress_data(test_data, COMPRESSION_ZLIB)?;
        let decompressed = Anvil::decompress_chunk_data(&ChunkData {
            length: compressed.len() as u32,
            compression: COMPRESSION_ZLIB,
            data: compressed,
            is_external: false,
        })?;
        
        assert_eq!(test_data, decompressed.as_slice());
        Ok(())
    }
}