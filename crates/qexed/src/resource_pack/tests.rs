use qexed_config::app::qexed::server::{ResourcePack, ResourcePackSource};

use super::{
    ResourcePackManager,
    local::{LocalResourcePack, request_path, sha1_hex},
    object_storage::object_storage_download_url,
    url::{local_download_url, resource_pack_route},
};

#[test]
fn local_url_uses_configured_host_and_bound_port() {
    let mut config = ResourcePack {
        enable: true,
        source: ResourcePackSource::Local,
        ..ResourcePack::default()
    };
    config.download_host = "example.org".to_string();
    let local = LocalResourcePack {
        route: resource_pack_route(config.id),
        bind: "127.0.0.1:0".parse().unwrap(),
        public_port: 25566,
        hash: String::new(),
        bytes: std::sync::Arc::new(Vec::new()),
    };

    assert_eq!(
        local_download_url(&config, "ignored", &local),
        format!("http://example.org:25566/resource-pack/{}.zip", config.id)
    );
}

#[test]
fn public_download_url_keeps_scheme_and_omits_bound_port() {
    let mut config = ResourcePack {
        enable: true,
        source: ResourcePackSource::Local,
        ..ResourcePack::default()
    };
    config.download_host = "https://cdn.example.org/packs".to_string();
    let local = LocalResourcePack {
        route: resource_pack_route(config.id),
        bind: "127.0.0.1:0".parse().unwrap(),
        public_port: 25566,
        hash: String::new(),
        bytes: std::sync::Arc::new(Vec::new()),
    };

    assert_eq!(
        local_download_url(&config, "ignored", &local),
        format!(
            "https://cdn.example.org/packs/resource-pack/{}.zip",
            config.id
        )
    );
}

#[test]
fn object_storage_url_prefers_public_base_url_for_cdn_and_edgeone() {
    let mut config = ResourcePack {
        enable: true,
        source: ResourcePackSource::ObjectStorage,
        ..ResourcePack::default()
    };
    config.object_storage.public_base_url = "https://packs.example.com/cache/".to_string();
    config.object_storage.object_key = "/minecraft/server.zip".to_string();

    assert_eq!(
        object_storage_download_url(&config).as_deref(),
        Some("https://packs.example.com/cache/minecraft/server.zip")
    );
}

#[test]
fn object_storage_url_builds_virtual_host_endpoint() {
    let mut config = ResourcePack {
        enable: true,
        source: ResourcePackSource::ObjectStorage,
        ..ResourcePack::default()
    };
    config.object_storage.endpoint = "obs.cn-north-4.myhuaweicloud.com".to_string();
    config.object_storage.bucket = "qexed-pack".to_string();
    config.object_storage.object_key = "resourcepacks/server.zip".to_string();

    assert_eq!(
        object_storage_download_url(&config).as_deref(),
        Some("https://qexed-pack.obs.cn-north-4.myhuaweicloud.com/resourcepacks/server.zip")
    );
}

#[test]
fn object_storage_url_builds_path_style_endpoint() {
    let mut config = ResourcePack {
        enable: true,
        source: ResourcePackSource::ObjectStorage,
        ..ResourcePack::default()
    };
    config.object_storage.endpoint = "https://cos.ap-guangzhou.myqcloud.com".to_string();
    config.object_storage.bucket = "qexed-1250000000".to_string();
    config.object_storage.object_key = "resourcepacks/server.zip".to_string();
    config.object_storage.force_path_style = true;

    assert_eq!(
        object_storage_download_url(&config).as_deref(),
        Some("https://cos.ap-guangzhou.myqcloud.com/qexed-1250000000/resourcepacks/server.zip")
    );
}

#[test]
fn request_path_accepts_origin_and_absolute_form_targets() {
    let request = "GET /resource-pack/test.zip HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
    assert_eq!(request_path(request), Some("/resource-pack/test.zip"));

    let request = "GET http://127.0.0.1:25566/resource-pack/test.zip?cache=1 HTTP/1.1\r\n\r\n";
    assert_eq!(request_path(request), Some("/resource-pack/test.zip"));
}

#[tokio::test]
async fn local_server_returns_pack_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let pack_path = dir.path().join("server.zip");
    tokio::fs::write(&pack_path, b"zip-bytes").await.unwrap();

    let mut config = ResourcePack {
        enable: true,
        source: ResourcePackSource::Local,
        ..ResourcePack::default()
    };
    config.path = pack_path.to_string_lossy().to_string();
    config.download_bind = "127.0.0.1:0".to_string();

    let mut manager = ResourcePackManager::from_config(&config).await.unwrap();
    manager.start().await.unwrap();
    let offer = manager.offer(&config, "127.0.0.1").unwrap();

    let http = reqwest::Client::builder().no_proxy().build().unwrap();
    let response = http.get(&offer.url).send().await.unwrap();
    let status = response.status();
    let body = response.bytes().await.unwrap();
    assert_eq!(status, reqwest::StatusCode::OK);
    assert_eq!(body, "zip-bytes");
    assert_eq!(offer.hash, sha1_hex(b"zip-bytes"));
}
