use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

const MAGIC: &[u8; 8] = b"QXPACK1\n";

pub fn pack_dir(source: impl AsRef<Path>, output: impl AsRef<Path>) -> Result<()> {
    let source = source.as_ref();
    let output = output.as_ref();
    let mut files = Vec::new();
    collect_files(source, source, &mut files)?;
    let mut writer =
        fs::File::create(output).with_context(|| format!("无法创建包 {}", output.display()))?;
    writer.write_all(MAGIC)?;
    writer.write_all(&(files.len() as u32).to_le_bytes())?;
    for (relative, path) in files {
        let data = fs::read(&path).with_context(|| format!("无法读取文件 {}", path.display()))?;
        let name = relative.to_string_lossy().replace('\\', "/");
        writer.write_all(&(name.len() as u32).to_le_bytes())?;
        writer.write_all(name.as_bytes())?;
        writer.write_all(&(data.len() as u64).to_le_bytes())?;
        writer.write_all(&data)?;
    }
    Ok(())
}

pub fn unpack(archive: impl AsRef<Path>, target: impl AsRef<Path>) -> Result<()> {
    let archive = archive.as_ref();
    let target = target.as_ref();
    let mut reader =
        fs::File::open(archive).with_context(|| format!("无法打开包 {}", archive.display()))?;
    let mut magic = [0_u8; 8];
    reader.read_exact(&mut magic)?;
    if &magic != MAGIC {
        anyhow::bail!("安装包格式无效: {}", archive.display());
    }

    fs::create_dir_all(target).with_context(|| format!("无法创建目录 {}", target.display()))?;
    let count = read_u32(&mut reader)?;
    for _ in 0..count {
        let name_len = read_u32(&mut reader)? as usize;
        let mut name = vec![0_u8; name_len];
        reader.read_exact(&mut name)?;
        let name = String::from_utf8(name).context("安装包内路径不是 UTF-8")?;
        let relative = safe_relative_path(&name)?;
        let file_len = read_u64(&mut reader)? as usize;
        let mut data = vec![0_u8; file_len];
        reader.read_exact(&mut data)?;

        let output = target.join(relative);
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&output, data).with_context(|| format!("无法写入文件 {}", output.display()))?;
    }
    Ok(())
}

fn collect_files(root: &Path, current: &Path, out: &mut Vec<(PathBuf, PathBuf)>) -> Result<()> {
    for entry in
        fs::read_dir(current).with_context(|| format!("无法读取目录 {}", current.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_files(root, &path, out)?;
        } else {
            let relative = path.strip_prefix(root)?.to_path_buf();
            out.push((relative, path));
        }
    }
    out.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(())
}

fn safe_relative_path(value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if path.is_absolute() || value.contains("..") {
        anyhow::bail!("安装包包含不安全路径: {value}");
    }
    Ok(path.to_path_buf())
}

fn read_u32(reader: &mut fs::File) -> Result<u32> {
    let mut bytes = [0_u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(reader: &mut fs::File) -> Result<u64> {
    let mut bytes = [0_u8; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_and_unpacks_directory() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let target = dir.path().join("target");
        fs::create_dir_all(source.join("config")).unwrap();
        fs::write(source.join("config/qexed.toml"), "version = 0").unwrap();
        let archive = dir.path().join("qexed.qxpack");

        pack_dir(&source, &archive).unwrap();
        unpack(&archive, &target).unwrap();

        assert_eq!(
            fs::read_to_string(target.join("config/qexed.toml")).unwrap(),
            "version = 0"
        );
    }
}
