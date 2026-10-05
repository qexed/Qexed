pub mod config;
pub mod error;
pub mod service;

use qexed_config::Config;
use qexed_doc::{interpolate, DocField, DocSchema};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

/// 内置简体中文翻译（编译期嵌入，离线兜底：语言服务器与本地缓存都缺失时仍可显示中文）。
pub const ZH_CN_JSON: &str = include_str!("locales/zh-CN.json");

/// 内置英文翻译（编译期嵌入）：非 zh-CN 语言在缓存与远程都不可用时的兜底表，
/// 避免英文文档回退成整表中文。
pub const EN_JSON: &str = include_str!("locales/en.json");

/// 按语言选择编译期内嵌的兜底翻译 json：zh-CN 系用中文表，其余一律英文表。
pub fn embedded_json(language: &str) -> &'static str {
    match language {
        "zh-CN" | "zh" | "zh-Hans" => ZH_CN_JSON,
        _ => EN_JSON,
    }
}

/// 字段翻译键派生规则（autodoc 各标签 → 键空间）：
///
/// | autodoc 来源                    | 键形态              | 说明 |
/// |--------------------------------|---------------------|------|
/// | `<Name>k`（显示名）             | `k`                 | 字段短名 |
/// | `<Name>k` + `<Value>/<Default>`| `k.desc`            | 描述模板，`{default}`/`{variants}`/`<Value name=x>` 的执行值按 `{x}` 插值 |
/// | `<Select>`/枚举变体             | `k.{variant}`       | 候选值显示名 |
/// | `<CheckErrorTip>`              | `k.check_error`     | 校验失败提示，缺省用内置消息 |
/// | `<Tip>`/`<Warn>`               | 独立键或直接文本     | DocI18nText，不派生 |
///
/// 所有派生子键均可缺失：`t` 系查询未命中时回退（desc 缺 → 显示名，变体名缺 → 变体原文）。
pub mod key_layout {
    /// 字段显示名键。
    pub fn name(field_key: &str) -> String {
        field_key.to_string()
    }

    /// 字段描述模板键（`<Value>`/`<Default>` 的插值模板）。
    pub fn desc(field_key: &str) -> String {
        format!("{field_key}.desc")
    }

    /// 候选值/枚举变体显示名键。
    pub fn variant(field_key: &str, variant: &str) -> String {
        format!("{field_key}.{variant}")
    }

    /// 校验失败提示键。
    pub fn check_error(field_key: &str) -> String {
        format!("{field_key}.check_error")
    }
}

/// 一份语言的翻译表：key → 文本。
#[derive(Debug, Clone, Default)]
pub struct Translations {
    map: HashMap<String, String>,
}

