//! Mojang 数据生成器：用缓存好的 server.jar 运行 `--reports`，
//! 产出 registries.json / blocks.json / packets.json。
//!
//! 静态注册表数据不再从仓库 assets 获取，全部由此处自动生成并缓存。

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::error::{IoCtx, MojangDataError};

pub(super) const REPORTS_MARKER: &str = ".qexed-reports-ready";
pub(super) const REPORT_FILES: &[&str] = &["registries.json", "blocks.json", "packets.json"];

/// 确保报告已生成：缓存有效直接返回，否则跑 datagen。
///
/// `server_jar` 必须已存在（调用方负责在删 jar 前先做 reports）。
pub(super) fn ensure_reports(
    java_bin: &Path,
    server_jar: &Path,
    cache_root: &Path,
    mc_version: &str,
) -> Result<PathBuf, MojangDataError> {
    let reports_dir = cache_root.join("reports");
    if reports_ready(&reports_dir, mc_version) {
        return Ok(reports_dir);
    }

    let lock_path = cache_root.join(".reports.lock");
    let _lock = super::lock::DownloadLock::acquire(&lock_path)?;
    if reports_ready(&reports_dir, mc_version) {
        return Ok(reports_dir);
    }

    run_datagen(java_bin, server_jar, cache_root, &reports_dir, mc_version)?;
    if !reports_ready(&reports_dir, mc_version) {
        return Err(MojangDataError::ReportsNotReady(reports_dir));
    }
    Ok(reports_dir)
}

/// 标记文件内容 = MC 版本；版本变化自动失效重跑。
fn reports_ready(reports_dir: &Path, mc_version: &str) -> bool {
    let marker = reports_dir.join(REPORTS_MARKER);
    let Ok(content) = std::fs::read_to_string(&marker) else {
        return false;
    };
    if content.trim() != mc_version {
        return false;
    }
    REPORT_FILES
        .iter()
        .all(|name| reports_dir.join(name).is_file())
}

/// 供 lib.rs 判断 reports 是否已就绪（只查标记与文件，不重跑）。
pub(super) fn reports_ready_for_api(reports_dir: &Path) -> bool {
    reports_ready(reports_dir, crate::MC_VERSION)
}

fn run_datagen(
    java_bin: &Path,
    server_jar: &Path,
    cache_root: &Path,
    reports_dir: &Path,
    mc_version: &str,
) -> Result<(), MojangDataError> {
    // 在独立临时目录里跑，datagen 会往 ./generated 写产物。
    let work_dir = cache_root.join("datagen.tmp");
    if work_dir.exists() {
        std::fs::remove_dir_all(&work_dir)
            .io_ctx(format!("无法清理 datagen 临时目录: {}", work_dir.display()))?;
    }
    std::fs::create_dir_all(&work_dir)
        .io_ctx(format!("无法创建 datagen 临时目录: {}", work_dir.display()))?;

    log::info!(
        "运行 Mojang 数据生成器 (--reports): java={}, jar={}, work={}",
        java_bin.display(),
        server_jar.display(),
        work_dir.display()
    );

    let output = Command::new(java_bin)
        .arg(format!("-DbundlerMainClass=net.minecraft.data.Main"))
        .arg("-jar")
        .arg(server_jar)
        .arg("--reports")
        .current_dir(&work_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .io_ctx("无法启动 Mojang 数据生成器")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(MojangDataError::DatagenFailed {
            status: output.status.to_string(),
            stderr: stderr.tail(2000),
            stdout: stdout.tail(500),
        });
    }

    // 产物在 <work>/generated/reports/*.json
    let generated = work_dir.join("generated/reports");
    if !generated.is_dir() {
        return Err(MojangDataError::DatagenFailed {
            status: "exit ok".to_string(),
            stderr: format!("未找到产物目录: {}", generated.display()),
            stdout: String::new(),
        });
    }

    std::fs::create_dir_all(reports_dir)
        .io_ctx(format!("无法创建 reports 目录: {}", reports_dir.display()))?;
    let mut copied = 0usize;
    for name in REPORT_FILES {
        let from = generated.join(name);
        if !from.is_file() {
            continue;
        }
        std::fs::copy(&from, reports_dir.join(name)).io_ctx(format!(
            "无法复制报告文件: {} -> {}",
            from.display(),
            reports_dir.join(name).display()
        ))?;
        copied += 1;
    }
    if copied == 0 {
        return Err(MojangDataError::DatagenFailed {
            status: "exit ok".to_string(),
            stderr: format!("产物目录中没有已知的报告文件: {}", generated.display()),
            stdout: String::new(),
        });
    }

    std::fs::remove_dir_all(&work_dir).ok();
    std::fs::write(reports_dir.join(REPORTS_MARKER), mc_version).io_ctx(
        "无法写入 reports ready 标记",
    )?;
    log::info!("Mojang reports 就绪: {copied} 个文件, path={}", reports_dir.display());
    Ok(())
}

/// 截断辅助：保留错误信息尾部（datagen 日志很长，头部长度无意义）。
trait KeepTail {
    fn tail(&self, n: usize) -> String;
}

impl KeepTail for str {
    fn tail(&self, n: usize) -> String {
        if self.len() <= n {
            return self.to_string();
        }
        let start = self.len() - n;
        // 对齐到 char 边界
        let start = self
            .char_indices()
            .map(|(i, _)| i)
            .find(|&i| i >= start)
            .unwrap_or(start);
        self[start..].to_string()
    }
}
