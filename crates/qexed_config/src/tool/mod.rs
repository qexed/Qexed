use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use toml_edit::{DocumentMut, Item, RawString, Value, value};

use crate::build;

const SENSITIVE_DISPLAY_VALUE: &str = "<stored in .secrets>";

#[derive(Debug, Clone, Copy)]
pub struct SplitConfigFile {
    pub file_name: &'static str,
    pub root_path: &'static str,
}

#[derive(Debug, Clone)]
pub struct OwnedSplitConfigFile {
    pub file_name: String,
    pub root_path: String,
}

// ========================
// AutoDoc 元数据
// ========================
#[derive(Debug, Clone)]
struct AutoDocConfig {
    system_version_hash: String,
    system_lang: String,
    setting_lang: String,
    setting_enable: bool,
}

impl Default for AutoDocConfig {
    fn default() -> Self {
        Self {
            system_version_hash: build::COMMIT_HASH.to_string(),
            system_lang: "zh-CN".to_string(),
            setting_lang: String::new(),
            setting_enable: true,
        }
    }
}

// ========================
// 通用的 AutoDoc 行为 Trait
// ========================
pub trait AutoDocConfigTrait {
    fn doc_fields(lang: &str) -> Vec<(String, String)>;
    fn default_display_fields(lang: &str) -> Vec<(String, String)>;
    fn sensitive_fields() -> Vec<String>;
    fn deprecation_fields(lang: &str) -> Vec<(String, String)>;
    fn pending_deprecated_fields(lang: &str) -> Vec<(String, String)>;
    fn warning_fields(lang: &str) -> Vec<(String, String)>;
    fn migration_notice_fields(lang: &str) -> Vec<(String, String)>;
    fn danger_fields(lang: &str) -> Vec<(String, String)>;
}

