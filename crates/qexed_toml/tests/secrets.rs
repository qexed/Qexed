//! 集成测试：验证 split_secrets / merge_secrets 的拆分、合并与往返。

use qexed_toml::{merge_secrets, split_secrets};
use toml_edit::{DocumentMut, Item};

const PLACEHOLDER: &str = "<stored in .secrets>";

fn doc(s: &str) -> DocumentMut {
    s.parse().expect("valid TOML")
}

/// 统计 Table 里"有效"的条目数（忽略内部留的空壳 Item::None）。
fn meaningful_entries(t: &toml_edit::Table) -> usize {
    t.iter().filter(|(_, v)| !matches!(v, Item::None)).count()
}

// ---------------------------------------------------------------------------
// 基本拆分
// ---------------------------------------------------------------------------

#[test]
fn split_top_level_secret() {
    let merged = doc(r#"
token = "sk-abc"
name = "alice"
"#);
    let (main, secrets) = split_secrets(&merged, &["token"], None).unwrap();

    assert_eq!(main["token"].as_str(), Some(PLACEHOLDER));
    assert_eq!(main["name"].as_str(), Some("alice"));

    assert_eq!(secrets["token"].as_str(), Some("sk-abc"));
}

#[test]
fn split_nested_table_secret() {
    let merged = doc(r#"
[auth]
username = "alice"
password = "hunter2"
"#);
    let (main, secrets) = split_secrets(&merged, &["auth.password"], None).unwrap();

    assert_eq!(main["auth"]["password"].as_str(), Some(PLACEHOLDER));
    assert_eq!(main["auth"]["username"].as_str(), Some("alice"));

    assert_eq!(secrets["auth"]["password"].as_str(), Some("hunter2"));
    // secret 里不应该把非机密字段也带进来
    assert!(
        secrets
            .as_table()
            .get("auth")
            .and_then(|i| i.as_table())
            .and_then(|t| t.get("username"))
            .is_none()
    );
}

#[test]
fn split_inline_table_secret() {
    let merged = doc(r#"
auth = { username = "alice", password = "hunter2" }
"#);
    let (main, secrets) = split_secrets(&merged, &["auth.password"], None).unwrap();

    assert_eq!(main["auth"]["password"].as_str(), Some(PLACEHOLDER));
    assert_eq!(main["auth"]["username"].as_str(), Some("alice"));

    assert_eq!(secrets["auth"]["password"].as_str(), Some("hunter2"));
}

#[test]
fn split_array_of_tables_wildcard() {
    let merged = doc(r#"
[[servers]]
host = "a.example.com"
api_key = "key-a"

[[servers]]
host = "b.example.com"
api_key = "key-b"
"#);
    let (main, secrets) = split_secrets(&merged, &["servers.*.api_key"], None).unwrap();

    let servers = main["servers"].as_array_of_tables().unwrap();
    assert_eq!(servers.len(), 2);
    assert_eq!(
        servers.get(0).unwrap()["host"].as_str(),
        Some("a.example.com")
    );
    assert_eq!(
        servers.get(0).unwrap()["api_key"].as_str(),
        Some(PLACEHOLDER)
    );
    assert_eq!(
        servers.get(1).unwrap()["host"].as_str(),
        Some("b.example.com")
    );
    assert_eq!(
        servers.get(1).unwrap()["api_key"].as_str(),
        Some(PLACEHOLDER)
    );

    let sec_servers = secrets["servers"].as_array_of_tables().unwrap();
    assert_eq!(sec_servers.len(), 2);
    assert_eq!(
        sec_servers.get(0).unwrap()["api_key"].as_str(),
        Some("key-a")
    );
    assert_eq!(
        sec_servers.get(1).unwrap()["api_key"].as_str(),
        Some("key-b")
    );
    // secret 里不该带 host
    assert!(sec_servers.get(0).unwrap().get("host").is_none());
}

#[test]
fn multiple_rules_are_applied() {
    let merged = doc(r#"
token = "sk-abc"
other = "public"

[auth]
password = "p"

[billing]
card = "4111"
"#);
    let (main, secrets) =
        split_secrets(&merged, &["token", "auth.password", "billing.card"], None).unwrap();

    assert_eq!(main["token"].as_str(), Some(PLACEHOLDER));
    assert_eq!(main["auth"]["password"].as_str(), Some(PLACEHOLDER));
    assert_eq!(main["billing"]["card"].as_str(), Some(PLACEHOLDER));
    assert_eq!(main["other"].as_str(), Some("public"));

    assert_eq!(secrets["token"].as_str(), Some("sk-abc"));
    assert_eq!(secrets["auth"]["password"].as_str(), Some("p"));
    assert_eq!(secrets["billing"]["card"].as_str(), Some("4111"));
}

// ---------------------------------------------------------------------------
// 往返：split → merge
// ---------------------------------------------------------------------------

#[test]
fn roundtrip_split_then_merge() {
    let original = doc(r#"
token = "sk-abc"
name = "alice"

[auth]
username = "alice"
password = "hunter2"

[[servers]]
host = "a.example.com"
api_key = "key-a"
"#);

    let (main, secrets) = split_secrets(
        &original,
        &["token", "auth.password", "servers.*.api_key"],
        None,
    )
    .unwrap();

    // 主文件全是占位符
    assert_eq!(main["token"].as_str(), Some(PLACEHOLDER));
    assert_eq!(main["auth"]["password"].as_str(), Some(PLACEHOLDER));
    assert_eq!(
        main["servers"]
            .as_array_of_tables()
            .unwrap()
            .get(0)
            .unwrap()["api_key"]
            .as_str(),
        Some(PLACEHOLDER)
    );

    // 再合并回去
    let restored = merge_secrets(&main, &secrets).unwrap();
    assert_eq!(restored["token"].as_str(), Some("sk-abc"));
    assert_eq!(restored["name"].as_str(), Some("alice"));
    assert_eq!(restored["auth"]["username"].as_str(), Some("alice"));
    assert_eq!(restored["auth"]["password"].as_str(), Some("hunter2"));
    assert_eq!(
        restored["servers"]
            .as_array_of_tables()
            .unwrap()
            .get(0)
            .unwrap()["api_key"]
            .as_str(),
        Some("key-a")
    );
}

#[test]
fn roundtrip_inline_table() {
    let original = doc(r#"
auth = { username = "alice", password = "hunter2" }
"#);
    let (main, secrets) = split_secrets(&original, &["auth.password"], None).unwrap();
    let restored = merge_secrets(&main, &secrets).unwrap();

    assert_eq!(restored["auth"]["password"].as_str(), Some("hunter2"));
    assert_eq!(restored["auth"]["username"].as_str(), Some("alice"));
}

// ---------------------------------------------------------------------------
// secret 参数：扩展规则 / 值的来源
// ---------------------------------------------------------------------------

#[test]
fn previously_secret_fields_remain_secret() {
    // 上一次保存时的 secrets 里有 legacy_key，
    // 尽管现在 secrets_rule 不再包含它，它依然要被抽出去。
    let old_secrets = doc(r#"
legacy_key = "old-value"
"#);
    let merged = doc(r#"
legacy_key = "new-value"
token = "sk-abc"
"#);

    let (main, secrets) = split_secrets(&merged, &["token"], Some(&old_secrets)).unwrap();

    assert_eq!(main["legacy_key"].as_str(), Some(PLACEHOLDER));
    assert_eq!(main["token"].as_str(), Some(PLACEHOLDER));
    assert_eq!(secrets["legacy_key"].as_str(), Some("new-value"));
    assert_eq!(secrets["token"].as_str(), Some("sk-abc"));
}

#[test]
fn secret_argument_only_extends_rules_not_values() {
    // secret 参数仅用于"哪些字段算机密"的扩展，值取自 merged（当前真实内容），
    // 不应该保留 secret 文件里的陈旧值。
    let old_secrets = doc(r#"
token = "stale-value"
"#);
    let merged = doc(r#"
token = "fresh-value"
"#);
    let (_main, secrets) = split_secrets(&merged, &[], Some(&old_secrets)).unwrap();

    assert_eq!(secrets["token"].as_str(), Some("fresh-value"));
}

// ---------------------------------------------------------------------------
// 边界情况
// ---------------------------------------------------------------------------

#[test]
fn wildcard_terminating_path_is_ignored() {
    // `servers.*` 无法把整个表替换成标量占位符，应当安全跳过而不 panic。
    let merged = doc(r#"
[[servers]]
host = "a"
api_key = "k"
"#);
    let (main, secrets) = split_secrets(&merged, &["servers.*"], None).unwrap();

    assert_eq!(
        main["servers"]
            .as_array_of_tables()
            .unwrap()
            .get(0)
            .unwrap()["api_key"]
            .as_str(),
        Some("k")
    );
    // secret 里不应该出现任何有效字段（可能残留空壳 Item::None）。
    assert_eq!(meaningful_entries(secrets.as_table()), 0);
}

#[test]
fn missing_path_is_ignored() {
    let merged = doc(r#"
name = "alice"
"#);
    let (main, secrets) = split_secrets(&merged, &["nope.not.there"], None).unwrap();

    assert_eq!(main["name"].as_str(), Some("alice"));
    assert_eq!(meaningful_entries(secrets.as_table()), 0);
}

#[test]
fn no_rules_no_old_secret_returns_original() {
    let merged = doc(r#"
a = 1
b = 2
"#);
    let (main, secrets) = split_secrets(&merged, &[], None).unwrap();

    assert_eq!(main["a"].as_integer(), Some(1));
    assert_eq!(main["b"].as_integer(), Some(2));
    assert_eq!(meaningful_entries(secrets.as_table()), 0);
}

#[test]
fn original_document_is_not_mutated() {
    let merged = doc(r#"
token = "sk-abc"
"#);
    let before = merged.to_string();

    let _ = split_secrets(&merged, &["token"], None).unwrap();

    // 传入的 &DocumentMut 不被修改
    assert_eq!(merged.to_string(), before);
    assert_eq!(merged["token"].as_str(), Some("sk-abc"));
}

#[test]
fn array_of_inline_tables_wildcard() {
    let merged = doc(r#"
servers = [
    { host = "a", api_key = "key-a" },
    { host = "b", api_key = "key-b" },
]
"#);
    let (main, secrets) = split_secrets(&merged, &["servers.*.api_key"], None).unwrap();

    let arr = main["servers"].as_array().unwrap();
    assert_eq!(arr.len(), 2);
    for v in arr.iter() {
        let it = v.as_inline_table().unwrap();
        assert_eq!(it.get("api_key").unwrap().as_str(), Some(PLACEHOLDER));
    }

    let sec_arr = secrets["servers"].as_array().unwrap();
    assert_eq!(sec_arr.len(), 2);
    assert_eq!(
        sec_arr
            .get(0)
            .unwrap()
            .as_inline_table()
            .unwrap()
            .get("api_key")
            .unwrap()
            .as_str(),
        Some("key-a")
    );
    assert_eq!(
        sec_arr
            .get(1)
            .unwrap()
            .as_inline_table()
            .unwrap()
            .get("api_key")
            .unwrap()
            .as_str(),
        Some("key-b")
    );
}
