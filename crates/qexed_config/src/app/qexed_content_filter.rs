use serde::{Deserialize, Serialize};

use crate::app::qexed::server::ContentFilter;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QexedContentFilter {
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
}