// ========================
// 主配置 Trait
// ========================
pub trait AppConfigTrait:
    Serialize + for<'de> Deserialize<'de> + Default + Sized + AutoDocConfigTrait
{
    const PATH: &'static str;
    const NAME: &'static str;

    fn split_config_files() -> &'static [SplitConfigFile] {
        &[]
    }

    fn dynamic_split_config_files(
        _doc: &DocumentMut,
        _split_dir: &std::path::Path,
    ) -> Result<Vec<OwnedSplitConfigFile>> {
        Ok(Vec::new())
    }

    fn load_or_create_default(
        lang: Option<String>,
        enable_auto_doc: Option<bool>,
        config_path: Option<std::path::PathBuf>,
    ) -> Result<Self> {
        // ----- 名称校验 -----
        if !Self::NAME
            .chars()
            .all(|c| c.is_ascii_alphabetic() || c.is_ascii_digit() || c == '_')
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Invalid NAME: '{}' - only alphanumeric characters and underscores allowed",
                    Self::NAME
                ),
            )
            .into());
        }

        // ----- 路径构建 -----
        let base_dir = config_path.unwrap_or_else(|| std::path::PathBuf::from("./config"));
        let final_path = build_safe_path(&base_dir, Self::PATH)?;
        let path = final_path.join(Self::NAME).with_extension("toml");
        let secrets_path = secrets_path_for(&path)?;
        let split_dir = split_dir_for(&path)?;

        // ----- 文件不存在：创建全新配置 -----
        if !path.exists() {
            return Self::create_new_config(
                &path,
                &secrets_path,
                &split_dir,
                lang,
                enable_auto_doc,
            );
        }

        // ===== 文件存在：原地更新 =====
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("无法读取文件 {}", path.display()))?;
        let mut doc = content
            .parse::<DocumentMut>()
            .with_context(|| "TOML 格式错误")?;

        // 1. 提取当前文件中的 AutoDoc 元数据
        overlay_split_config_files::<Self>(&mut doc, &split_dir)?;
        let file_auto_doc = extract_auto_doc(&doc);
        let current_auto_doc = AutoDocConfig::default();

        // 2. 确定最终启用的语言和 enable 标志
        let effective_lang = lang
            .or(if !file_auto_doc.setting_lang.is_empty() {
                Some(file_auto_doc.setting_lang.clone())
            } else if !file_auto_doc.system_lang.is_empty() {
                Some(file_auto_doc.system_lang.clone())
            } else {
                None
            })
            .unwrap_or_else(|| "zh-CN".to_string());

        let effective_enable = enable_auto_doc.unwrap_or(file_auto_doc.setting_enable);

        // 3. 更新系统版本哈希（原地）
        doc.insert(
            "auto_doc_system_version_hash",
            value(&current_auto_doc.system_version_hash),
        );

        // 4. ✅ 原地补充缺失字段（不破坏顺序和注释）
        // 字段完整性以 Default 序列化出的 TOML 结构为准。
        // 不能用 serde 反序列化是否成功判断完整性，因为 #[serde(default)] 会掩盖缺失字段。
        let default_config = Self::default();
        let default_toml =
            toml::to_string_pretty(&default_config).with_context(|| "序列化默认配置失败")?;
        let default_doc = default_toml
            .parse::<DocumentMut>()
            .expect("默认 TOML 必须合法");

        let sensitive_fields = Self::sensitive_fields();
        let secrets_doc = read_secrets_doc(&secrets_path)?;

        merge_missing_default_items(doc.as_item_mut(), default_doc.as_item());
        let effective_secrets = migrate_sensitive_fields(&mut doc, secrets_doc, &sensitive_fields)?;
        overlay_missing_sensitive_defaults(
            &mut doc,
            &default_doc,
            &effective_secrets,
            &sensitive_fields,
        );
        apply_sensitive_display_values(&mut doc, &effective_secrets, &sensitive_fields);
        write_secrets_doc(&secrets_path, &effective_secrets)?;

        // 5. ✅ 更新字段注释（原地）
        if effective_enable {
            let doc_fields = Self::doc_fields(&effective_lang);
            let deprecation_fields = Self::deprecation_fields(&effective_lang);
            let pending_fields = Self::pending_deprecated_fields(&effective_lang);
            let warning_fields = Self::warning_fields(&effective_lang);
            let migration_fields = Self::migration_notice_fields(&effective_lang);
            let danger_fields = Self::danger_fields(&effective_lang);

            for (key, base_comment) in &doc_fields {
                // 检查嵌套字段是否存在
                if !item_exists_in_doc(&doc, key) {
                    continue;
                }

                let mut comment_parts = vec![base_comment.clone()];

                if let Some((_, text)) = danger_fields.iter().find(|(k, _)| k == key) {
                    comment_parts.push(
                        rust_i18n::t!("autodoc.danger", locale = effective_lang, text = text)
                            .to_string(),
                    );
                }
                if let Some((_, text)) = warning_fields.iter().find(|(k, _)| k == key) {
                    comment_parts.push(
                        rust_i18n::t!("autodoc.warning", locale = effective_lang, text = text)
                            .to_string(),
                    );
                }
                if let Some((_, text)) = pending_fields.iter().find(|(k, _)| k == key) {
                    comment_parts.push(
                        rust_i18n::t!(
                            "autodoc.pending_deprecated",
                            locale = effective_lang,
                            text = text
                        )
                        .to_string(),
                    );
                }
                if let Some((_, text)) = deprecation_fields.iter().find(|(k, _)| k == key) {
                    comment_parts.push(
                        rust_i18n::t!("autodoc.deprecated", locale = effective_lang, text = text)
                            .to_string(),
                    );
                }
                if let Some((_, text)) = migration_fields.iter().find(|(k, _)| k == key) {
                    comment_parts.push(
                        rust_i18n::t!("autodoc.migration", locale = effective_lang, text = text)
                            .to_string(),
                    );
                }

                update_autodoc_for_key(&mut doc, key, &comment_parts.join("\n\n"));
            }
        }

        // 6. ✅ 更新 Header（只保留一个）
        ensure_or_update_doc_header(&mut doc, &effective_lang);

        // 7. ✅ 写回文件
        write_config_documents::<Self>(&doc, &path, &split_dir)?;

        // 8. 反序列化
        let clean_doc = remove_auto_doc_fields(&doc);
        let mut config_doc = clean_doc.clone();
        overlay_sensitive_values(&mut config_doc, &effective_secrets, &sensitive_fields);
        let config: Self = toml::from_str(&config_doc.to_string())
            .with_context(|| "配置类型不匹配，可能字段已更改")?;

        Ok(config)
    }

    // 创建全新配置文件
    fn create_new_config(
        path: &std::path::Path,
        secrets_path: &std::path::Path,
        split_dir: &std::path::Path,
        lang: Option<String>,
        enable_auto_doc: Option<bool>,
    ) -> Result<Self> {
        let config = Self::default();
        let mut doc = DocumentMut::new();

        let mut auto_doc = AutoDocConfig::default();
        if let Some(l) = lang {
            auto_doc.system_lang = l;
        }
        if let Some(e) = enable_auto_doc {
            auto_doc.setting_enable = e;
        }

        insert_auto_doc_fields(&mut doc, &auto_doc);

        let user_toml = toml::to_string_pretty(&config).with_context(|| "序列化默认配置失败")?;
        let user_doc = user_toml
            .parse::<DocumentMut>()
            .expect("序列化后的 TOML 应合法");

        let sensitive_fields = Self::sensitive_fields();
        let mut secrets_doc = DocumentMut::new();

        for (k, v) in user_doc.iter() {
            if !k.starts_with("auto_doc_") {
                doc.insert(k, v.clone());
            }
        }
        move_sensitive_fields_to_secrets(&mut doc, &mut secrets_doc, &sensitive_fields);
        apply_sensitive_display_values(&mut doc, &secrets_doc, &sensitive_fields);

        let effective_lang = if !auto_doc.setting_lang.is_empty() {
            auto_doc.setting_lang.clone()
        } else {
            auto_doc.system_lang.clone()
        };

        let doc_fields = Self::doc_fields(&effective_lang);
        let deprecation_fields = Self::deprecation_fields(&effective_lang);
        let pending_fields = Self::pending_deprecated_fields(&effective_lang);
        let warning_fields = Self::warning_fields(&effective_lang);
        let migration_fields = Self::migration_notice_fields(&effective_lang);
        let danger_fields = Self::danger_fields(&effective_lang);

        for (key, base_comment) in doc_fields {
            // 检查嵌套字段是否存在
            if !item_exists_in_doc(&doc, &key) {
                continue;
            }

            let mut comment_parts = vec![base_comment];

            // 辅助闭包：在所有字段列表中找相同 key
            let find_text = |list: &[(String, String)]| {
                list.iter().find(|(k, _)| k == &key).map(|(_, v)| v.clone())
            };

            if let Some(text) = find_text(&danger_fields) {
                comment_parts.push(
                    rust_i18n::t!("autodoc.danger", locale = effective_lang, text = text)
                        .to_string(),
                );
            }

            if let Some(text) = find_text(&warning_fields) {
                comment_parts.push(
                    rust_i18n::t!("autodoc.warning", locale = effective_lang, text = text)
                        .to_string(),
                );
            }

            if let Some(text) = find_text(&pending_fields) {
                comment_parts.push(
                    rust_i18n::t!(
                        "autodoc.pending_deprecated",
                        locale = effective_lang,
                        text = text
                    )
                    .to_string(),
                );
            }

            if let Some(text) = find_text(&deprecation_fields) {
                comment_parts.push(
                    rust_i18n::t!("autodoc.deprecated", locale = effective_lang, text = text)
                        .to_string(),
                );
            }

            if let Some(text) = find_text(&migration_fields) {
                comment_parts.push(
                    rust_i18n::t!("autodoc.migration", locale = effective_lang, text = text)
                        .to_string(),
                );
            }

            let full_comment = comment_parts.join("\n\n");
            update_autodoc_for_key(&mut doc, &key, &full_comment);
        }

        ensure_or_update_doc_header(&mut doc, &effective_lang);

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        write_config_documents::<Self>(&doc, path, split_dir)?;
        write_secrets_doc(secrets_path, &secrets_doc)?;
        Ok(config)
    }
}

