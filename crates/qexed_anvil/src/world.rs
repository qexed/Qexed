//! 完整存档（world 文件夹）的布局识别与迁移。
//!
//! Minecraft 存档不只是 .mca：`level.dat`（gzip NBT）、各维度目录、
//! 1.17+ 拆分出的 `entities/`、1.14+ 的 `poi/`、会话锁等。不同版本的
//! 目录约定也不同（下界/末地的 DIM 目录 vs dimensions 树）。

use std::path::{Path, PathBuf};

use crate::error::AnvilError;
use crate::migrate::{self, MigrateOptions, MigrationStats};

/// 存档布局描述（从磁盘探测）。
#[derive(Debug, Clone)]
pub struct WorldLayout {
    /// 存档根目录。
    pub root: PathBuf,
    /// `level.dat` 是否存在。
    pub has_level_dat: bool,
    /// `level.dat` 里的 DataVersion（解析失败为 None）。
    pub data_version: Option<i32>,
    /// 各维度的 (维度名, region 目录) 列表（含自定义维度）。
    pub dimensions: Vec<(String, PathBuf)>,
    /// 1.17+ 的实体目录（与维度同级的 entities/）。
    pub entities_dirs: Vec<PathBuf>,
    /// poi 目录。
    pub poi_dirs: Vec<PathBuf>,
    /// 旧版 MCRegion 文件（.mcr，1.2 之前）。无法迁移，仅报告。
    pub legacy_mcr_files: Vec<PathBuf>,
}

impl WorldLayout {
    /// 是否存在旧版区块（需要迁移）。
    pub fn needs_migration(&self) -> bool {
        self.data_version
            .map(|v| v < migrate::DATA_VERSION_1_18)
            .unwrap_or(false)
            || self.legacy_mcr_files.iter().any(|p| p.to_string_lossy().contains("region"))
    }
}

/// 探测存档目录的布局。
pub fn probe(root: impl AsRef<Path>) -> Result<WorldLayout, AnvilError> {
    let root = root.as_ref();
    if !root.is_dir() {
        return Err(AnvilError::OpenFailed {
            path: root.display().to_string(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "not a directory"),
        });
    }

    let level_dat = root.join("level.dat");
    let has_level_dat = level_dat.is_file();
    let data_version = if has_level_dat {
        read_level_dat_version(&level_dat)?
    } else {
        None
    };

    // 维度目录探测
    let mut dimensions = Vec::new();
    if root.join("region").is_dir() {
        dimensions.push(("minecraft:overworld".to_string(), root.join("region")));
    }
    // 旧版 DIM 目录（1.16 及以前 + 一直保持兼容的下界/末地）
    for (name, dir) in [
        ("minecraft:the_nether", "DIM-1"),
        ("minecraft:the_end", "DIM1"),
    ] {
        let dim_dir = root.join(dir);
        if dim_dir.join("region").is_dir() {
            dimensions.push((name.to_string(), dim_dir.join("region")));
        }
    }
    // 1.16+ 自定义维度树：dimensions/<namespace>/<path>/region
    let dims_root = root.join("dimensions");
    if dims_root.is_dir() {
        if let Ok(namespaces) = std::fs::read_dir(&dims_root) {
            for ns_entry in namespaces.flatten() {
                let ns = ns_entry.file_name().to_string_lossy().to_string();
                if let Ok(paths) = std::fs::read_dir(ns_entry.path()) {
                    for path_entry in paths.flatten() {
                        if path_entry.path().join("region").is_dir() {
                            let path_seg = path_entry.file_name().to_string_lossy().to_string();
                            dimensions.push((
                                format!("{ns}:{path_seg}"),
                                path_entry.path().join("region"),
                            ));
                        }
                    }
                }
            }
        }
    }

    // entities：与维度平级（1.17+），或在 DIM 目录内
    let mut entities_dirs = Vec::new();
    if root.join("entities").is_dir() {
        entities_dirs.push(root.join("entities"));
    }
    for dir in ["DIM-1", "DIM1"] {
        let e = root.join(dir).join("entities");
        if e.is_dir() {
            entities_dirs.push(e);
        }
    }

    // poi：同上
    let mut poi_dirs = Vec::new();
    if root.join("poi").is_dir() {
        poi_dirs.push(root.join("poi"));
    }
    for dir in ["DIM-1", "DIM1"] {
        let p = root.join(dir).join("poi");
        if p.is_dir() {
            poi_dirs.push(p);
        }
    }

    // 旧版 .mcr 文件
    let mut legacy_mcr_files = Vec::new();
    for (_, region_dir) in &dimensions {
        if let Ok(entries) = std::fs::read_dir(region_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("mcr") {
                    legacy_mcr_files.push(path);
                }
            }
        }
    }

    Ok(WorldLayout {
        root: root.to_path_buf(),
        has_level_dat,
        data_version,
        dimensions,
        entities_dirs,
        poi_dirs,
        legacy_mcr_files,
    })
}