impl Translations {
    /// 解析翻译 json（形如 { "key": "文本", ... }）。
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        Ok(Self { map: serde_json::from_str(json)? })
    }

    /// 查翻译；命中返回文本，未命中返回 None（调用方决定是否回退显示 key）。
    pub fn get(&self, key: &str) -> Option<&str> {
        self.map.get(key).map(String::as_str)
    }

    /// 查翻译，未命中回退返回 key 本身（与 LanguageConfig.server_url 缺失时的约定一致）。
    /// 返回 String：命中时克隆表内文本，未命中时归还 key，避免生命周期纠缠。
    pub fn t(&self, key: &str) -> String {
        self.map.get(key).cloned().unwrap_or_else(|| key.to_string())
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

static CURRENT: RwLock<Option<Translations>> = RwLock::new(None);

/// 全局翻译查询：查当前语言表（init 装载；缓存/远程来源），未命中回退 key 本身。
/// init 之前调用时懒加载内嵌 zh-CN 兜底。
pub fn t(key: &str) -> String {
    {
        let guard = CURRENT.read().unwrap();
        if let Some(table) = guard.as_ref() {
            return table.t(key);
        }
    }
    // init 前兜底：解析失败则置空表（查询全部回退 key），不重复尝试。
    let parsed = Translations::from_json(ZH_CN_JSON).unwrap_or_default();
    let text = parsed.t(key);
    *CURRENT.write().unwrap() = Some(parsed);
    text
}

/// 语言文件本地存储目录：`<config-path>/languages/`，目录布局与
/// qexed_language_server 的数据树同构（qexed/<commit>/...、<author>/<plugin>/<commit>/...），
/// 既作下载缓存，也可直接交给 qexed_language_server --root 提供服务。
pub fn storage_dir() -> Result<PathBuf, error::LanguageError> {
    let base = qexed_config::config_path()?;
    Ok(base.join("languages"))
}

/// 全局语言服务初始化。
///
/// * `commit` —— 本体构建 commit（主 crate 传 `shadow::SHORT_COMMIT`），决定语言文件版本路由。
/// * `language_override` —— `--language` 启动参数，覆盖配置文件里的 language。
///
/// 流程：加载配置 → 解析目标语言 → 缓存命中/远程拉取本体翻译 → 装全局表。
/// 全源失败时以内嵌 zh-CN 兜底，进程照常启动。
pub async fn init(
    commit: &str,
    language_override: Option<&str>,
) -> Result<(), error::LanguageError> {
    let config = config::LanguageConfig::load_and_create_default(true)?;
    let language = language_override
        .map(str::to_string)
        .unwrap_or_else(|| config.language.clone());
    service::run(&config, commit, &language).await
}

/// 按语言加载本体翻译表（不改动全局表）：缓存 → 远程 → 内嵌 zh-CN 兜底。
/// 离线工具用（如 doc_to_mdx 生成多语言文档）。
pub async fn load_translations(
    commit: &str,
    language: &str,
) -> Result<Translations, error::LanguageError> {
    let config = config::LanguageConfig::load_and_create_default(false)?;
    Ok(service::load_translations(&config, commit, language).await)
}

/// 按需拉取插件翻译（插件系统加载插件后调用），写入本地缓存并返回翻译表。
/// 三级回退：plugin_server_url[plugin] → plugin_author_server_url[author] → server_url。
pub async fn ensure_plugin_translations(
    author: &str,
    plugin: &str,
    commit: &str,
    language: &str,
) -> Result<Translations, error::LanguageError> {
    let config = config::LanguageConfig::load_and_create_default(false)?;
    let cache = service::plugin_cache_path(author, plugin, commit, language)?;
    let body = service::fetch_plugin(&config, author, plugin, commit, language);
    service::ensure_translations(cache, body).await
}

// ---------------------------------------------------------------------------
// DocSchema / DocField 汉化
// ---------------------------------------------------------------------------
//
// qexed_doc 只描述结构（key + values），不携带译文；渲染需要「翻译表 + %{name}
// 插值」。这里把 t(key) 的结果按 values 插值后写回 document 字段，递归覆盖 sub。
//
// 依赖方向：qexed_language → qexed_doc（单向），不会与 qexed_doc 形成环。

/// 用全局翻译表（`t`）汉化一份 schema（就地修改）。
pub fn translate_schema(schema: &mut DocSchema) {
    translate_schema_with(schema, &t);
}

/// 用指定翻译函数汉化一份 schema（就地修改）。
/// 供测试与离线工具注入自定义 `Translations`。
pub fn translate_schema_with<F>(schema: &mut DocSchema, t: &F)
where
    F: Fn(&str) -> String,
{
    // 结构体级：t(key) + values 插值。key 为空串时（无 <Name> 块）保留 None。
    schema.document = if schema.key.is_empty() {
        None
    } else {
        Some(interpolate(&t(&schema.key), &schema.values))
    };

    for field in &mut schema.fields {
        translate_field_with(field, t);
    }
}

/// 用全局翻译表汉化单个字段（递归处理 sub）。
pub fn translate_field(field: &mut DocField) {
    translate_field_with(field, &t);
}

/// 用指定翻译函数汉化单个字段（递归处理 sub）。
pub fn translate_field_with<F>(field: &mut DocField, t: &F)
where
    F: Fn(&str) -> String,
{
    // 字段显示名：key 为空时不翻译。
    field.document = if field.key.is_empty() {
        None
    } else {
        Some(interpolate(&t(&field.key), &field.values))
    };

    // 递归处理嵌套字段。
    if let Some(sub) = field.sub.as_mut() {
        translate_schema_with(sub, t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_zh_cn_parses_and_translates() {
        let table = Translations::from_json(ZH_CN_JSON).unwrap();
        assert!(!table.is_empty());
        assert_eq!(table.get("qexed.server.started"), Some("Qexed 服务器已启动，监听 %{addr}"));
        // 回退语义:未命中原样返回 key
        assert_eq!(table.t("qexed.not.exist.key"), "qexed.not.exist.key");
    }

    #[test]
    fn derived_key_layout_hits_embedded() {
        let table = Translations::from_json(ZH_CN_JSON).unwrap();
        let k = "qexed.crates.log.config.LogConfig.level";
        // 显示名
        assert_eq!(table.t(&key_layout::name(k)), "日志级别");
        // 描述模板（<Value name="system">/<Default> 的插值宿主）
        assert_eq!(
            table.t(&key_layout::desc(k)),
            "日志级别，默认 %{default}，可选：%{variants}"
        );
        // 变体显示名
        assert_eq!(table.t(&key_layout::variant(k, "Info")), "信息");
        // 校验失败提示
        assert!(table.get(&key_layout::check_error(k)).is_some());
        // 回退链:未派生的字段 desc 缺失 → 调用方回退显示名
        assert!(table.get(&key_layout::desc("qexed.nope.field")).is_none());
        assert_eq!(table.t(&key_layout::name("qexed.nope.field")), "qexed.nope.field");
    }

    #[test]
    fn desc_template_interpolates_values() {
        // <Value name="system"> 执行值 + <Default> 渲染进 desc 模板
        let table = Translations::from_json(ZH_CN_JSON).unwrap();
        let template = table.t(&key_layout::desc("qexed.crates.log.config.LogConfig.level"));
        let out = template
            .replace("%{default}", "Info")
            .replace("%{variants}", "追踪/调试/信息/警告/错误/关闭");
        assert_eq!(out, "日志级别，默认 Info，可选：追踪/调试/信息/警告/错误/关闭");
    }

    #[test]
    fn global_t_falls_back_to_key() {
        assert_eq!(t("qexed.server.stopping"), "Qexed 服务器正在关闭");
        assert_eq!(t("totally.unknown"), "totally.unknown");
    }

    // ---- DocSchema / DocField 汉化 -----------------------------------------

    /// 用内嵌 zh-CN 构造只读翻译函数，避免依赖全局 CURRENT 状态。
    fn embedded_t(key: &str) -> String {
        Translations::from_json(ZH_CN_JSON).unwrap().t(key)
    }

    fn sample_schema() -> DocSchema {
        DocSchema {
            name: "LogConfig".into(),
            key: "qexed.crates.log.config.LogConfig".into(),
            secrets: vec![],
            secret_placeholder: String::new(),
            values: Default::default(),
            fields: vec![DocField {
                path: "level".into(),
                key: "qexed.crates.log.config.LogConfig.level".into(),
                value_type: "LogLevel".into(),
                writable: true,
                values: [
                    ("default".to_string(), serde_json::json!("Info")),
                    (
                        "variants".to_string(),
                        serde_json::json!("Trace/Debug/Info/Warn/Error/Off"),
                    ),
                ]
                .into_iter()
                .collect(),
                default: Some(serde_json::json!("Info")),
                variants: vec!["Trace".into(), "Info".into()],
                check: None,
                check_error_tip: None,
                sub: None,
                aliases: vec![],
                optional: false,
                flattened: false,
                select: vec![],
                min: None,
                max: None,
                password: false,
                tip: None,
                warn: None,
                document: None,
            }],
            document: None,
        }
    }

    #[test]
    fn translate_schema_fills_struct_and_field_documents() {
        let mut schema = sample_schema();
        translate_schema_with(&mut schema, &embedded_t);

        // 结构体级 document
        assert_eq!(schema.document.as_deref(), Some("日志"));

        // 字段级 document：命中显示名（key 本身无 %{...} 占位符）
        assert_eq!(schema.fields[0].document.as_deref(), Some("日志级别"));
    }

    #[test]
    fn translate_schema_interpolates_field_values() {
        // 直接构造一个 key 落在 `.desc` 上的字段，验证 %{default}/%{variants} 插值。
        let mut schema = sample_schema();
        schema.fields[0].key =
            "qexed.crates.log.config.LogConfig.level.desc".to_string();

        translate_schema_with(&mut schema, &embedded_t);

        assert_eq!(
            schema.fields[0].document.as_deref(),
            // interpolate 对字符串值原样插值，不加引号（与 desc_template_interpolates_values 一致）。
            Some("日志级别，默认 Info，可选：Trace/Debug/Info/Warn/Error/Off"),
        );
    }

    #[test]
    fn translate_schema_recurses_into_sub() {
        // 给字段挂一个 sub schema，验证递归回填。
        let mut schema = sample_schema();
        let mut sub = sample_schema();
        sub.key = "qexed.crates.language.config.LanguageConfig".into();
        sub.fields[0].key = "qexed.crates.language.config.LanguageConfig.enable".into();
        sub.fields[0].values.clear();
        schema.fields[0].sub = Some(Box::new(sub));

        translate_schema_with(&mut schema, &embedded_t);

        let sub = schema.fields[0].sub.as_ref().unwrap();
        assert_eq!(sub.document.as_deref(), Some("语言"));
        assert_eq!(sub.fields[0].document.as_deref(), Some("启用"));
    }

    #[test]
    fn translate_schema_skips_empty_key() {
        // 无 <Name> 块：结构体 key 空串 → document 保持 None。
        let mut schema = sample_schema();
        schema.key = String::new();
        translate_schema_with(&mut schema, &embedded_t);
        assert!(schema.document.is_none());
    }
}