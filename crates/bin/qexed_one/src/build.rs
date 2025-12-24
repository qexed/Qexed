use std::env;
use std::process::Command;
use vergen::EmitBuilder;

fn main() {
    // 注入构建信息
    EmitBuilder::builder()
        .build_timestamp()
        .git_sha(true)
        .git_branch()
        .emit()
        .unwrap();
    
    // 获取构建时间
    let build_time = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    println!("cargo:rustc-env=CARGO_BUILD_TIMESTAMP={}", build_time);
    
    // 尝试获取Git信息（如果可用）
    if let Ok(output) = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
    {
        if output.status.success() {
            let commit_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
            println!("cargo:rustc-env=GIT_COMMIT_HASH={}", commit_hash);
        }
    }
}