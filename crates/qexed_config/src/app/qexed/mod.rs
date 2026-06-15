use serde::{Deserialize, Serialize};
pub mod args;
#[derive(Debug, Serialize, Deserialize)]
pub struct Qexed {

}

impl Qexed {

}

impl Default for Qexed {
    fn default() -> Self {
        Self {

        }
    }
}

impl qexed_config::tool::AppConfigTrait for Qexed {
    const PATH: &'static str = "/";
    const NAME: &'static str = "qexed";
}