// ========================
// 辅助函数
// ========================

// 检查嵌套项是否存在于文档中
fn item_exists_in_doc(doc: &DocumentMut, key: &str) -> bool {
    let parts: Vec<&str> = key.split('.').collect();

    if parts.len() == 1 {
        return doc.contains_key(parts[0]);
    }

    get_item_by_path(doc, &parts).is_some()
}

// 根据路径获取 Item 引用
fn secrets_path_for(path: &std::path::Path) -> Result<std::path::PathBuf> {
    let file_name = path.file_name().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "config path has no file name",
        )
    })?;
    Ok(path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join(".secrets")
        .join(file_name))
}

fn split_dir_for(path: &std::path::Path) -> Result<std::path::PathBuf> {
    let stem = path
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "config path has no valid file stem",
            )
        })?;
    Ok(path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join(format!("{stem}.d")))
}

fn sync_split_config_files<T: AppConfigTrait>(
    doc: &DocumentMut,
    split_dir: &std::path::Path,
) -> Result<()> {
    let split_files = resolved_split_config_files::<T>(doc, split_dir)?;
    if split_files.is_empty() {
        return Ok(());
    }

    std::fs::create_dir_all(split_dir)?;
    for split_file in &split_files {
        let file_path = safe_split_file_path(split_dir, &split_file.file_name)?;
        let Some(item) = get_item_by_dotted_path(doc, &split_file.root_path) else {
            continue;
        };

        let mut split_doc = DocumentMut::new();
        set_item_by_dotted_path(&mut split_doc, &split_file.root_path, item.clone())?;
        prune_nested_split_config_items(&mut split_doc, &split_file.root_path, &split_files);
        std::fs::write(&file_path, split_doc.to_string())
            .with_context(|| format!("无法写入拆分配置文件 {}", file_path.display()))?;
    }

    Ok(())
}

fn write_config_documents<T: AppConfigTrait>(
    doc: &DocumentMut,
    path: &std::path::Path,
    split_dir: &std::path::Path,
) -> Result<()> {
    let mut main_doc = doc.clone();
    let split_files = resolved_split_config_files::<T>(&main_doc, split_dir)?;
    sync_split_config_files::<T>(&main_doc, split_dir)?;
    prune_split_config_items(&mut main_doc, &split_files);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, main_doc.to_string())?;
    Ok(())
}

fn overlay_split_config_files<T: AppConfigTrait>(
    doc: &mut DocumentMut,
    split_dir: &std::path::Path,
) -> Result<()> {
    let split_files = resolved_split_config_files::<T>(doc, split_dir)?;
    for split_file in split_files {
        let file_path = safe_split_file_path(split_dir, &split_file.file_name)?;
        if !file_path.exists() {
            continue;
        }

        let content = std::fs::read_to_string(&file_path)
            .with_context(|| format!("无法读取拆分配置文件 {}", file_path.display()))?;
        let split_doc = content
            .parse::<DocumentMut>()
            .with_context(|| format!("拆分配置文件 TOML 格式错误: {}", file_path.display()))?;
        overlay_config_items(doc.as_item_mut(), split_doc.as_item());
    }

    Ok(())
}

