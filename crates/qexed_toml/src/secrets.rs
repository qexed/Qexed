use toml_edit::{value, Array, ArrayOfTables, DocumentMut, InlineTable, Item, Table, Value};

use crate::{TomlError, merge};

const SENSITIVE_DISPLAY_VALUE: &str = "<stored in .secrets>";

/// 合并机密字段的内容，合并原内容
pub fn merge_secrets(raw: &DocumentMut, secret: &DocumentMut) -> Result<DocumentMut, TomlError> {
    Ok(merge(raw, secret)?)
}

#[derive(Debug, Clone)]
enum Seg {
    Key(String),
    Wildcard,
}

fn parse_path(s: &str) -> Vec<Seg> {
    s.split('.')
        .map(|p| {
            if p == "*" {
                Seg::Wildcard
            } else {
                Seg::Key(p.to_string())
            }
        })
        .collect()
}

fn is_placeholder(item: &Item) -> bool {
    matches!(
        item,
        Item::Value(Value::String(s)) if s.value().as_str() == SENSITIVE_DISPLAY_VALUE
    )
}

fn collect_paths(item: &Item, prefix: Vec<Seg>, out: &mut Vec<Vec<Seg>>) {
    match item {
        Item::Table(t) => {
            for (k, v) in t.iter() {
                let mut p = prefix.clone();
                p.push(Seg::Key(k.to_string()));
                collect_paths(v, p, out);
            }
        }
        Item::ArrayOfTables(arr) => {
            for t in arr.iter() {
                for (k, v) in t.iter() {
                    let mut p = prefix.clone();
                    p.push(Seg::Wildcard);
                    p.push(Seg::Key(k.to_string()));
                    collect_paths(v, p, out);
                }
            }
        }
        Item::Value(Value::InlineTable(it)) => {
            for (k, v) in it.iter() {
                let mut p = prefix.clone();
                p.push(Seg::Key(k.to_string()));
                collect_paths(&Item::Value(v.clone()), p, out);
            }
        }
        Item::Value(Value::Array(arr)) => {
            for v in arr.iter() {
                let mut p = prefix.clone();
                p.push(Seg::Wildcard);
                collect_paths(&Item::Value(v.clone()), p, out);
            }
        }
        Item::None => {}
        _ => {
            if !prefix.is_empty() {
                out.push(prefix);
            }
        }
    }
}

/// 拆解原始内容，拆解为不含机密内容的内容和机密内容
pub fn split_secrets(
    merged: &DocumentMut,
    secrets_rule: &[&str],
    secret: Option<&DocumentMut>,
) -> Result<(DocumentMut, DocumentMut), TomlError> {
    let mut main = merged.clone();
    let mut secrets = DocumentMut::new();

    let mut paths: Vec<Vec<Seg>> = Vec::with_capacity(secrets_rule.len());
    for rule in secrets_rule {
        paths.push(parse_path(rule));
    }
    if let Some(sec) = secret {
        let mut old_paths = Vec::new();
        collect_paths(sec.as_item(), Vec::new(), &mut old_paths);
        paths.extend(old_paths);
    }

    for path in &paths {
        extract(main.as_item_mut(), secrets.as_item_mut(), path);
    }

    prune_empty(secrets.as_item_mut());

    Ok((main, secrets))
}

fn extract(main: &mut Item, secret: &mut Item, path: &[Seg]) {
    if path.is_empty() {
        if is_placeholder(main) {
            return;
        }
        *secret = main.clone();
        *main = value(SENSITIVE_DISPLAY_VALUE);
        return;
    }

    match &path[0] {
        Seg::Key(k) => extract_key(main, secret, k, &path[1..]),
        Seg::Wildcard => extract_wildcard(main, secret, &path[1..]),
    }
}

fn extract_key(main: &mut Item, secret: &mut Item, key: &str, rest: &[Seg]) {
    match main {
        Item::Table(t) => {
            if !t.contains_key(key) {
                return;
            }
            let child = t.get_mut(key).unwrap();
            let secret_child = ensure_child(secret, key);
            extract(child, secret_child, rest);
        }
        Item::Value(Value::InlineTable(it)) => {
            if !it.contains_key(key) {
                return;
            }
            // secret 侧统一用 Table 存储；如果当前是 InlineTable，会被转换为 Table。
            let secret_table = as_table_mut(secret);
            if !secret_table.contains_key(key) {
                secret_table.insert(key, Item::Table(Table::new()));
            }
            let main_child = std::mem::replace(
                it.get_mut(key).unwrap(),
                Value::InlineTable(InlineTable::new()),
            );
            let mut main_item = Item::Value(main_child);
            let secret_child = secret_table.get_mut(key).unwrap();
            extract(&mut main_item, secret_child, rest);
            if let Item::Value(v) = main_item {
                *it.get_mut(key).unwrap() = v;
            }
        }
        _ => {}
    }
}

