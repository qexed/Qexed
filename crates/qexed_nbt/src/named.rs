//! Named (disk) NBT codec: compound root WITH a name; optional gzip via flate2.
use crate::net::{read_compound_content, read_string, write_compound_content, write_string};
use crate::{NbtError, Tag, tag_id};
use byteorder::{ReadBytesExt, WriteBytesExt};
use std::io::{BufReader, BufWriter, Cursor, Read, Seek, SeekFrom, Write};
use std::sync::Arc;

/// Named-format NBT reader/writer (root id + root name + compound).
pub struct NbtIo;

impl NbtIo {
    pub fn from_reader<R: Read>(mut reader: R) -> Result<(String, Tag), NbtError> {
        let id = reader.read_u8()?;
        if id != tag_id::COMPOUND {
            return Err(NbtError::Deserialize(format!(
                "named NBT root must be Compound, got 0x{id:02X}")));
        }
        let name = read_string(&mut reader)?;
        Ok((name, Tag::Compound(Arc::new(read_compound_content(&mut reader)?))))
    }

    pub fn to_writer<W: Write>(mut writer: W, name: &str, tag: &Tag) -> Result<(), NbtError> {
        let Tag::Compound(map) = tag else {
            return Err(NbtError::Serialize("named NBT root must be Compound".into()));
        };
        writer.write_u8(tag_id::COMPOUND)?;
        write_string(&mut writer, name)?;
        write_compound_content(&mut writer, map)
    }
}

/// Reads a named NBT file, transparently decompressing gzip.
pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<(String, Tag), NbtError> {
    use flate2::read::GzDecoder;
    use std::fs::File;

    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut magic = [0u8; 2];
    reader.read_exact(&mut magic)?;
    reader.seek(SeekFrom::Start(0))?;

    if magic == [0x1F, 0x8B] {
        NbtIo::from_reader(GzDecoder::new(reader))
    } else {
        NbtIo::from_reader(reader)
    }
}

/// Writes a named NBT file, optionally gzip-compressed.
pub fn to_file<P: AsRef<std::path::Path>>(
    path: P,
    name: &str,
    tag: &Tag,
    compress: bool,
) -> Result<(), NbtError> {
    use flate2::{Compression, write::GzEncoder};
    use std::fs::File;

    let file = File::create(path)?;
    if compress {
        NbtIo::to_writer(GzEncoder::new(BufWriter::new(file), Compression::default()), name, tag)
    } else {
        NbtIo::to_writer(BufWriter::new(file), name, tag)
    }
}

pub fn from_slice(data: &[u8]) -> Result<(String, Tag), NbtError> {
    NbtIo::from_reader(Cursor::new(data))
}

pub fn to_vec(name: &str, tag: &Tag) -> Result<Vec<u8>, NbtError> {
    let mut buf = Vec::new();
    NbtIo::to_writer(&mut buf, name, tag)?;
    Ok(buf)
}
