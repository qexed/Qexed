use anyhow::Result;
use std::path::PathBuf;

const GROUP_PREFIX: &str = "group.";
const WILDCARD: &str = "*";
#[derive(Debug, Default)]
pub(super) struct PermissionSnapshot {
    pub(super) nodes: Vec<PermissionNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(super) struct PermissionNode {
    permission: String,
    value: bool,
}

impl<'de> serde::Deserialize<'de> for PermissionNode {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum NodeValue {
            Text(String),
            Detailed { permission: String, value: bool },
        }

        match NodeValue::deserialize(deserializer)? {
            NodeValue::Text(permission) => Ok(PermissionNode::new(permission, true)),
            NodeValue::Detailed { permission, value } => Ok(PermissionNode::new(permission, value)),
        }
    }
}

impl PermissionNode {
    pub(super) fn new(permission: impl Into<String>, value: bool) -> Self {
        Self {
            permission: normalize_node(permission.into()),
            value,
        }
    }

    pub(super) fn inherited_group(&self) -> Option<&str> {
        self.value
            .then(|| self.permission.strip_prefix(GROUP_PREFIX))
            .flatten()
            .filter(|group| !group.is_empty())
    }

    pub(super) fn matches_exact_permission(&self, permission: &str) -> bool {
        self.permission == normalize_node(permission)
    }
}

pub(super) fn resolve_permission(nodes: &[PermissionNode], permission: &str) -> Option<bool> {
    let permission = normalize_node(permission);

    for node in nodes.iter().rev() {
        if node_matches(&node.permission, &permission) {
            return Some(node.value);
        }
    }

    None
}

fn node_matches(node: &str, permission: &str) -> bool {
    node == permission || node == WILDCARD || wildcard_matches(node, permission)
}

fn wildcard_matches(node: &str, permission: &str) -> bool {
    node.strip_suffix(".*")
        .is_some_and(|prefix| permission == prefix || permission.starts_with(&format!("{prefix}.")))
}

fn normalize_node(permission: impl AsRef<str>) -> String {
    permission.as_ref().trim().to_ascii_lowercase()
}

pub(super) fn normalize_group(group: impl AsRef<str>) -> String {
    group.as_ref().trim().to_ascii_lowercase()
}

pub(super) fn normalize_user_key(user: impl AsRef<str>) -> String {
    user.as_ref().trim().to_ascii_lowercase()
}

pub(super) fn normalize_context(context: impl AsRef<str>) -> String {
    let context = context.as_ref().trim();
    if context.is_empty() {
        "global".to_string()
    } else {
        context.to_string()
    }
}

pub(super) fn local_permission_path(path: &str) -> Result<PathBuf> {
    let path = path.trim();
    if path.is_empty() {
        anyhow::bail!("local permission file path cannot be empty");
    }
    Ok(PathBuf::from(path))
}

pub(super) fn contexts_match(contexts: &str) -> bool {
    let contexts = contexts.trim();
    if contexts.is_empty() || contexts == "{}" {
        return true;
    }

    let Ok(value) = serde_json::from_str::<serde_json::Value>(contexts) else {
        return false;
    };
    let Some(object) = value.as_object() else {
        return false;
    };

    object.is_empty()
}

pub(super) fn validate_sql_identifier(value: &str, label: &str) -> Result<()> {
    let valid = value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
    if valid {
        Ok(())
    } else {
        anyhow::bail!("{label} may only contain ASCII letters, digits, and underscore")
    }
}
