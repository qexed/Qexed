use std::{fs, io};
use toml_edit::DocumentMut;

use crate::TomlError; // 加上 Item、Value
// 新建文件
pub fn create_file(path: &std::path::Path, config: &DocumentMut) -> Result<(), TomlError> {
    if has_file(path)? {
        return Err(TomlError::AlreadyExists);
    }
    // AutoDoc 的配置注释 v6版本不在此实现，bug又多又费时。
    // 没注释让他们自己去官网对比文档
    // 现版本不内嵌注释了
    fs::write(path, config.to_string())?;
    Ok(())
}
// 读取文件
pub fn load_file(path: &std::path::Path) -> Result<DocumentMut, TomlError> {
    if !has_file(path)? {
        return Err(TomlError::NotFound);
    }

    Ok(fs::read_to_string(path)?.parse::<DocumentMut>()?)
}
/// 保存文件
pub fn save_file(path: &std::path::Path, config: &DocumentMut) -> Result<(), TomlError> {
    if !has_file(path)? {
        // 不存在和新建没区别，我偷个懒咋地
        return create_file(path, config);
    } else {
        Ok(fs::write(path, config.to_string())?)
    }
}
/// 是否存在
pub fn has_file(path: &std::path::Path) -> Result<bool, TomlError> {
    if path.as_os_str().is_empty() {
        return Ok(false);
    }
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() => Ok(true),
        Ok(meta) if meta.is_dir() => Err(TomlError::InvalidInput(format!(
            "路径是文件夹，不是文件: {}",
            path.display()
        ))),
        Ok(_) => Err(TomlError::InvalidInput(format!(
            "路径存在但不是普通文件: {}",
            path.display()
        ))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}