fn prune_split_config_items(doc: &mut DocumentMut, split_files: &[OwnedSplitConfigFile]) {
    for split_file in split_files {
        let _ = take_item_by_dotted_path(doc, &split_file.root_path);
    }
}

fn prune_nested_split_config_items(
    doc: &mut DocumentMut,
    root_path: &str,
    split_files: &[OwnedSplitConfigFile],
) {
    let child_prefix = format!("{root_path}.");
    for split_file in split_files {
        if split_file.root_path.starts_with(&child_prefix) {
            let _ = take_item_by_dotted_path(doc, &split_file.root_path);
        }
    }
}

fn resolved_split_config_files<T: AppConfigTrait>(
    doc: &DocumentMut,
    split_dir: &std::path::Path,
) -> Result<Vec<OwnedSplitConfigFile>> {
    let mut split_files = T::split_config_files()
        .iter()
        .map(|split| OwnedSplitConfigFile {
            file_name: split.file_name.to_string(),
            root_path: split.root_path.to_string(),
        })
        .collect::<Vec<_>>();
    split_files.extend(T::dynamic_split_config_files(doc, split_dir)?);
    split_files.sort_by(|left, right| left.root_path.cmp(&right.root_path));
    split_files.dedup_by(|left, right| left.root_path == right.root_path);
    Ok(split_files)
}

fn safe_split_file_path(
    split_dir: &std::path::Path,
    file_name: &str,
) -> Result<std::path::PathBuf> {
    let file_path = std::path::Path::new(file_name);
    if file_path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid split config file extension: {file_name}"),
        )
        .into());
    }
    for component in file_path.components() {
        match component {
            std::path::Component::Normal(_) => {}
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("invalid split config file path: {file_name}"),
                )
                .into());
            }
        }
    }

    Ok(split_dir.join(file_path))
}

fn read_secrets_doc(path: &std::path::Path) -> Result<DocumentMut> {
    if !path.exists() {
        return Ok(DocumentMut::new());
    }

    let content = std::fs::read_to_string(path)
        .with_context(|| format!("无法读取敏感配置文件 {}", path.display()))?;
    content
        .parse::<DocumentMut>()
        .with_context(|| format!("敏感配置文件 TOML 格式错误: {}", path.display()))
}

fn write_secrets_doc(path: &std::path::Path, doc: &DocumentMut) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, doc.to_string())?;
    Ok(())
}

fn migrate_sensitive_fields(
    doc: &mut DocumentMut,
    mut secrets_doc: DocumentMut,
    sensitive_fields: &[String],
) -> Result<DocumentMut> {
    for path in sensitive_fields {
        if let Some(item) = take_item_by_dotted_path(doc, path)? {
            if is_sensitive_display_item(&item)
                && get_item_by_dotted_path(&secrets_doc, path).is_some()
            {
                continue;
            }
            set_item_by_dotted_path(&mut secrets_doc, path, item)?;
        }
    }
    Ok(secrets_doc)
}

fn move_sensitive_fields_to_secrets(
    doc: &mut DocumentMut,
    secrets_doc: &mut DocumentMut,
    sensitive_fields: &[String],
) {
    for path in sensitive_fields {
        if let Ok(Some(item)) = take_item_by_dotted_path(doc, path) {
            let _ = set_item_by_dotted_path(secrets_doc, path, item);
        }
    }
}

fn overlay_missing_sensitive_defaults(
    doc: &mut DocumentMut,
    defaults: &DocumentMut,
    secrets_doc: &DocumentMut,
    sensitive_fields: &[String],
) {
    for path in sensitive_fields {
        if get_item_by_dotted_path(secrets_doc, path).is_some()
            || get_item_by_dotted_path(doc, path).is_some()
        {
            continue;
        }
        if let Some(default_item) = get_item_by_dotted_path(defaults, path) {
            let _ = set_item_by_dotted_path(doc, path, default_item.clone());
        }
    }
}

fn apply_sensitive_display_values(
    doc: &mut DocumentMut,
    secrets_doc: &DocumentMut,
    sensitive_fields: &[String],
) {
    for path in sensitive_fields {
        if get_item_by_dotted_path(secrets_doc, path).is_some() {
            let _ = set_item_by_dotted_path(doc, path, value(SENSITIVE_DISPLAY_VALUE));
        }
    }
}

fn overlay_sensitive_values(
    doc: &mut DocumentMut,
    secrets_doc: &DocumentMut,
    sensitive_fields: &[String],
) {
    for path in sensitive_fields {
        if let Some(secret_item) = get_item_by_dotted_path(secrets_doc, path) {
            let _ = set_item_by_dotted_path(doc, path, secret_item.clone());
        }
    }
}

fn get_item_by_dotted_path<'a>(doc: &'a DocumentMut, path: &str) -> Option<&'a Item> {
    let parts = path.split('.').collect::<Vec<_>>();
    get_item_by_path(doc, &parts)
}

