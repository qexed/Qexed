//! 最小 docker 执行器：不依赖任何 docker SDK crate，
//! 直接拼 `docker` CLI 参数，统一 linux 路径约定。
//!
//! 挂载约定：
//! - 宿主工作区根 -> `/workspace`（只读）：snippet 的 path 依赖直接命中真实 crate；
//! - 物化目录（target/doc-verify）-> `/doc-verify`（可写）：cargo 的构建与产物落这里。

use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result};

/// 宿主工作区根的挂载路径。Windows 版 docker CLI 只认盘符形式
/// （E:/code/...），/e/code/... 会被当成字面根路径挂载出空目录。
pub fn host_workspace_root() -> String {
    // CARGO_MANIFEST_DIR = <root>/crates/qexed_doc_verify，上两级是工作区根
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.parent().and_then(Path::parent).unwrap_or(manifest);
    to_host_mount(&root.to_path_buf())
}

fn to_host_mount(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// 容器内工作区挂载点。
pub const CONTAINER_WORKSPACE: &str = "/workspace";

/// `docker run --rm`：返回 (退出码, 合并输出尾部)。
/// 不用 login shell（-l）：/etc/profile 会重置 PATH，丢掉镜像 ENV 里的 cargo 路径。
/// stdout/stderr 都必须排空，否则子进程写满管道缓冲区会死锁。
pub fn docker_run(image: &str, host_dir: &Path, workdir: &str, command: &str) -> Result<(i32, String)> {
    let host = host_dir.to_string_lossy().replace('\\', "/");
    let mut child = std::process::Command::new("docker")
        .args([
            "run",
            "--rm",
            "-v",
            &format!("{}:{CONTAINER_WORKSPACE}:ro", host_workspace_root()),
            "-v",
            &format!("{host}:/doc-verify"),
            "--tmpfs",
            "/tmp:rw,size=512m",
            "-w",
            workdir,
            image,
            "bash",
            "-c",
            command,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("启动 docker 失败（daemon 未运行或 docker 不在 PATH）")?;
    let mut out = child.stdout.take().unwrap();
    let mut err = child.stderr.take().unwrap();
    let h1 = std::thread::spawn(move || {
        use std::io::Read;
        let mut buf = String::new();
        let _ = out.read_to_string(&mut buf);
        buf
    });
    let h2 = std::thread::spawn(move || {
        use std::io::Read;
        let mut buf = String::new();
        let _ = err.read_to_string(&mut buf);
        buf
    });
    let status = child.wait().context("等待 docker 退出失败")?;
    let mut combined = h1.join().unwrap_or_default();
    combined.push_str(&h2.join().unwrap_or_default());
    Ok((status.code().unwrap_or(-1), combined))
}

/// 确保镜像存在，缺失时 docker pull。
pub fn ensure_image(image: &str) -> Result<()> {
    let ok = std::process::Command::new("docker")
        .args(["image", "inspect", image])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("无法执行 docker image inspect")?
        .success();
    if ok {
        return Ok(());
    }
    println!("拉取镜像 {image} ...");
    let status = std::process::Command::new("docker")
        .args(["pull", image])
        .status()
        .context("docker pull 失败")?;
    if !status.success() {
        anyhow::bail!("docker pull {image} 失败");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_workspace_root_points_at_repo_root() {
        let root = host_workspace_root();
        assert!(!root.contains("/crates"), "workspace root 误指到 crates: {root}");
        let ok = root.ends_with("qexed-v6") || root.ends_with("Qexed") || root.ends_with("qexed");
        assert!(ok, "workspace root 不对: {root}");
    }

    #[test]
    fn host_mount_path_keeps_drive_letter() {
        let p = to_host_mount(Path::new("E:/code/qexed-v6"));
        assert_eq!(p, "E:/code/qexed-v6");
        let q = to_host_mount(Path::new("E:\\code\\qexed-v6"));
        assert_eq!(q, "E:/code/qexed-v6");
    }
}