fn extract_wildcard(main: &mut Item, secret: &mut Item, rest: &[Seg]) {
    match main {
        Item::ArrayOfTables(arr) => {
            // `a.*` 结尾无法把整张表替换成占位符，跳过
            if rest.is_empty() {
                return;
            }
            let secret_arr = ensure_aot(secret);
            while secret_arr.len() < arr.len() {
                secret_arr.push(Table::new());
            }
            for i in 0..arr.len() {
                let main_table = std::mem::replace(arr.get_mut(i).unwrap(), Table::new());
                let secret_table = std::mem::replace(secret_arr.get_mut(i).unwrap(), Table::new());
                let mut main_item = Item::Table(main_table);
                let mut secret_item = Item::Table(secret_table);

                extract(&mut main_item, &mut secret_item, rest);

                if let Item::Table(mt) = main_item {
                    *arr.get_mut(i).unwrap() = mt;
                }
                if let Item::Table(st) = secret_item {
                    *secret_arr.get_mut(i).unwrap() = st;
                }
            }
        }
        Item::Value(Value::Array(arr)) => {
            if rest.is_empty() {
                return;
            }
            let secret_arr = ensure_array(secret);
            while secret_arr.len() < arr.len() {
                secret_arr.push(Value::InlineTable(InlineTable::new()));
            }
            let n = arr.len();
            for i in 0..n {
                let main_val = std::mem::replace(
                    arr.get_mut(i).unwrap(),
                    Value::InlineTable(InlineTable::new()),
                );
                let secret_val = std::mem::replace(
                    secret_arr.get_mut(i).unwrap(),
                    Value::InlineTable(InlineTable::new()),
                );
                let mut main_item = Item::Value(main_val);
                let mut secret_item = Item::Value(secret_val);

                extract(&mut main_item, &mut secret_item, rest);

                if let Item::Value(mv) = main_item {
                    *arr.get_mut(i).unwrap() = mv;
                }
                // 递归中 secret 可能被转成了 Table，统一塞回 Value。
                *secret_arr.get_mut(i).unwrap() = item_into_value(secret_item);
            }
        }
        _ => {}
    }
}

/// 把 Item 压成 Value（用于写入 Array）。
/// - `Item::Value(v)` 直接透传
/// - `Item::Table(t)` 转成 InlineTable（仅保留 Value 子项）
fn item_into_value(item: Item) -> Value {
    match item {
        Item::Value(v) => v,
        Item::Table(t) => {
            let mut it = InlineTable::new();
            for (k, v) in t.iter() {
                match v {
                    Item::Value(val) => {
                        it.insert(k, val.clone());
                    }
                    Item::Table(tt) => {
                        let nested = item_into_value(Item::Table(tt.clone()));
                        it.insert(k, nested);
                    }
                    _ => {}
                }
            }
            Value::InlineTable(it)
        }
        _ => Value::InlineTable(InlineTable::new()),
    }
}

/// 确保 `secret` 是 Table（如果不是会被转换），并返回 `key` 对应的子项
/// （不存在则插入空表占位）。
///
/// 注意：这里**不能**用 `Item::None` 占位——在 toml_edit 里它是 tombstone 语义，
/// 插入后 `get_mut` 拿不到，会直接 panic。
fn ensure_child<'a>(secret: &'a mut Item, key: &str) -> &'a mut Item {
    let t = as_table_mut(secret);
    if !t.contains_key(key) {
        t.insert(key, Item::Table(Table::new()));
    }
    t.get_mut(key).expect("just inserted")
}

fn as_table_mut(item: &mut Item) -> &mut Table {
    if !matches!(item, Item::Table(_)) {
        *item = Item::Table(Table::new());
    }
    match item {
        Item::Table(t) => t,
        _ => unreachable!(),
    }
}

fn ensure_aot(item: &mut Item) -> &mut ArrayOfTables {
    if !matches!(item, Item::ArrayOfTables(_)) {
        *item = Item::ArrayOfTables(ArrayOfTables::new());
    }
    match item {
        Item::ArrayOfTables(a) => a,
        _ => unreachable!(),
    }
}

fn ensure_array(item: &mut Item) -> &mut Array {
    if !matches!(item, Item::Value(Value::Array(_))) {
        *item = Item::Value(Value::Array(Array::new()));
    }
    match item {
        Item::Value(Value::Array(a)) => a,
        _ => unreachable!(),
    }
}

/// 递归删除空表 / 空表数组。返回 true 表示当前 item 现在为空、应由调用者移除。
fn prune_empty(item: &mut Item) -> bool {
    match item {
        Item::Table(t) => {
            let keys: Vec<String> = t.iter().map(|(k, _)| k.to_string()).collect();
            for k in keys {
                let remove = match t.get_mut(&k) {
                    Some(child) => prune_empty(child),
                    None => true,
                };
                if remove {
                    t.remove(&k);
                }
            }
            t.is_empty()
        }
        Item::ArrayOfTables(arr) => {
            let mut i = 0;
            while i < arr.len() {
                let should_remove = {
                    let mut tmp = Item::Table(std::mem::replace(
                        arr.get_mut(i).unwrap(),
                        Table::new(),
                    ));
                    let r = prune_empty(&mut tmp);
                    if let Item::Table(t) = tmp {
                        *arr.get_mut(i).unwrap() = t;
                    }
                    r
                };
                if should_remove {
                    arr.remove(i);
                } else {
                    i += 1;
                }
            }
            arr.is_empty()
        }
        Item::Value(Value::InlineTable(it)) => {
            let keys: Vec<String> = it.iter().map(|(k, _)| k.to_string()).collect();
            for k in keys {
                let remove = match it.get_mut(&k) {
                    Some(child) => {
                        let mut tmp = Item::Value(child.clone());
                        let r = prune_empty(&mut tmp);
                        if let Item::Value(v) = tmp {
                            *child = v;
                        }
                        r
                    }
                    None => true,
                };
                if remove {
                    it.remove(&k);
                }
            }
            it.is_empty()
        }
        Item::Value(Value::Array(arr)) => {
            for v in arr.iter_mut() {
                let mut tmp = Item::Value(v.clone());
                prune_empty(&mut tmp);
                if let Item::Value(nv) = tmp {
                    *v = nv;
                }
            }
            false
        }
        _ => false,
    }
}