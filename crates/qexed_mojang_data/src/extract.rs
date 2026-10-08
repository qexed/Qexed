use std::{
    fs::File,
    io::{BufReader, Read, Seek},
    path::{Path, PathBuf},
};

use super::paths::DATA_MARKER;
use crate::MC_VERSION;
use crate::error::{IoCtx, MojangDataError};

pub(super) fn extract_minecraft_data(
    jar_path: &Path,
    cache_root: &Path,
) -> Result<(), MojangDataError> {
    let data_root = cache_root.join("data/minecraft");
    let tmp_root = cache_root.join("data.tmp");

    if tmp_root.exists() {
        std::fs::remove_dir_all(&tmp_root).io_ctx(format!(
            "无法清理 Mojang 临时数据目录: {}",
            tmp_root.display()
        ))?;
    }
    std::fs::create_dir_all(&tmp_root).io_ctx(format!(
        "无法创建 Mojang 临时数据目录: {}",
        tmp_root.display()
    ))?;

    let file = File::open(jar_path).io_ctx(format!("无法打开 Mojang jar: {}", jar_path.display()))?;
    let reader = BufReader::new(file);
    let mut archive = zip::ZipArchive::new(reader)?;

    let extracted = extract_data_from_archive(&mut archive, &tmp_root)?;
    let lang_extracted = extract_lang_files_from_archive(&mut archive, &tmp_root)?;

    if extracted == 0 {
        return Err(MojangDataError::NoMinecraftData);
    }

    let total_extracted = extracted + lang_extracted;
    std::fs::write(
        tmp_root.join("minecraft").join(DATA_MARKER),
        total_extracted.to_string(),
    )
    .io_ctx("无法写入 Mojang 数据缓存完成标记")?;

    if data_root.exists() {
        std::fs::remove_dir_all(&data_root).io_ctx(format!(
            "无法替换旧 Mojang 数据缓存: {}",
            data_root.display()
        ))?;
    }
    let final_root = cache_root.join("data");
    if final_root.exists() {
        std::fs::remove_dir_all(&final_root).io_ctx(format!(
            "无法清理旧 Mojang 数据目录: {}",
            final_root.display()
        ))?;
    }
    std::fs::rename(&tmp_root, &final_root).io_ctx(format!(
        "无法安装 Mojang 数据缓存: {}",
        final_root.display()
    ))?;

    log::info!(
        "Mojang registry data cached: version={}, registry_files={}, lang_files={}, path={}",
        MC_VERSION,
        extracted,
        lang_extracted,
        data_root.display()
    );
    Ok(())
}

fn extract_data_from_archive<R>(
    archive: &mut zip::ZipArchive<R>,
    tmp_root: &Path,
) -> Result<usize, MojangDataError>
where
    R: Read + Seek,
{
    let mut extracted = 0usize;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.is_dir() {
            continue;
        }
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        let Some(relative) = strip_minecraft_data_prefix(&name) else {
            continue;
        };
        if relative.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }

        let target = tmp_root.join("minecraft").join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).io_ctx(format!(
                "无法创建 Mojang 数据目录: {}",
                parent.display()
            ))?;
        }
        let mut output = File::create(&target).io_ctx(format!(
            "无法写入 Mojang 数据文件: {}",
            target.display()
        ))?;
        std::io::copy(&mut entry, &mut output).io_ctx(format!(
            "无法抽取 Mojang 数据文件: {}",
            target.display()
        ))?;
        extracted += 1;
    }
    Ok(extracted)
}

fn extract_lang_files_from_archive<R>(
    archive: &mut zip::ZipArchive<R>,
    tmp_root: &Path,
) -> Result<usize, MojangDataError>
where
    R: Read + Seek,
{
    let mut extracted = 0usize;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.is_dir() {
            continue;
        }
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        let Some(relative) = strip_minecraft_lang_prefix(&name) else {
            continue;
        };
        if relative.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }

        let target = tmp_root.join("minecraft").join("lang").join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).io_ctx(format!(
                "无法创建 Mojang lang 目录: {}",
                parent.display()
            ))?;
        }
        let mut output = File::create(&target).io_ctx(format!(
            "无法写入 Mojang lang 文件: {}",
            target.display()
        ))?;
        std::io::copy(&mut entry, &mut output).io_ctx(format!(
            "无法抽取 Mojang lang 文件: {}",
            target.display()
        ))?;
        extracted += 1;
    }
    Ok(extracted)
}

fn strip_minecraft_lang_prefix(path: &Path) -> Option<PathBuf> {
    let mut components = path.components();
    match (components.next(), components.next(), components.next()) {
        (
            Some(std::path::Component::Normal(assets)),
            Some(std::path::Component::Normal(minecraft)),
            Some(std::path::Component::Normal(lang)),
        ) if assets == "assets" && minecraft == "minecraft" && lang == "lang" => {
            Some(components.as_path().to_path_buf())
        }
        _ => None,
    }
}

fn strip_minecraft_data_prefix(path: &Path) -> Option<PathBuf> {
    let mut components = path.components();
    match (components.next(), components.next()) {
        (
            Some(std::path::Component::Normal(data)),
            Some(std::path::Component::Normal(minecraft)),
        ) if data == "data" && minecraft == "minecraft" => Some(components.as_path().to_path_buf()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{strip_minecraft_data_prefix, strip_minecraft_lang_prefix};

    #[test]
    fn strips_minecraft_data_prefix() {
        let path = Path::new("data/minecraft/tags/damage_type/is_fire.json");
        assert_eq!(
            strip_minecraft_data_prefix(path).unwrap(),
            Path::new("tags/damage_type/is_fire.json")
        );
    }

    #[test]
    fn rejects_non_minecraft_data_path() {
        assert!(
            strip_minecraft_data_prefix(Path::new("assets/minecraft/lang/en_us.json")).is_none()
        );
    }

    #[test]
    fn strips_minecraft_lang_prefix() {
        let path = Path::new("assets/minecraft/lang/en_us.json");
        assert_eq!(
            strip_minecraft_lang_prefix(path).unwrap(),
            Path::new("en_us.json")
        );
    }

    #[test]
    fn strips_minecraft_lang_prefix_for_zh_cn() {
        let path = Path::new("assets/minecraft/lang/zh_cn.json");
        assert_eq!(
            strip_minecraft_lang_prefix(path).unwrap(),
            Path::new("zh_cn.json")
        );
    }

    #[test]
    fn rejects_non_minecraft_lang_path() {
        assert!(
            strip_minecraft_lang_prefix(Path::new(
                "data/minecraft/tags/damage_type/is_fire.json"
            ))
            .is_none()
        );
    }
}
