use std::{
    fs::File,
    io::{Cursor, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use bytes::{Buf, BufMut, BytesMut};
use flate2::read::{GzDecoder, ZlibDecoder};

const SECTOR_SIZE: usize = 4096;
const HEADER_SIZE: usize = SECTOR_SIZE * 2;
const CHUNK_COUNT: usize = 1024;
const EXTERNAL_STREAM_FLAG: u8 = 0x80;
const COMPRESSION_GZIP: u8 = 1;
const COMPRESSION_ZLIB: u8 = 2;
const COMPRESSION_NONE: u8 = 3;
const COMPRESSION_LZ4: u8 = 4;
const COMPRESSION_CUSTOM: u8 = 127;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkData {
    pub compression: u8,
    pub data: Vec<u8>,
}

impl ChunkData {
    pub fn zlib(data: &[u8]) -> Result<Self> {
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(data)?;
        Ok(Self {
            compression: COMPRESSION_ZLIB,
            data: encoder.finish()?,
        })
    }

    pub fn decompress(&self) -> Result<Vec<u8>> {
        match self.compression & !EXTERNAL_STREAM_FLAG {
            COMPRESSION_GZIP => {
                let mut decoder = GzDecoder::new(self.data.as_slice());
                let mut out = Vec::new();
                decoder.read_to_end(&mut out)?;
                Ok(out)
            }
            COMPRESSION_ZLIB => {
                let mut decoder = ZlibDecoder::new(self.data.as_slice());
                let mut out = Vec::new();
                decoder.read_to_end(&mut out)?;
                Ok(out)
            }
            COMPRESSION_NONE => Ok(self.data.clone()),
            COMPRESSION_LZ4 => anyhow::bail!("暂不支持 LZ4 区块压缩"),
            COMPRESSION_CUSTOM => anyhow::bail!("暂不支持自定义区块压缩"),
            compression => anyhow::bail!("不支持的区块压缩类型: {compression}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AnvilRegion {
    path: PathBuf,
    header: RegionHeader,
    data: BytesMut,
}

impl AnvilRegion {
    #[allow(dead_code)]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            header: RegionHeader::default(),
            data: BytesMut::new(),
        }
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let mut file =
            File::open(path).with_context(|| format!("无法打开区域文件: {}", path.display()))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        if bytes.len() < HEADER_SIZE {
            anyhow::bail!("区域文件头长度不足: {}", path.display());
        }

        let header = RegionHeader::read(&bytes[..HEADER_SIZE])?;
        Ok(Self {
            path: path.to_path_buf(),
            header,
            data: BytesMut::from(&bytes[HEADER_SIZE..]),
        })
    }

    pub fn read_chunk(&self, chunk_x: i32, chunk_z: i32) -> Result<Option<ChunkData>> {
        let Some(location) = self.header.location(chunk_x, chunk_z) else {
            return Ok(None);
        };

        if location.offset == 0 || location.sector_count == 0 {
            return Ok(None);
        }

        let offset = location.offset as usize * SECTOR_SIZE;
        let local_offset = offset
            .checked_sub(HEADER_SIZE)
            .context("区块偏移落在区域文件头内")?;
        if local_offset + 5 > self.data.len() {
            anyhow::bail!("区块偏移超出区域文件数据范围");
        }

        let mut cursor = Cursor::new(&self.data[local_offset..]);
        let length = cursor.get_u32() as usize;
        let compression = cursor.get_u8();
        if length == 0 {
            return Ok(None);
        }
        if compression & EXTERNAL_STREAM_FLAG != 0 {
            anyhow::bail!("暂不支持外部 .mcc 区块数据");
        }

        let data_length = length.saturating_sub(1);
        if local_offset + 5 + data_length > self.data.len() {
            anyhow::bail!("区块数据长度超出区域文件范围");
        }

        Ok(Some(ChunkData {
            compression,
            data: self.data[local_offset + 5..local_offset + 5 + data_length].to_vec(),
        }))
    }

    #[allow(dead_code)]
    pub fn write_chunk(&mut self, chunk_x: i32, chunk_z: i32, chunk: ChunkData) -> Result<()> {
        let mut payload = BytesMut::new();
        payload.put_u32((chunk.data.len() + 1) as u32);
        payload.put_u8(chunk.compression);
        payload.put_slice(&chunk.data);

        let sector_count = payload.len().div_ceil(SECTOR_SIZE);
        if sector_count > u8::MAX as usize {
            anyhow::bail!("区块数据过大，暂不支持外部 .mcc 存储");
        }

        let chunk_index = chunk_index(chunk_x, chunk_z);
        let current_location = self.header.locations[chunk_index];
        let fitting_current_location = valid_location(current_location)
            .filter(|location| sector_count <= usize::from(location.sector_count));
        let offset = if let Some(location) = fitting_current_location {
            self.write_into_existing_location(location, &payload)?
        } else if let Some(location) = self.free_location(sector_count, Some(chunk_index)) {
            self.write_into_existing_location(location, &payload)?
        } else {
            self.append_aligned(&payload)
        };
        self.header.set_location(
            chunk_x,
            chunk_z,
            ChunkLocation {
                offset,
                sector_count: sector_count as u8,
            },
        );
        self.header
            .set_timestamp(chunk_x, chunk_z, unix_timestamp());
        Ok(())
    }

    #[allow(dead_code)]
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut file = File::create(&self.path)
            .with_context(|| format!("无法保存区域文件: {}", self.path.display()))?;
        self.header.write(&mut file)?;
        file.write_all(&self.data)?;
        file.flush()?;
        file.seek(SeekFrom::End(0))?;
        Ok(())
    }

    fn append_aligned(&mut self, payload: &[u8]) -> u32 {
        let absolute_len = HEADER_SIZE + self.data.len();
        let aligned_len = absolute_len.div_ceil(SECTOR_SIZE) * SECTOR_SIZE;
        if aligned_len > absolute_len {
            self.data.resize(aligned_len - HEADER_SIZE, 0);
        }

        let offset = (aligned_len / SECTOR_SIZE) as u32;
        self.data.put_slice(payload);
        let padded_len = self.data.len().div_ceil(SECTOR_SIZE) * SECTOR_SIZE;
        self.data.resize(padded_len, 0);
        offset
    }

    fn write_into_existing_location(
        &mut self,
        location: ChunkLocation,
        payload: &[u8],
    ) -> Result<u32> {
        let offset = location.offset as usize * SECTOR_SIZE;
        let local_offset = offset
            .checked_sub(HEADER_SIZE)
            .context("鍖哄潡鍋忕Щ钀藉湪鍖哄煙鏂囦欢澶村唴")?;
        let byte_len = usize::from(location.sector_count) * SECTOR_SIZE;
        let end = local_offset + byte_len;
        if end > self.data.len() {
            anyhow::bail!("鍖哄潡鍋忕Щ瓒呭嚭鍖哄煙鏂囦欢鏁版嵁鑼冨洿");
        }
        if payload.len() > byte_len {
            anyhow::bail!("chunk payload exceeds existing sector capacity");
        }

        self.data[local_offset..end].fill(0);
        self.data[local_offset..local_offset + payload.len()].copy_from_slice(payload);
        Ok(location.offset)
    }

    fn free_location(
        &self,
        sector_count: usize,
        ignored_chunk_index: Option<usize>,
    ) -> Option<ChunkLocation> {
        let total_sectors = (HEADER_SIZE + self.data.len()).div_ceil(SECTOR_SIZE);
        let mut occupied = vec![false; total_sectors.max(2)];
        occupied[0] = true;
        occupied[1] = true;

        for (index, location) in self.header.locations.iter().copied().enumerate() {
            if Some(index) == ignored_chunk_index {
                continue;
            }
            let Some(location) = valid_location(location) else {
                continue;
            };
            let start = location.offset as usize;
            let end = start.saturating_add(usize::from(location.sector_count));
            for sector in start..end.min(occupied.len()) {
                occupied[sector] = true;
            }
        }

        let mut run_start = 0usize;
        let mut run_len = 0usize;
        for (sector, is_occupied) in occupied.iter().copied().enumerate().skip(2) {
            if is_occupied {
                run_len = 0;
                continue;
            }
            if run_len == 0 {
                run_start = sector;
            }
            run_len += 1;
            if run_len >= sector_count {
                return Some(ChunkLocation {
                    offset: run_start as u32,
                    sector_count: sector_count as u8,
                });
            }
        }

        None
    }
}

#[derive(Debug, Clone)]
struct RegionHeader {
    locations: [ChunkLocation; CHUNK_COUNT],
    timestamps: [u32; CHUNK_COUNT],
}

impl Default for RegionHeader {
    fn default() -> Self {
        Self {
            locations: [ChunkLocation::default(); CHUNK_COUNT],
            timestamps: [0; CHUNK_COUNT],
        }
    }
}

impl RegionHeader {
    fn read(bytes: &[u8]) -> Result<Self> {
        let mut header = Self::default();
        for index in 0..CHUNK_COUNT {
            let offset = index * 4;
            header.locations[index] = ChunkLocation::from_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ]);
        }

        for index in 0..CHUNK_COUNT {
            let offset = SECTOR_SIZE + index * 4;
            header.timestamps[index] = u32::from_be_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ]);
        }
        Ok(header)
    }

    fn write(&self, writer: &mut impl Write) -> Result<()> {
        for location in self.locations {
            writer.write_all(&location.to_bytes())?;
        }

        for timestamp in self.timestamps {
            writer.write_all(&timestamp.to_be_bytes())?;
        }
        Ok(())
    }

    fn location(&self, chunk_x: i32, chunk_z: i32) -> Option<ChunkLocation> {
        self.locations.get(chunk_index(chunk_x, chunk_z)).copied()
    }

    fn set_location(&mut self, chunk_x: i32, chunk_z: i32, location: ChunkLocation) {
        self.locations[chunk_index(chunk_x, chunk_z)] = location;
    }

    fn set_timestamp(&mut self, chunk_x: i32, chunk_z: i32, timestamp: u32) {
        self.timestamps[chunk_index(chunk_x, chunk_z)] = timestamp;
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct ChunkLocation {
    offset: u32,
    sector_count: u8,
}

impl ChunkLocation {
    fn from_bytes(bytes: [u8; 4]) -> Self {
        Self {
            offset: u32::from_be_bytes([0, bytes[0], bytes[1], bytes[2]]),
            sector_count: bytes[3],
        }
    }

    fn to_bytes(self) -> [u8; 4] {
        let offset = self.offset.to_be_bytes();
        [offset[1], offset[2], offset[3], self.sector_count]
    }
}

fn valid_location(location: ChunkLocation) -> Option<ChunkLocation> {
    (location.offset > 0 && location.sector_count > 0).then_some(location)
}

fn chunk_index(chunk_x: i32, chunk_z: i32) -> usize {
    (chunk_x.rem_euclid(32) + chunk_z.rem_euclid(32) * 32) as usize
}

fn unix_timestamp() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs().min(u32::MAX as u64) as u32)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{AnvilRegion, ChunkData};

    #[test]
    fn region_roundtrip_preserves_chunk_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("r.0.0.mca");
        let raw = b"hello chunk";
        let chunk = ChunkData::zlib(raw).unwrap();

        let mut region = AnvilRegion::new(&path);
        region.write_chunk(0, 0, chunk).unwrap();
        region.save().unwrap();

        let region = AnvilRegion::from_file(&path).unwrap();
        let loaded = region.read_chunk(0, 0).unwrap().unwrap();
        assert_eq!(loaded.decompress().unwrap(), raw);
        assert!(region.read_chunk(1, 0).unwrap().is_none());
    }

    #[test]
    fn rewriting_chunk_reuses_existing_sectors_when_payload_fits() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("r.0.0.mca");
        let large_raw = vec![42_u8; 7000];
        let small_raw = b"small chunk";
        let large = ChunkData {
            compression: super::COMPRESSION_NONE,
            data: large_raw.clone(),
        };
        let small = ChunkData {
            compression: super::COMPRESSION_NONE,
            data: small_raw.to_vec(),
        };

        let mut region = AnvilRegion::new(&path);
        region.write_chunk(0, 0, large).unwrap();
        region.save().unwrap();
        let initial_len = std::fs::metadata(&path).unwrap().len();

        let mut region = AnvilRegion::from_file(&path).unwrap();
        region.write_chunk(0, 0, small).unwrap();
        region.save().unwrap();

        assert_eq!(std::fs::metadata(&path).unwrap().len(), initial_len);
        let region = AnvilRegion::from_file(&path).unwrap();
        let loaded = region.read_chunk(0, 0).unwrap().unwrap();
        assert_eq!(loaded.decompress().unwrap(), small_raw);
    }

    #[test]
    fn writing_chunk_reuses_free_sectors_left_by_moved_chunk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("r.0.0.mca");
        let large_raw = vec![1_u8; 7000];
        let larger_raw = vec![2_u8; 13_000];
        let small_raw = b"uses free sectors";
        let large = ChunkData {
            compression: super::COMPRESSION_NONE,
            data: large_raw,
        };
        let larger = ChunkData {
            compression: super::COMPRESSION_NONE,
            data: larger_raw.clone(),
        };
        let small = ChunkData {
            compression: super::COMPRESSION_NONE,
            data: small_raw.to_vec(),
        };

        let mut region = AnvilRegion::new(&path);
        region.write_chunk(0, 0, large).unwrap();
        region.write_chunk(0, 0, larger).unwrap();
        region.save().unwrap();
        let len_before_free_reuse = std::fs::metadata(&path).unwrap().len();

        let mut region = AnvilRegion::from_file(&path).unwrap();
        region.write_chunk(1, 0, small).unwrap();
        region.save().unwrap();

        assert_eq!(
            std::fs::metadata(&path).unwrap().len(),
            len_before_free_reuse
        );
        let region = AnvilRegion::from_file(&path).unwrap();
        assert_eq!(
            region
                .read_chunk(0, 0)
                .unwrap()
                .unwrap()
                .decompress()
                .unwrap(),
            larger_raw
        );
        assert_eq!(
            region
                .read_chunk(1, 0)
                .unwrap()
                .unwrap()
                .decompress()
                .unwrap(),
            small_raw
        );
    }

    #[test]
    fn decompress_supports_uncompressed_chunks() {
        let chunk = ChunkData {
            compression: super::COMPRESSION_NONE,
            data: b"plain chunk".to_vec(),
        };

        assert_eq!(chunk.decompress().unwrap(), b"plain chunk");
    }

    #[test]
    fn decompress_supports_gzip_chunks() {
        let raw = b"gzip chunk";
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        std::io::Write::write_all(&mut encoder, raw).unwrap();
        let chunk = ChunkData {
            compression: super::COMPRESSION_GZIP,
            data: encoder.finish().unwrap(),
        };

        assert_eq!(chunk.decompress().unwrap(), raw);
    }
}
