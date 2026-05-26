pub(super) fn join_url_path(base: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

pub(super) fn local_download_url(
    config: &qexed_config::app::qexed::server::ResourcePack,
    login_host: &str,
    local: &super::local::LocalResourcePack,
) -> String {
    let host = config.download_host.trim();
    if host.starts_with("http://") || host.starts_with("https://") {
        let base = host.trim_end_matches('/');
        return format!("{base}{}", local.route);
    }

    let host = if host.is_empty() {
        sanitize_login_host(login_host)
    } else {
        host.to_string()
    };

    let host = format_url_host(&host);
    if host_has_explicit_port(&host) {
        format!("http://{host}{}", local.route)
    } else {
        format!("http://{}:{}{}", host, local.public_port, local.route)
    }
}

fn sanitize_login_host(host: &str) -> String {
    let host = host.trim().trim_end_matches('.');
    if host.is_empty() || host == "0.0.0.0" || host == "::" {
        "127.0.0.1".to_string()
    } else {
        host.to_string()
    }
}

fn format_url_host(host: &str) -> String {
    if host.contains(':') && !host.starts_with('[') {
        let colon_count = host.chars().filter(|ch| *ch == ':').count();
        if colon_count == 1 {
            host.to_string()
        } else {
            format!("[{host}]")
        }
    } else {
        host.to_string()
    }
}

fn host_has_explicit_port(host: &str) -> bool {
    if let Some(rest) = host.strip_prefix('[') {
        return rest.contains("]:");
    }

    let Some((_, port)) = host.rsplit_once(':') else {
        return false;
    };
    port.chars().all(|ch| ch.is_ascii_digit())
}

pub(super) fn resource_pack_route(id: uuid::Uuid) -> String {
    format!("/resource-pack/{id}.zip")
}