fn take_item_by_dotted_path(doc: &mut DocumentMut, path: &str) -> Result<Option<Item>> {
    let parts = path.split('.').collect::<Vec<_>>();
    if parts.is_empty() {
        return Ok(None);
    }
    if parts.len() == 1 {
        return Ok(doc.as_table_mut().remove(parts[0]));
    }

    let (parent_parts, last) = parts.split_at(parts.len() - 1);
    let Some(parent) = get_item_mut_by_path(doc, parent_parts) else {
        return Ok(None);
    };
    match parent {
        Item::Table(table) => Ok(table.remove(last[0])),
        _ => Ok(None),
    }
}

fn set_item_by_dotted_path(doc: &mut DocumentMut, path: &str, item: Item) -> Result<()> {
    let parts = path
        .split('.')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return Ok(());
    }

    let mut table = doc.as_table_mut();
    for part in &parts[..parts.len() - 1] {
        let item = table
            .entry(part)
            .or_insert_with(|| Item::Table(toml_edit::Table::new()));
        if !item.is_table() {
            *item = Item::Table(toml_edit::Table::new());
        }
        table = item.as_table_mut().ok_or_else(|| {
            anyhow::anyhow!("failed to create table for sensitive config path: {path}")
        })?;
    }

    table.insert(parts[parts.len() - 1], item);
    Ok(())
}

fn is_sensitive_display_item(item: &Item) -> bool {
    item.as_str()
        .is_some_and(|value| value == SENSITIVE_DISPLAY_VALUE)
}

fn merge_missing_default_items(target: &mut Item, defaults: &Item) {
    match (target, defaults) {
        (Item::Table(target_table), Item::Table(default_table)) => {
            merge_missing_table_items(target_table, default_table);
        }
        (Item::Table(target_table), Item::Value(Value::InlineTable(default_table))) => {
            merge_missing_table_from_inline_table(target_table, default_table);
        }
        (Item::Value(target_value), default_item) => {
            merge_missing_value_items(target_value, default_item);
        }
        (Item::ArrayOfTables(target_tables), Item::ArrayOfTables(default_tables)) => {
            if let Some(default_table) = default_tables.iter().next() {
                for target_table in target_tables.iter_mut() {
                    merge_missing_table_items(target_table, default_table);
                }
            }
        }
        _ => {}
    }
}

fn overlay_config_items(target: &mut Item, source: &Item) {
    match (target, source) {
        (Item::Table(target_table), Item::Table(source_table)) => {
            overlay_config_table_items(target_table, source_table);
        }
        (Item::Table(target_table), Item::Value(Value::InlineTable(source_table))) => {
            overlay_config_table_from_inline_table(target_table, source_table);
        }
        (Item::Value(Value::InlineTable(target_table)), Item::Table(source_table)) => {
            overlay_config_inline_table_from_table(target_table, source_table);
        }
        (
            Item::Value(Value::InlineTable(target_table)),
            Item::Value(Value::InlineTable(source_table)),
        ) => {
            overlay_config_inline_table_items(target_table, source_table);
        }
        (target_item, source_item) => {
            *target_item = source_item.clone();
        }
    }
}

fn overlay_config_table_items(
    target_table: &mut toml_edit::Table,
    source_table: &toml_edit::Table,
) {
    for (key, source_item) in source_table.iter() {
        if key.starts_with("auto_doc_") {
            continue;
        }

        match target_table.get_mut(key) {
            Some(target_item) => overlay_config_items(target_item, source_item),
            None => {
                target_table.insert(key, source_item.clone());
            }
        }
    }
}

fn overlay_config_table_from_inline_table(
    target_table: &mut toml_edit::Table,
    source_table: &toml_edit::InlineTable,
) {
    for (key, source_value) in source_table.iter() {
        match target_table.get_mut(key) {
            Some(target_item) => {
                let source_item = Item::Value(source_value.clone());
                overlay_config_items(target_item, &source_item);
            }
            None => {
                target_table.insert(key, Item::Value(source_value.clone()));
            }
        }
    }
}

fn overlay_config_inline_table_from_table(
    target_table: &mut toml_edit::InlineTable,
    source_table: &toml_edit::Table,
) {
    for (key, source_item) in source_table.iter() {
        if key.starts_with("auto_doc_") {
            continue;
        }

        match target_table.get_mut(key) {
            Some(target_value) => {
                let mut target_item = Item::Value(target_value.clone());
                overlay_config_items(&mut target_item, source_item);
                if let Some(value) = default_item_to_value(&target_item) {
                    *target_value = value;
                }
            }
            None => {
                if let Some(source_value) = default_item_to_value(source_item) {
                    target_table.insert(key, source_value);
                }
            }
        }
    }
}

fn overlay_config_inline_table_items(
    target_table: &mut toml_edit::InlineTable,
    source_table: &toml_edit::InlineTable,
) {
    for (key, source_value) in source_table.iter() {
        match target_table.get_mut(key) {
            Some(target_value) => {
                let mut target_item = Item::Value(target_value.clone());
                let source_item = Item::Value(source_value.clone());
                overlay_config_items(&mut target_item, &source_item);
                if let Some(value) = default_item_to_value(&target_item) {
                    *target_value = value;
                }
            }
            None => {
                target_table.insert(key, source_value.clone());
            }
        }
    }
}

