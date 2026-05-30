use qexed_config_macros::AutoDoc;
use serde::{Deserialize, Serialize};

use crate::app::qexed::server::ContentFilter;

#[derive(Debug, Clone, Serialize, Deserialize, AutoDoc)]
pub struct QexedContentFilter {
    #[AutoDoc(key = "config.qexed.server.content_filter", sub)]
    pub content_filter: ContentFilter,
}

impl Default for QexedContentFilter {
    fn default() -> Self {
        Self {
            content_filter: ContentFilter::default(),
        }
    }
}

impl qexed_config::tool::AppConfigTrait for QexedContentFilter {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed_content_filter";

    fn config_file_description(lang: &str, config_file: &str, _root_path: Option<&str>) -> String {
        rust_i18n::t!(
            "autodoc.file_description.qexed.content_filter",
            locale = lang,
            file = config_file
        )
        .to_string()
    }
}
