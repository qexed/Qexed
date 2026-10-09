use qexed_config::Config;
use qexed_server::config::ServerConfig;

fn fresh_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("server_cfg_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn nested_play_loads_default() {
    let dir = fresh_dir("load");
    let cfg = ServerConfig::load_and_create_default_at(&dir, false).unwrap();
    assert_eq!(cfg.bind, "0.0.0.0:25565");
    assert_eq!(cfg.play.view_distance, 8);
    assert_eq!(cfg.play.dimension_name, "minecraft:overworld");
}

#[test]
fn nested_play_roundtrip() {
    let dir = fresh_dir("round");
    let mut cfg = ServerConfig::default();
    cfg.play.view_distance = 32;
    cfg.motd = "hello".to_string();
    ServerConfig::save_file_at(&dir, &cfg).unwrap();
    let back = ServerConfig::load_file_at(&dir, false).unwrap();
    assert_eq!(back.play.view_distance, 32);
    assert_eq!(back.motd, "hello");
}

#[test]
fn nested_play_partial_file_fills_defaults() {
    let dir = fresh_dir("partial");
    std::fs::write(
        dir.join("server.toml"),
        "bind = \"1.2.3.4:1\"\nmax_players = 99\n",
    )
    .unwrap();
    let cfg = ServerConfig::load_file_at(&dir, false).unwrap();
    assert_eq!(cfg.bind, "1.2.3.4:1");
    assert_eq!(cfg.max_players, 99);
    assert_eq!(cfg.play.view_distance, 8, "missing play section gets defaults");
}
