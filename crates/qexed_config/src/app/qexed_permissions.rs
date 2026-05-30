use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};

use crate::app::qexed::server::Permissions;

#[derive(Debug, Clone, Serialize, Deserialize, AutoDoc)]
pub struct QexedPermissions {
    #[AutoDoc(key = "config.qexed.server.permissions", sub)]
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

    fn config_file_description(lang: &str, config_file: &str, _root_path: Option<&str>) -> String {
        rust_i18n::t!(
            "autodoc.file_description.qexed.permissions",
            locale = lang,
            file = config_file
        )
        .to_string()
    }
}
