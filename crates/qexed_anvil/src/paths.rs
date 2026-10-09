//! 存档目录约定：维度 -> region 目录、区块坐标 -> 区域文件名。

use std::path::{Path, PathBuf};

/// 三大 vanilla 维度。
pub const OVERWORLD: &str = "minecraft:overworld";
pub const THE_NETHER: &str = "minecraft:the_nether";
pub const THE_END: &str = "minecraft:the_end";

/// vanilla 存档布局下的维度 region 目录。
///
/// 主世界在根下；下界/末地在历史目录（DIM-1 / DIM1）；
/// 其他维度按惯例在 `dimensions/<namespace>/<path>/region`。
pub fn dimension_region_dir(save_root: &Path, dimension: &str) -> PathBuf {
    match dimension {
        THE_NETHER => save_root.join("DIM-1").join("region"),
        THE_END => save_root.join("DIM1").join("region"),
        OVERWORLD | "" => save_root.join("region"),
        other => {
            let (namespace, path) = split_dimension(other);
            save_root
                .join("dimensions")
                .join(namespace)
                .join(path)
                .join("region")
        }
    }
}

/// 区块坐标对应的区域文件名（`r.<x/32>.<z/32>.mca`，向下取整）。
pub fn region_file_name(chunk_x: i32, chunk_z: i32) -> String {
    format!("r.{}.{}.mca", floor_div(chunk_x, 32), floor_div(chunk_z, 32))
}

/// 区块坐标对应的区域文件完整路径。
pub fn region_file_path(save_root: &Path, dimension: &str, chunk_x: i32, chunk_z: i32) -> PathBuf {
    dimension_region_dir(save_root, dimension).join(region_file_name(chunk_x, chunk_z))
}

/// 列出维度目录下全部区域文件（目录不存在时返回空）。
pub fn list_region_files(save_root: &Path, dimension: &str) -> std::io::Result<Vec<PathBuf>> {
    let dir = dimension_region_dir(save_root, dimension);
    let mut files = Vec::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(files),
        Err(e) => return Err(e),
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("mca") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// 向下取整除法（i32 地板除）。
pub fn floor_div(value: i32, divisor: i32) -> i32 {
    value.div_euclid(divisor)
}

/// `minecraft:the_nether` -> (`minecraft`, `the_nether`)；无命名空间时默认 `minecraft`。
fn split_dimension(dimension: &str) -> (&str, &str) {
    match dimension.split_once(':') {
        Some((namespace, path)) => (namespace, path),
        None => ("minecraft", dimension),
    }
}
