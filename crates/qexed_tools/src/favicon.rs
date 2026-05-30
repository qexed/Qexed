use std::{fs, path::Path};

use anyhow::{Context, Result, anyhow};
use base64::Engine as _;
use toml_edit::value;

use crate::config_edit;

pub fn convert_to_data_uri(input: impl AsRef<Path>) -> Result<String> {
    let input = input.as_ref();
    let bytes = fs::read(input).with_context(|| format!("无法读取图片文件 {}", input.display()))?;
    if !is_png(&bytes) {
        anyhow::bail!("当前轻量转换器只接受 PNG 输入；请先导出 64x64 PNG");
    }

    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

pub fn write_data_uri(output: impl AsRef<Path>, data_uri: &str) -> Result<()> {
    let output = output.as_ref();
    fs::write(output, data_uri).with_context(|| format!("无法写入输出文件 {}", output.display()))
}

pub fn update_config(config: impl AsRef<Path>, data_uri: &str) -> Result<()> {
    let config = config.as_ref();
    let (config_path, mut doc) = config_edit::read_server_doc(config)?;
    let server = doc["server"]
        .as_table_mut()
        .context("配置文件缺少 [server] 表")?;
    server.insert("favicon", value(data_uri));
    config_edit::write_doc(&config_path, &doc)
}

fn is_png(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n")
}

pub fn decode_data_uri(data_uri: &str) -> Result<Vec<u8>> {
    let payload = data_uri
        .strip_prefix("data:image/png;base64,")
        .ok_or_else(|| anyhow!("不是 favicon PNG data URI"))?;
    base64::engine::general_purpose::STANDARD
        .decode(payload)
        .context("favicon base64 无效")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_png_to_data_uri() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("icon.png");
        fs::write(&path, b"\x89PNG\r\n\x1a\nabc").unwrap();

        let data_uri = convert_to_data_uri(&path).unwrap();
        assert!(data_uri.starts_with("data:image/png;base64,"));
        assert_eq!(decode_data_uri(&data_uri).unwrap(), b"\x89PNG\r\n\x1a\nabc");
    }

    #[test]
    fn updates_qexed_server_config() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("qexed.toml");
        let server_path = dir.path().join("qexed_server.toml");
        fs::write(&config, "version = 0\n").unwrap();
        fs::write(&server_path, "[server]\nfavicon = \"old\"\n").unwrap();

        update_config(&config, "data:image/png;base64,abc").unwrap();

        let main = fs::read_to_string(&config).unwrap();
        let server = fs::read_to_string(server_path).unwrap();
        assert!(!main.contains("data:image/png;base64,abc"));
        assert!(server.contains("favicon = \"data:image/png;base64,abc\""));
    }
}
