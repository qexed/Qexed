use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use std::env;
use log::warn;

// 定义 GitHub Release API 响应结构
#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,// 版本标签（如 "v0.1.0"）
}

pub async fn check_version() {
    // 创建带超时的 HTTP 客户端（5秒超时）
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .user_agent("Qexed-Updater/1.0")  // GitHub API 要求 User-Agent
        .build()
        .expect("Failed to create HTTP client");

    // GitHub API URL（公共仓库无需认证）
    let url = "https://api.github.com/repos/qexed/qexed/releases/latest";
    
    match client.get(url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                // 解析 JSON 响应
                match response.json::<GitHubRelease>().await {
                    Ok(release) => {
                        let local_version = env!("CARGO_PKG_VERSION");
                        
                        // 统一版本格式（移除可能的 'v' 前缀）
                        let remote_version = release.tag_name.trim_start_matches('v');
                        let local_version_clean = local_version.trim_start_matches('v');
                        
                        if remote_version != local_version_clean {
                            warn!(
                                "发现新版本: {} (当前: {})", 
                                release.tag_name, 
                                local_version
                            );
                        } else {
                            // 可选：记录已是最新版本
                            // warn!("已是最新版本: {}", local_version);
                        }
                    }
                    Err(e) => warn!("解析版本信息失败: {}", e),
                }
            } else if response.status() == reqwest::StatusCode::NOT_FOUND {
                warn!("仓库尚未发布任何正式版本（404 Not Found）");
            } else {
                warn!("GitHub API 返回错误状态码: {}", response.status());
            }
        }
        Err(e) => warn!("版本检查失败: {}", e),
    }
}