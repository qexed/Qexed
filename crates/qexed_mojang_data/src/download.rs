use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
    time::Instant,
};

use sha1::{Digest, Sha1};

use super::manifest::DownloadInfo;
use crate::error::{IoCtx, MojangDataError};

pub(super) fn download_if_needed(
    client: &reqwest::blocking::Client,
    kind: &str,
    asset: &DownloadInfo,
    target: &Path,
) -> Result<(), MojangDataError> {
    if target.is_file() {
        match verify_sha1(target, &asset.sha1) {
            Ok(()) => return Ok(()),
            Err(err) => {
                log::warn!(
                    "{}",
                    qexed_language::t("qexed.mojang_data.jar.checksum_failed")
                        .replace("%{path}", &target.display().to_string())
                        .replace("%{error}", &err.to_string())
                );
            }
        }
    }

    let mut response = client.get(&asset.url).send()?.error_for_status()?;
    let total = if asset.size > 0 {
        asset.size
    } else {
        response.content_length().unwrap_or(0)
    };

    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).io_ctx(format!(
            "无法创建 Mojang jar 缓存目录: {}",
            parent.display()
        ))?;
    }
    let tmp = target.with_extension("jar.tmp");
    let file = File::create(&tmp)
        .io_ctx(format!("无法写入 Mojang jar 临时缓存: {}", tmp.display()))?;
    let mut writer = ProgressWriter::new(kind, file, total);
    let copied = response.copy_to(&mut writer)?;
    writer.finish();

    if asset.size > 0 && copied != asset.size {
        return Err(MojangDataError::SizeMismatch {
            expected: asset.size,
            actual: copied as usize,
        });
    }
    verify_sha1(&tmp, &asset.sha1)?;
    std::fs::rename(&tmp, target)
        .io_ctx(format!("无法保存 Mojang jar 缓存: {}", target.display()))?;
    Ok(())
}

struct ProgressWriter<W> {
    label: String,
    inner: W,
    total: u64,
    written: u64,
    started: Instant,
    last_draw: Instant,
    last_width: usize,
}

impl<W: Write> ProgressWriter<W> {
    fn new(label: &str, inner: W, total: u64) -> Self {
        let now = Instant::now();
        let mut this = Self {
            label: format!("{label}.jar"),
            inner,
            total,
            written: 0,
            started: now,
            last_draw: now,
            last_width: 0,
        };
        this.draw(true);
        this
    }

    fn draw(&mut self, force: bool) {
        let now = Instant::now();
        if !force && now.duration_since(self.last_draw).as_millis() < 80 {
            return;
        }
        self.last_draw = now;
        let elapsed = now.duration_since(self.started).as_secs_f64().max(0.001);
        let speed = self.written as f64 / elapsed;
        let bar = render_bar(self.written, self.total);
        let body = if self.total > 0 {
            format!(
                "{:<12} {}  {} / {}  {}/s",
                self.label,
                bar,
                format_bytes(self.written),
                format_bytes(self.total),
                format_bytes(speed as u64)
            )
        } else {
            format!(
                "{:<12} {}  {}  {}/s",
                self.label,
                bar,
                format_bytes(self.written),
                format_bytes(speed as u64)
            )
        };
        let pad = self.last_width.saturating_sub(body.chars().count());
        let line = format!("\r{body}{}\x1b[K", " ".repeat(pad));
        self.last_width = body.chars().count();
        let mut stderr = std::io::stderr().lock();
        let _ = write!(stderr, "{line}");
        let _ = stderr.flush();
    }

    fn finish(&mut self) {
        self.draw(true);
        let _ = writeln!(std::io::stderr());
    }
}

impl<W: Write> Write for ProgressWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.written += n as u64;
        self.draw(false);
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

fn render_bar(written: u64, total: u64) -> String {
    const WIDTH: usize = 28;
    if total == 0 {
        return format!("[{}]", ".".repeat(WIDTH));
    }
    let filled = ((written.min(total) as f64 / total as f64) * WIDTH as f64).round() as usize;
    let filled = filled.min(WIDTH);
    let mut bar = String::from("[");
    for i in 0..WIDTH {
        if i < filled {
            bar.push('=');
        } else if i == filled {
            bar.push('>');
        } else {
            bar.push(' ');
        }
    }
    bar.push(']');
    bar
}

fn format_bytes(n: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    let n = n as f64;
    let (value, unit) = if n >= GIB {
        (n / GIB, "GiB")
    } else if n >= MIB {
        (n / MIB, "MiB")
    } else if n >= KIB {
        (n / KIB, "KiB")
    } else {
        (n, "B")
    };
    format!("{value:>6.1} {unit:<3}")
}

fn verify_sha1(path: &Path, expected: &str) -> Result<(), MojangDataError> {
    let mut file =
        File::open(path).io_ctx(format!("无法打开文件用于 SHA1 校验: {}", path.display()))?;
    let mut hasher = Sha1::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .io_ctx(format!("无法读取文件用于 SHA1 校验: {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    verify_digest(hasher.finalize().as_slice(), expected)
}

fn verify_digest(actual: &[u8], expected: &str) -> Result<(), MojangDataError> {
    let actual = hex::encode(actual);
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(MojangDataError::Sha1Mismatch {
            expected: expected.to_string(),
            actual,
        });
    }
    Ok(())
}
