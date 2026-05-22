use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use toml_edit::{DocumentMut, RawString, value};

use crate::build;

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

        // ----- 文件不存在：创建全新配置 -----
        if !path.exists() {
            return Self::create_new_config(&path, lang, enable_auto_doc);
        }

        // ===== 文件存在：原地更新 =====
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("无法读取文件 {}", path.display()))?;
        let mut doc = content
            .parse::<DocumentMut>()
            .with_context(|| "TOML 格式错误")?;

        // 1. 提取当前文件中的 AutoDoc 元数据
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
        let default_config = Self::default();
        let default_toml =
            toml::to_string_pretty(&default_config).with_context(|| "序列化默认配置失败")?;
        let default_doc = default_toml
            .parse::<DocumentMut>()
            .expect("默认 TOML 必须合法");

        for (k, v) in default_doc.iter() {
            if !k.starts_with("auto_doc_") && !doc.contains_key(k) {
                doc.insert(k, v.clone());
            }
        }

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
        let new_content = doc.to_string();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, &new_content)?;

        // 8. 反序列化
        let clean_doc = remove_auto_doc_fields(&doc);
        let config: Self = toml::from_str(&clean_doc.to_string())
            .with_context(|| "配置类型不匹配，可能字段已更改")?;

        Ok(config)
    }

    // 创建全新配置文件
    fn create_new_config(
        path: &std::path::Path,
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

        for (k, v) in user_doc.iter() {
            if !k.starts_with("auto_doc_") {
                doc.insert(k, v.clone());
            }
        }

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
        std::fs::write(path, doc.to_string())?;
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
            if let toml_edit::Item::Table(table) = parent_item {
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
    let lines: Vec<&str> = prefix.lines().collect();
    let mut result = Vec::new();
    let mut in_block = false;
    let mut block_inserted = false;

    for line in &lines {
        if line.trim() == start_marker.trim() {
            result.push(start_marker.to_string());
            result.push(new_content.to_string());
            result.push(end_marker.to_string());
            in_block = true;
            block_inserted = true;
        } else if line.trim() == end_marker.trim() {
            in_block = false;
            continue;
        } else if in_block {
            continue;
        } else {
            result.push(line.to_string());
        }
    }

    if !block_inserted {
        result.push(start_marker.to_string());
        result.push(new_content.to_string());
        result.push(end_marker.to_string());
    }

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
