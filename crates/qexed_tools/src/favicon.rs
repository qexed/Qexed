use std::{fs, path::Path};

use anyhow::{Context, Result, anyhow};
use base64::Engine as _;
use toml_edit::{DocumentMut, value};

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
    let content = fs::read_to_string(config)
        .with_context(|| format!("无法读取配置文件 {}", config.display()))?;
    let mut doc = content
        .parse::<DocumentMut>()
        .with_context(|| format!("配置文件不是合法 TOML: {}", config.display()))?;
    let server = doc["server"]
        .as_table_mut()
        .context("配置文件缺少 [server] 表")?;
    server.insert("favicon", value(data_uri));
    fs::write(config, doc.to_string())
        .with_context(|| format!("无法写入配置文件 {}", config.display()))
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
}
