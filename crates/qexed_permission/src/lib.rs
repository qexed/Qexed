use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PermissionNode(String);

#[derive(Debug, Clone, Default)]
pub struct PermissionSet {
    nodes: HashSet<PermissionNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionDecision {
    Allow,
    Deny,
}

impl PermissionNode {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for PermissionNode {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for PermissionNode {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl PermissionSet {
    pub fn grant(&mut self, node: impl Into<PermissionNode>) {
        self.nodes.insert(node.into());
    }

    pub fn revoke(&mut self, node: &str) {
        self.nodes.remove(&PermissionNode::new(node));
    }

    pub fn check(&self, node: &str) -> PermissionDecision {
        if self.allows(node) {
            PermissionDecision::Allow
        } else {
            PermissionDecision::Deny
        }
    }

    pub fn allows(&self, node: &str) -> bool {
        self.nodes.contains(&PermissionNode::new("*"))
            || self.nodes.contains(&PermissionNode::new(node))
            || wildcard_prefixes(node)
                .any(|prefix| self.nodes.contains(&PermissionNode::new(prefix)))
    }
}

fn wildcard_prefixes(node: &str) -> impl Iterator<Item = String> + '_ {
    node.match_indices('.')
        .map(|(index, _)| format!("{}.*", &node[..index]))
}

#[cfg(test)]
mod tests {
    use super::{PermissionDecision, PermissionSet};

    #[test]
    fn exact_permission_allows_node() {
        let mut permissions = PermissionSet::default();
        permissions.grant("qexed.command.help");

        assert_eq!(
            permissions.check("qexed.command.help"),
            PermissionDecision::Allow
        );
        assert_eq!(
            permissions.check("qexed.command.version"),
            PermissionDecision::Deny
        );
    }

    #[test]
    fn wildcard_permission_allows_children() {
        let mut permissions = PermissionSet::default();
        permissions.grant("qexed.command.*");

        assert!(permissions.allows("qexed.command.help"));
        assert!(!permissions.allows("qexed.chat.send"));
    }

    #[test]
    fn global_wildcard_allows_everything() {
        let mut permissions = PermissionSet::default();
        permissions.grant("*");

        assert!(permissions.allows("qexed.anything"));
    }
}