/// 读取 level.dat 的 DataVersion（gzip named NBT -> Data.DataVersion）。
fn read_level_dat_version(path: &Path) -> Result<Option<i32>, AnvilError> {
    let Ok((_, root)) = qexed_nbt::from_file(path) else {
        return Ok(None);
    };
    let Tag::Compound(fields) = &root else {
        return Ok(None);
    };
    let Some(Tag::Compound(data)) = fields.get("Data") else {
        return Ok(None);
    };
    match data.get("DataVersion") {
        Some(Tag::Int(v)) => Ok(Some(*v)),
        // 1.9 之前没有 DataVersion 字段
        _ => Ok(None),
    }
}

use qexed_nbt::Tag;

/// 整个存档的迁移结果。
#[derive(Debug, Clone, Default)]
pub struct WorldMigrationReport {
    pub dimensions: Vec<(String, MigrationStats)>,
    /// level.dat 是否更新了 DataVersion。
    pub level_dat_updated: bool,
    /// 探测到但未迁移的 .mcr（MCRegion，1.2 之前）。需先用游戏或专用工具升级。
    pub skipped_mcr: usize,
}

/// 迁移整个存档目录：所有维度的 region 文件 + level.dat。
///
/// - 区块：1.13–1.17 布局 -> 现代；解析失败保持原样
/// - level.dat：DataVersion 更新为选项的目标版本（仅当更旧时）
/// - .mcr（pre-1.2 MCRegion）：跳过并报告——容器格式不同，本库不支持
/// - entities/poi 目录：不动（容器相同，内容版本语义由游戏自己处理）
pub fn migrate_world(
    root: impl AsRef<Path>,
    options: &MigrateOptions,
) -> Result<WorldMigrationReport, AnvilError> {
    let layout = probe(&root)?;
    let mut report = WorldMigrationReport::default();
    report.skipped_mcr = layout.legacy_mcr_files.len();

    for (name, region_dir) in &layout.dimensions {
        let files = list_mca_in(region_dir)?;
        let mut stats = MigrationStats::default();
        for file in files {
            let s = migrate::migrate_region_file(&file, options)?;
            stats.migrated += s.migrated;
            stats.modern += s.modern;
            stats.unsupported += s.unsupported;
            stats.failed += s.failed;
        }
        report.dimensions.push((name.clone(), stats));
    }

    // level.dat 更新
    if layout.has_level_dat {
        let level_path = layout.root.join("level.dat");
        if let Some(current) = layout.data_version {
            if current < options.target_data_version {
                update_level_dat_version(&level_path, options.target_data_version)?;
                report.level_dat_updated = true;
            }
        }
    }

    Ok(report)
}

fn list_mca_in(dir: &Path) -> Result<Vec<PathBuf>, AnvilError> {
    let mut files = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(files),
        Err(e) => return Err(e.into()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("mca") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// 更新 level.dat 的 DataVersion（保留其余字段），并写回 gzip named NBT。
fn update_level_dat_version(path: &Path, target: i32) -> Result<(), AnvilError> {
    let (name, root) = qexed_nbt::from_file(path)?;
    let Tag::Compound(fields) = &root else {
        return Err(AnvilError::NotACompound);
    };

    let mut new_fields = fields.as_ref().clone();
    let Some(Tag::Compound(data)) = new_fields.get_mut("Data") else {
        return Err(AnvilError::MissingField("level.dat Data".to_string()));
    };
    let data = std::sync::Arc::make_mut(data);
    data.insert("DataVersion".to_string(), Tag::Int(target));

    let new_root = Tag::Compound(std::sync::Arc::new(new_fields));
    Ok(qexed_nbt::to_file(path, &name, &new_root, true)?)
}