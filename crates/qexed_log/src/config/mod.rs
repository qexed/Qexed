use qexed_doc_macros::Doc;
use serde::{Deserialize, Serialize};

pub mod loglevel;
pub mod mode;

use self::loglevel::LogLevel;
/// ```autodoc
/// <Name>qexed.crates.log.config.LogConfig</Name>
/// <Attr name="writable" />
/// ```
#[qexed_config_macros::app_config("/", "log")]
#[derive(Debug, Serialize, Deserialize, Doc)]
pub struct LogConfig {
    /// ```autodoc
    /// <Name>qexed.crates.log.config.LogConfig.level</Name>
    /// <Value name="system">LogLevel::default()</Value>
    /// <Default>Info</Default>
    /// ```
    pub level: loglevel::LogLevel,
    /// ```autodoc
    /// <Name>qexed.crates.log.config.LogConfig.filename</Name>
    /// ```
    pub filename: String,
    /// ```autodoc
    /// <Name>qexed.crates.log.config.LogConfig.mode</Name>
    /// ```
    pub mode: mode::MODE,
    /// ```autodoc
    /// <Name>qexed.crates.log.config.LogConfig.maxbackups</Name>
    /// ```
    pub maxbackups: u32,
    /// ```autodoc
    /// <Name>qexed.crates.log.config.LogConfig.compress</Name>
    /// ```
    pub compress: bool,
    /// ```autodoc
    /// <Name>qexed.crates.log.config.LogConfig.json</Name>
    /// ```
    pub json: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: loglevel::LogLevel::default(),
            filename: "./log/qexed.log".to_string(),
            mode: mode::MODE::default(),
            maxbackups: 30,
            compress: true,
            json: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tip_and_warn_kinds() {
        // 用户示例:json 字段 <Warn> 是 i18n 键
        let schema = LogConfig::schema();
        let json = schema.fields.iter().find(|f| f.path == "json").unwrap();
        // json 字段当前未声明 <Warn>/<Tip>：均为 None
        assert!(json.warn.is_none());
        assert!(json.tip.is_none());
        // 分类器:直接文本 / 前缀覆盖 / 纯键
        let text = qexed_doc::classify_i18n_text("开启后日志输出为 JSON 格式，便于采集");
        assert_eq!(text.kind, qexed_doc::I18nTextKind::Text);
        assert_eq!(text.value, "开启后日志输出为 JSON 格式，便于采集");
        let forced = qexed_doc::classify_i18n_text("text:qexed.looks.like.key");
        assert_eq!(forced.kind, qexed_doc::I18nTextKind::Text);
        let key = qexed_doc::classify_i18n_text("qexed.log.some.key");
        assert_eq!(key.kind, qexed_doc::I18nTextKind::I18n);
        // schema JSON 可解析且字段齐全
        let parsed: qexed_doc::serde_json::Value =
            qexed_doc::serde_json::from_str(&LogConfig::schema_json()).unwrap();
        assert_eq!(parsed["fields"].as_array().unwrap().len(), 6);
        // DocI18nText 序列化形态（kind 小写）由 qexed_doc 序列化约定保证：
        let sample = qexed_doc::classify_i18n_text("qexed.some.key");
        let encoded = qexed_doc::serde_json::to_string(&sample).unwrap();
        assert_eq!(encoded, "{\"kind\":\"i18n\",\"value\":\"qexed.some.key\"}");
    }
}