fn merge_missing_table_items(
    target_table: &mut toml_edit::Table,
    default_table: &toml_edit::Table,
) {
    for (key, default_item) in default_table.iter() {
        if key.starts_with("auto_doc_") {
            continue;
        }

        match target_table.get_mut(key) {
            Some(target_item) => merge_missing_default_items(target_item, default_item),
            None => {
                target_table.insert(key, default_item.clone());
            }
        }
    }
}

fn merge_missing_table_from_inline_table(
    target_table: &mut toml_edit::Table,
    default_table: &toml_edit::InlineTable,
) {
    for (key, default_value) in default_table.iter() {
        if key.starts_with("auto_doc_") {
            continue;
        }

        match target_table.get_mut(key) {
            Some(target_item) => {
                let default_item = Item::Value(default_value.clone());
                merge_missing_default_items(target_item, &default_item);
            }
            None => {
                target_table.insert(key, Item::Value(default_value.clone()));
            }
        }
    }
}

fn merge_missing_value_items(target_value: &mut Value, defaults: &Item) {
    match (target_value, defaults) {
        (Value::InlineTable(target_table), Item::Table(default_table)) => {
            merge_missing_inline_table_from_table(target_table, default_table);
        }
        (Value::InlineTable(target_table), Item::Value(Value::InlineTable(default_table))) => {
            merge_missing_inline_table_items(target_table, default_table);
        }
        (Value::Array(target_array), Item::Value(Value::Array(default_array))) => {
            for (target_value, default_value) in target_array.iter_mut().zip(default_array.iter()) {
                let default_item = Item::Value(default_value.clone());
                merge_missing_value_items(target_value, &default_item);
            }
        }
        _ => {}
    }
}

fn merge_missing_inline_table_from_table(
    target_table: &mut toml_edit::InlineTable,
    default_table: &toml_edit::Table,
) {
    for (key, default_item) in default_table.iter() {
        if key.starts_with("auto_doc_") {
            continue;
        }

        match target_table.get_mut(key) {
            Some(target_value) => merge_missing_value_items(target_value, default_item),
            None => {
                if let Some(default_value) = default_item_to_value(default_item) {
                    target_table.insert(key, default_value);
                }
            }
        }
    }
}

fn merge_missing_inline_table_items(
    target_table: &mut toml_edit::InlineTable,
    default_table: &toml_edit::InlineTable,
) {
    for (key, default_value) in default_table.iter() {
        if key.starts_with("auto_doc_") {
            continue;
        }

        match target_table.get_mut(key) {
            Some(target_value) => {
                let default_item = Item::Value(default_value.clone());
                merge_missing_value_items(target_value, &default_item);
            }
            None => {
                target_table.insert(key, default_value.clone());
            }
        }
    }
}

fn default_item_to_value(default_item: &Item) -> Option<Value> {
    default_item.clone().into_value().ok()
}

