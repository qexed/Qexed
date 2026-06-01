use serde::{Deserialize, Serialize};

use crate::app::qexed::server::Permissions;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QexedPermissions {
    pub permissions: Permissions,
}

impl Default for QexedPermissions {
    fn default() -> Self {
        Self {
            permissions: Permissions::default(),
        }
    }
}

impl qexed_config::tool::AppConfigTrait for QexedPermissions {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed_permissions";
}
