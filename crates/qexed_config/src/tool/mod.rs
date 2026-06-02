use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use toml_edit::{DocumentMut, Item, Value, value};

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
// 涓婚厤缃?Trait
// ========================
pub trait AppConfigTrait: Serialize + for<'de> Deserialize<'de> + Default + Sized {
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

    fn sensitive_fields() -> Vec<String> {
        Vec::new()
    }

    fn load_or_create_default(
        lang: Option<String>,
        enable_auto_doc: Option<bool>,
        config_path: Option<std::path::PathBuf>,
    ) -> Result<Self> {
        let _ = (lang, enable_auto_doc);
        validate_config_name(Self::NAME)?;

        let base_dir = config_path.unwrap_or_else(|| std::path::PathBuf::from("./config"));
        let final_path = build_safe_path(&base_dir, Self::PATH)?;
        let path = final_path.join(Self::NAME).with_extension("toml");
        let secrets_path = secrets_path_for(&path)?;
        let split_dir = split_dir_for(&path)?;

        if !path.exists() {
            return Self::create_new_config(&path, &secrets_path, &split_dir, None, None);
        }

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read config file {}", path.display()))?;
        let mut doc = content
            .parse::<DocumentMut>()
            .with_context(|| format!("invalid TOML in {}", path.display()))?;
        overlay_split_config_files::<Self>(&mut doc, &split_dir)?;

        let default_doc = default_config_document::<Self>()?;
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
        remove_root_items_by_prefix(&mut doc, "auto_doc_");
        write_config_documents::<Self>(&doc, &path, &split_dir)?;

        let mut config_doc = doc.clone();
        overlay_sensitive_values(&mut config_doc, &effective_secrets, &sensitive_fields);
        toml::from_str(&config_doc.to_string())
            .with_context(|| format!("config type mismatch for {}", path.display()))
    }

    fn save_to_config(
        &self,
        lang: Option<String>,
        enable_auto_doc: Option<bool>,
        config_path: Option<std::path::PathBuf>,
    ) -> Result<()> {
        let _ = (lang, enable_auto_doc);
        validate_config_name(Self::NAME)?;

        let base_dir = config_path.unwrap_or_else(|| std::path::PathBuf::from("./config"));
        let final_path = build_safe_path(&base_dir, Self::PATH)?;
        let path = final_path.join(Self::NAME).with_extension("toml");
        let secrets_path = secrets_path_for(&path)?;
        let split_dir = split_dir_for(&path)?;

        let mut doc = if path.exists() {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read config file {}", path.display()))?;
            let mut existing = content
                .parse::<DocumentMut>()
                .with_context(|| format!("invalid TOML in {}", path.display()))?;
            overlay_split_config_files::<Self>(&mut existing, &split_dir)?;
            existing
        } else {
            DocumentMut::new()
        };

        let user_toml = toml::to_string_pretty(self).with_context(|| "serialize config failed")?;
        let user_doc = user_toml
            .parse::<DocumentMut>()
            .expect("serialized TOML must be valid");
        overlay_config_items(doc.as_item_mut(), user_doc.as_item());

        let sensitive_fields = Self::sensitive_fields();
        let mut secrets_doc = read_secrets_doc(&secrets_path)?;
        move_sensitive_fields_to_secrets(&mut doc, &mut secrets_doc, &sensitive_fields);
        apply_sensitive_display_values(&mut doc, &secrets_doc, &sensitive_fields);
        remove_root_items_by_prefix(&mut doc, "auto_doc_");
        write_config_documents::<Self>(&doc, &path, &split_dir)?;
        write_secrets_doc(&secrets_path, &secrets_doc)?;
        Ok(())
    }

    fn create_new_config(
        path: &std::path::Path,
        secrets_path: &std::path::Path,
        split_dir: &std::path::Path,
        lang: Option<String>,
        enable_auto_doc: Option<bool>,
    ) -> Result<Self> {
        let _ = (lang, enable_auto_doc);
        let config = Self::default();
        let mut doc = default_config_document::<Self>()?;
        let sensitive_fields = Self::sensitive_fields();
        let mut secrets_doc = DocumentMut::new();

        move_sensitive_fields_to_secrets(&mut doc, &mut secrets_doc, &sensitive_fields);
        apply_sensitive_display_values(&mut doc, &secrets_doc, &sensitive_fields);
        write_config_documents::<Self>(&doc, path, split_dir)?;
        write_secrets_doc(secrets_path, &secrets_doc)?;
        Ok(config)
    }
}