fn get_item_by_path<'a>(doc: &'a DocumentMut, path: &[&str]) -> Option<&'a toml_edit::Item> {
    let mut current = doc.as_item();

    for part in path {
        match current {
            toml_edit::Item::Table(table) => {
                if let Some(item) = table.get(part) {
                    current = item;
                } else {
                    return None;
                }
            }
            toml_edit::Item::ArrayOfTables(array) => {
                // 如果是数组表，尝试获取第一个元素
                if let Some(first) = array.iter().next() {
                    if let Some(item) = first.get(part) {
                        current = item;
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    }

    Some(current)
}

// 更新 AutoDoc 注释
fn update_autodoc_for_key(doc: &mut DocumentMut, key: &str, new_comment: &str) {
    let start_marker = "# ======= AutoDoc =======";
    let end_marker = "# =======================";

    let processed_comment = ensure_comment_prefix(new_comment);
    let parts: Vec<&str> = key.split('.').collect();

    if parts.len() == 1 {
        // 顶层键或表
        if let Some(item) = doc.get_mut(parts[0]) {
            match item {
                // ✅ 表头：[Name]
                toml_edit::Item::Table(table) => {
                    update_table_decor(table, start_marker, end_marker, &processed_comment);
                }
                // ✅ 普通键值
                _ => {
                    if let Some(mut key_mut) = doc.key_mut(parts[0]) {
                        update_key_decor(
                            &mut key_mut,
                            start_marker,
                            end_marker,
                            &processed_comment,
                        );
                    }
                }
            }
        }
    } else {
        // 嵌套键：Name.hello
        let (parent_parts, last_part) = parts.split_at(parts.len() - 1);

        if let Some(parent_item) = get_item_mut_by_path(doc, parent_parts) {
            match parent_item {
                toml_edit::Item::Table(table) => {
                    // 判断最后一个部分是表还是键
                    if let Some(last_item) = table.get_mut(last_part[0]) {
                        match last_item {
                            // ✅ 子表
                            toml_edit::Item::Table(sub_table) => {
                                update_table_decor(
                                    sub_table,
                                    start_marker,
                                    end_marker,
                                    &processed_comment,
                                );
                            }
                            // ✅ 数组表
                            toml_edit::Item::ArrayOfTables(array) => {
                                update_array_tables_decor(
                                    array,
                                    start_marker,
                                    end_marker,
                                    &processed_comment,
                                );
                            }
                            // ✅ 普通键
                            _ => {
                                if let Some(mut key_mut) = table.key_mut(last_part[0]) {
                                    update_key_decor(
                                        &mut key_mut,
                                        start_marker,
                                        end_marker,
                                        &processed_comment,
                                    );
                                }
                            }
                        }
                    }
                }
                // 当父节点是数组表时，不给元素内字段批量写注释，避免在 [[...]] 前重复膨胀
                toml_edit::Item::ArrayOfTables(_) => {}
                _ => {}
            }
        }
    }
}
fn get_item_mut_by_path<'a>(
    doc: &'a mut DocumentMut,
    path: &[&str],
) -> Option<&'a mut toml_edit::Item> {
    let mut current = doc.as_table_mut();

    for (i, part) in path.iter().enumerate() {
        if i == path.len() - 1 {
            // 最后一个部分，返回可变引用
            return current.get_mut(part);
        } else {
            // 中间部分，继续深入
            match current.get_mut(part) {
                Some(toml_edit::Item::Table(table)) => {
                    current = table;
                }
                Some(toml_edit::Item::ArrayOfTables(array)) => {
                    // 如果是数组表，取第一个元素
                    if let Some(first) = array.iter_mut().next() {
                        current = first;
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        }
    }

    None
}
// 更新键的装饰
fn update_key_decor(
    key_mut: &mut toml_edit::KeyMut,
    start_marker: &str,
    end_marker: &str,
    new_content: &str,
) {
    let old_prefix = key_mut
        .leaf_decor_mut()
        .prefix()
        .map(|r| r.as_str().unwrap_or(""))
        .unwrap_or("")
        .to_string();

    let new_prefix = replace_autodoc_block(&old_prefix, start_marker, end_marker, new_content);

    let mut final_prefix = new_prefix;
    if !final_prefix.ends_with('\n') {
        final_prefix.push('\n');
    }

    key_mut
        .leaf_decor_mut()
        .set_prefix(RawString::from(final_prefix));
}
fn update_table_decor(
    table: &mut toml_edit::Table,
    start_marker: &str,
    end_marker: &str,
    new_content: &str,
) {
    let old_prefix = table
        .decor_mut()
        .prefix()
        .map(|r| r.as_str().unwrap_or(""))
        .unwrap_or("")
        .to_string();

    let new_prefix = replace_autodoc_block(&old_prefix, start_marker, end_marker, new_content);
    let mut final_prefix = new_prefix;
    if !final_prefix.ends_with('\n') {
        final_prefix.push('\n');
    }
    table.decor_mut().set_prefix(RawString::from(final_prefix));
}

fn update_array_tables_decor(
    array: &mut toml_edit::ArrayOfTables,
    start_marker: &str,
    end_marker: &str,
    new_content: &str,
) {
    if let Some(first) = array.iter_mut().next() {
        update_table_decor(first, start_marker, end_marker, new_content);
    }
}
fn build_safe_path(
    base: &std::path::Path,
    path: &'static str,
) -> std::io::Result<std::path::PathBuf> {
    let base_abs = std::path::absolute(base)?;
    let sub_trimmed = path.trim_start_matches('/');
    let sub_path = std::path::Path::new(sub_trimmed);

    for comp in sub_path.components() {
        match comp {
            std::path::Component::ParentDir | std::path::Component::CurDir => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("Sub path contains illegal component: {:?}", comp),
                ));
            }
            _ => {}
        }
    }

    let final_path = base_abs.join(sub_path);
    if !final_path.starts_with(&base_abs) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Final path escapes base directory",
        ));
    }

    Ok(final_path)
}

fn extract_auto_doc(doc: &DocumentMut) -> AutoDocConfig {
    let mut config = AutoDocConfig::default();

    if let Some(v) = doc
        .get("auto_doc_system_version_hash")
        .and_then(|v| v.as_str())
    {
        config.system_version_hash = v.to_string();
    }
    if let Some(v) = doc.get("auto_doc_system_lang").and_then(|v| v.as_str()) {
        config.system_lang = v.to_string();
    }
    if let Some(v) = doc.get("auto_doc_setting_lang").and_then(|v| v.as_str()) {
        config.setting_lang = v.to_string();
    }
    if let Some(v) = doc.get("auto_doc_setting_enable").and_then(|v| v.as_bool()) {
        config.setting_enable = v;
    }

    config
}

fn insert_auto_doc_fields(doc: &mut DocumentMut, config: &AutoDocConfig) {
    doc.insert(
        "auto_doc_system_version_hash",
        value(&config.system_version_hash),
    );
    doc.insert("auto_doc_system_lang", value(&config.system_lang));
    doc.insert("auto_doc_setting_lang", value(&config.setting_lang));
    doc.insert("auto_doc_setting_enable", value(config.setting_enable));
}

fn remove_auto_doc_fields(doc: &DocumentMut) -> DocumentMut {
    let mut clean = DocumentMut::new();
    for (k, v) in doc.iter() {
        if !k.starts_with("auto_doc_") {
            clean.insert(k, v.clone());
        }
    }
    clean
}

