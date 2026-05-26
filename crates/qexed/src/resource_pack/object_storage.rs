use qexed_config::app::qexed::server::ResourcePack;

use super::url::join_url_path;

pub(super) fn object_storage_download_url(config: &ResourcePack) -> Option<String> {
    let storage = &config.object_storage;
    let object_key = storage.object_key.trim().trim_start_matches('/');
    if object_key.is_empty() {
        return None;
    }

    let public_base_url = storage.public_base_url.trim();
    if !public_base_url.is_empty() {
        return Some(join_url_path(public_base_url, object_key));
    }

    let endpoint = storage.endpoint.trim();
    let bucket = storage.bucket.trim();
    if endpoint.is_empty() || bucket.is_empty() {
        return None;
    }

    let endpoint = endpoint_with_scheme(endpoint);
    if storage.force_path_style {
        Some(join_url_path(
            &join_url_path(&endpoint, bucket.trim_matches('/')),
            object_key,
        ))
    } else {
        Some(join_url_path(
            &virtual_host_endpoint(&endpoint, bucket),
            object_key,
        ))
    }
}

fn endpoint_with_scheme(endpoint: &str) -> String {
    if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        endpoint.trim_end_matches('/').to_string()
    } else {
        format!("https://{}", endpoint.trim_end_matches('/'))
    }
}

fn virtual_host_endpoint(endpoint: &str, bucket: &str) -> String {
    if let Some(rest) = endpoint.strip_prefix("https://") {
        format!("https://{bucket}.{}", rest.trim_start_matches('/'))
    } else if let Some(rest) = endpoint.strip_prefix("http://") {
        format!("http://{bucket}.{}", rest.trim_start_matches('/'))
    } else {
        format!("https://{bucket}.{}", endpoint.trim_start_matches('/'))
    }
}
