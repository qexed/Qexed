use toml_edit::{DocumentMut, Item, Value};
use crate::TomlError;

/// 合并两个 toml 数据
/// new的数据写入在old上(若old没有则补字段，若有则修改)
pub fn merge(old: &DocumentMut, new: &DocumentMut) -> Result<DocumentMut, TomlError> {
    let mut result = old.clone();
    merge_item(result.as_item_mut(), new.as_item());
    Ok(result)
}

/// 用 source 覆盖 target（递归）
fn merge_item(target: &mut Item, source: &Item) {
    match (target, source) {
        // Table vs Table
        (Item::Table(target_table), Item::Table(source_table)) => {
            merge_table(target_table, source_table);
        }
        // Table vs InlineTable
        (Item::Table(target_table), Item::Value(Value::InlineTable(source_table))) => {
            merge_table_from_inline_table(target_table, source_table);
        }
        // InlineTable vs Table
        (Item::Value(Value::InlineTable(target_table)), Item::Table(source_table)) => {
            merge_inline_table_from_table(target_table, source_table);
        }
        // InlineTable vs InlineTable
        (
            Item::Value(Value::InlineTable(target_table)),
            Item::Value(Value::InlineTable(source_table)),
        ) => {
            merge_inline_table(target_table, source_table);
        }
        // ArrayOfTables vs ArrayOfTables：按索引逐表合并，多出来的直接追加
        (Item::ArrayOfTables(target_tables), Item::ArrayOfTables(source_tables)) => {
            merge_array_of_tables(target_tables, source_tables);
        }
        // Array vs Array：逐项递归（元素若是 inline table 就能保留 target 侧的其他字段）
        (Item::Value(Value::Array(target_arr)), Item::Value(Value::Array(source_arr))) => {
            merge_array(target_arr, source_arr);
        }
        // 其余情况（标量、类型不兼容等）：直接覆盖
        (target_item, source_item) => {
            *target_item = source_item.clone();
        }
    }
}

fn merge_table(target: &mut toml_edit::Table, source: &toml_edit::Table) {
    for (key, source_item) in source.iter() {
        match target.get_mut(key) {
            Some(target_item) => merge_item(target_item, source_item),
            None => {
                target.insert(key, source_item.clone());
            }
        }
    }
}

fn merge_table_from_inline_table(target: &mut toml_edit::Table, source: &toml_edit::InlineTable) {
    for (key, source_value) in source.iter() {
        let source_item = Item::Value(source_value.clone());
        match target.get_mut(key) {
            Some(target_item) => merge_item(target_item, &source_item),
            None => {
                target.insert(key, Item::Value(source_value.clone()));
            }
        }
    }
}

fn merge_inline_table_from_table(target: &mut toml_edit::InlineTable, source: &toml_edit::Table) {
    for (key, source_item) in source.iter() {
        match target.get_mut(key) {
            Some(target_value) => {
                let mut target_item = Item::Value(target_value.clone());
                merge_item(&mut target_item, source_item);
                if let Ok(value) = target_item.into_value() {
                    *target_value = value;
                }
            }
            None => {
                if let Ok(value) = source_item.clone().into_value() {
                    target.insert(key, value);
                }
            }
        }
    }
}

fn merge_inline_table(target: &mut toml_edit::InlineTable, source: &toml_edit::InlineTable) {
    for (key, source_value) in source.iter() {
        match target.get_mut(key) {
            Some(target_value) => {
                let mut target_item = Item::Value(target_value.clone());
                let source_item = Item::Value(source_value.clone());
                merge_item(&mut target_item, &source_item);
                if let Ok(value) = target_item.into_value() {
                    *target_value = value;
                }
            }
            None => {
                target.insert(key, source_value.clone());
            }
        }
    }
}

fn merge_array_of_tables(target: &mut toml_edit::ArrayOfTables, source: &toml_edit::ArrayOfTables) {
    for (i, source_table) in source.iter().enumerate() {
        match target.get_mut(i) {
            Some(target_table) => merge_table(target_table, source_table),
            None => target.push(source_table.clone()),
        }
    }
}

/// Array vs Array：按索引逐项 merge_item。
///
/// - 元素是 InlineTable/Table 时，会把 source 的字段合并进 target 的对应元素，
///   保留 target 侧其他字段（这正是 `servers = [{host, api_key}]` 场景需要的）。
/// - 元素是标量时，merge_item 走兜底分支即覆盖，语义和"整体替换"一致。
/// - source 更长则补 push；target 更长则保留多余的尾部元素（与 `merge_array_of_tables` 一致）。
fn merge_array(target: &mut toml_edit::Array, source: &toml_edit::Array) {
    for (i, source_value) in source.iter().enumerate() {
        match target.get_mut(i) {
            Some(target_value) => {
                let mut t = Item::Value(target_value.clone());
                let s = Item::Value(source_value.clone());
                merge_item(&mut t, &s);
                if let Item::Value(v) = t {
                    *target_value = v;
                }
            }
            None => {
                target.push(source_value.clone());
            }
        }
    }
}