fn replace_autodoc_block(
    prefix: &str,
    start_marker: &str,
    end_marker: &str,
    new_content: &str,
) -> String {
    let mut result = Vec::new();
    let mut in_block = false;

    for line in prefix.lines() {
        let trimmed = line.trim();
        if trimmed == start_marker.trim() {
            in_block = true;
            continue;
        }
        if in_block && trimmed == end_marker.trim() {
            in_block = false;
            continue;
        }
        if in_block {
            continue;
        }
        result.push(line.to_string());
    }

    while result.last().is_some_and(|line| line.trim().is_empty()) {
        result.pop();
    }

    result.push(start_marker.to_string());
    result.push(new_content.to_string());
    result.push(end_marker.to_string());

    result.join("\n")
}

fn ensure_comment_prefix(text: &str) -> String {
    text.lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                "#".to_string()
            } else if trimmed.starts_with('#') {
                line.to_string()
            } else {
                format!("# {}", trimmed)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn ensure_or_update_doc_header(doc: &mut DocumentMut, lang: &str) {
    let header_text = rust_i18n::t!("autodoc.header", locale = lang);
    let processed_header = ensure_comment_prefix(&header_text);
    let start_marker = "# ==== AutoDocHeader ====";
    let end_marker = "# =======================";
    let new_header_block = format!("{}\n{}\n{}", start_marker, processed_header, end_marker);

    remove_doc_headers(doc, start_marker, end_marker);

    let current_prefix = doc
        .decor()
        .prefix()
        .map(|r| r.as_str().unwrap_or(""))
        .unwrap_or("")
        .to_string();
    let new_prefix = if current_prefix.is_empty() {
        format!("{}\n", new_header_block)
    } else {
        format!("{}\n\n{}", new_header_block, current_prefix)
    };

    doc.decor_mut().set_prefix(RawString::from(new_prefix));
}

fn remove_doc_headers(doc: &mut DocumentMut, start_marker: &str, end_marker: &str) {
    let current_prefix = doc
        .decor()
        .prefix()
        .map(|r| r.as_str().unwrap_or(""))
        .unwrap_or("")
        .to_string();
    let new_prefix = remove_doc_header_blocks(&current_prefix, start_marker, end_marker);
    doc.decor_mut().set_prefix(RawString::from(new_prefix));

    remove_doc_headers_from_table(doc.as_table_mut(), start_marker, end_marker);
}

fn remove_doc_headers_from_table(
    table: &mut toml_edit::Table,
    start_marker: &str,
    end_marker: &str,
) {
    let keys = table
        .iter()
        .map(|(key, _)| key.to_string())
        .collect::<Vec<_>>();

    for key in keys {
        if let Some(mut key_mut) = table.key_mut(&key) {
            let current_prefix = key_mut
                .leaf_decor_mut()
                .prefix()
                .map(|r| r.as_str().unwrap_or(""))
                .unwrap_or("")
                .to_string();
            let new_prefix = remove_doc_header_blocks(&current_prefix, start_marker, end_marker);
            key_mut
                .leaf_decor_mut()
                .set_prefix(RawString::from(new_prefix));
        }

        if let Some(item) = table.get_mut(&key) {
            match item {
                toml_edit::Item::Table(table) => {
                    let current_prefix = table
                        .decor_mut()
                        .prefix()
                        .map(|r| r.as_str().unwrap_or(""))
                        .unwrap_or("")
                        .to_string();
                    let new_prefix =
                        remove_doc_header_blocks(&current_prefix, start_marker, end_marker);
                    table.decor_mut().set_prefix(RawString::from(new_prefix));
                    remove_doc_headers_from_table(table, start_marker, end_marker);
                }
                toml_edit::Item::ArrayOfTables(array) => {
                    for table in array.iter_mut() {
                        let current_prefix = table
                            .decor_mut()
                            .prefix()
                            .map(|r| r.as_str().unwrap_or(""))
                            .unwrap_or("")
                            .to_string();
                        let new_prefix =
                            remove_doc_header_blocks(&current_prefix, start_marker, end_marker);
                        table.decor_mut().set_prefix(RawString::from(new_prefix));
                        remove_doc_headers_from_table(table, start_marker, end_marker);
                    }
                }
                _ => {}
            }
        }
    }
}

fn remove_doc_header_blocks(prefix: &str, start_marker: &str, end_marker: &str) -> String {
    let lines: Vec<&str> = prefix.lines().collect();
    let mut result = String::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        if trimmed == start_marker {
            let mut j = i + 1;
            while j < lines.len() {
                let next_trimmed = lines[j].trim();
                if next_trimmed == start_marker
                    || next_trimmed == "# ======= AutoDoc ======="
                    || next_trimmed.starts_with('[')
                    || (!next_trimmed.is_empty() && !next_trimmed.starts_with('#'))
                {
                    break;
                }
                if next_trimmed == end_marker {
                    j += 1;
                    break;
                }
                j += 1;
            }

            i = j;
            continue;
        }

        result.push_str(line);
        result.push('\n');
        i += 1;
    }

    while result.starts_with('\n') {
        result.remove(0);
    }

    result
}