// ========================
// 杈呭姪鍑芥暟
// ========================

fn validate_config_name(name: &str) -> Result<()> {
    if name
        .chars()
        .all(|c| c.is_ascii_alphabetic() || c.is_ascii_digit() || c == '_')
    {
        return Ok(());
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!(
            "invalid config name `{name}`: only alphanumeric characters and underscores are allowed"
        ),
    )
    .into())
}

fn default_config_document<T: AppConfigTrait>() -> Result<DocumentMut> {
    let default_toml =
        toml::to_string_pretty(&T::default()).with_context(|| "serialize default config failed")?;
    default_toml
        .parse::<DocumentMut>()
        .with_context(|| "serialized default config is not valid TOML")
}

fn remove_root_items_by_prefix(doc: &mut DocumentMut, prefix: &str) {
    let keys = doc
        .iter()
        .map(|(key, _)| key.to_string())
        .filter(|key| key.starts_with(prefix))
        .collect::<Vec<_>>();
    for key in keys {
        doc.remove(&key);
    }
}

// 鏍规嵁璺緞鑾峰彇 Item 寮曠敤
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
        set_item_by_dotted_path(
            &mut split_doc,
            split_file_local_root_path(&split_file.root_path),
            item.clone(),
        )?;
        prune_nested_split_config_items(
            &mut split_doc,
            split_file_local_root_path(&split_file.root_path),
            &split_files,
        );
        std::fs::write(&file_path, split_doc.to_string())
            .with_context(|| format!("鏃犳硶鍐欏叆鎷嗗垎閰嶇疆鏂囦欢 {}", file_path.display()))?;
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
            .with_context(|| format!("鏃犳硶璇诲彇鎷嗗垎閰嶇疆鏂囦欢 {}", file_path.display()))?;
        let split_doc = content.parse::<DocumentMut>().with_context(|| {
            format!(
                "鎷嗗垎閰嶇疆鏂囦欢 TOML 鏍煎紡閿欒: {}",
                file_path.display()
            )
        })?;
        let mut mapped_doc = DocumentMut::new();
        if let Some(item) = get_item_by_dotted_path(&split_doc, &split_file.root_path) {
            set_item_by_dotted_path(&mut mapped_doc, &split_file.root_path, item.clone())?;
            overlay_config_items(doc.as_item_mut(), mapped_doc.as_item());
            continue;
        }
        let local_root_path = split_file_local_root_path(&split_file.root_path);
        if let Some(item) = get_item_by_dotted_path(&split_doc, local_root_path) {
            set_item_by_dotted_path(&mut mapped_doc, &split_file.root_path, item.clone())?;
            overlay_config_items(doc.as_item_mut(), mapped_doc.as_item());
            continue;
        }
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

fn split_file_local_root_path(root_path: &str) -> &str {
    root_path.strip_prefix("server.").unwrap_or(root_path)
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
        .with_context(|| format!("鏃犳硶璇诲彇鏁忔劅閰嶇疆鏂囦欢 {}", path.display()))?;
    content
        .parse::<DocumentMut>()
        .with_context(|| format!("鏁忔劅閰嶇疆鏂囦欢 TOML 鏍煎紡閿欒: {}", path.display()))
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
                current = table.get(part)?;
            }
            toml_edit::Item::ArrayOfTables(array) => {
                current = array.iter().next()?.get(part)?;
            }
            _ => return None,
        }
    }

    Some(current)
}

fn get_item_mut_by_path<'a>(
    doc: &'a mut DocumentMut,
    path: &[&str],
) -> Option<&'a mut toml_edit::Item> {
    let mut current = doc.as_item_mut();

    for part in path {
        match current {
            toml_edit::Item::Table(table) => {
                current = table.get_mut(part)?;
            }
            toml_edit::Item::ArrayOfTables(array) => {
                current = array.iter_mut().next()?.get_mut(part)?;
            }
            _ => return None,
        }
    }

    Some(current)
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
