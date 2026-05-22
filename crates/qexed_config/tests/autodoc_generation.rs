use qexed_config::{
    app::{
        qexed::Qexed, qexed_ip_connection_speed_test::QexedIpConnectionSpeedTest,
        qexed_warden::QexedWarden,
    },
    tool::AppConfigTrait,
};

fn temp_config_dir(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "qexed_config_{}_{}_{}",
        name,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("系统时间不能早于 UNIX_EPOCH")
            .as_nanos()
    ));
    path
}

#[test]
fn generated_configs_include_autodoc_comments() -> anyhow::Result<()> {
    let dir = temp_config_dir("autodoc");

    Qexed::load_or_create_default(Some("zh-CN".to_string()), Some(true), Some(dir.clone()))?;
    QexedWarden::load_or_create_default(Some("zh-CN".to_string()), Some(true), Some(dir.clone()))?;
    QexedIpConnectionSpeedTest::load_or_create_default(
        Some("zh-CN".to_string()),
        Some(true),
        Some(dir.clone()),
    )?;

    let qexed = std::fs::read_to_string(dir.join("qexed.toml"))?;
    assert!(qexed.contains("# ==== AutoDocHeader ===="));
    assert!(qexed.contains("服务器监听地址"));
    assert!(qexed.contains("插件下载设置"));
    assert!(!qexed.contains("# config.qexed."));

    let warden = std::fs::read_to_string(dir.join("qexed_warden.toml"))?;
    assert!(warden.contains("典狱长数据存储设置"));
    assert!(warden.contains("MySQL 服务器地址"));
    assert!(warden.contains("MongoDB 服务器地址"));
    assert!(warden.contains("Pika 服务器地址"));
    assert!(!warden.contains("# config.qexed_warden."));

    let speed_test = std::fs::read_to_string(dir.join("qexed_ip_connect_speed_test.toml"))?;
    assert!(speed_test.contains("IP 连接速率检测"));
    assert!(speed_test.contains("Pika 服务器地址"));
    assert!(!speed_test.contains("# config.qexed_ip_connection_speed_test."));

    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}

#[test]
fn existing_config_keeps_single_autodoc_header() -> anyhow::Result<()> {
    let dir = temp_config_dir("autodoc_header");

    Qexed::load_or_create_default(Some("zh-CN".to_string()), Some(true), Some(dir.clone()))?;

    let qexed_path = dir.join("qexed.toml");
    let qexed = std::fs::read_to_string(&qexed_path)?;
    let duplicate_header = r#"# ==== AutoDocHeader ====
# 手动制造的重复 Header
# ==== AutoDocHeader ====
# 手动制造的重复 Header
"#;
    std::fs::write(&qexed_path, format!("{duplicate_header}{qexed}"))?;

    Qexed::load_or_create_default(Some("zh-CN".to_string()), Some(true), Some(dir.clone()))?;

    let qexed = std::fs::read_to_string(qexed_path)?;
    assert_eq!(qexed.matches("# ==== AutoDocHeader ====").count(), 1);
    assert!(qexed.contains("服务器监听地址"));

    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}
