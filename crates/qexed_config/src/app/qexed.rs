use serde::{Deserialize, Serialize};

use crate::{
    tool::AppConfigTrait,
};

#[derive(Debug,Serialize, Deserialize)]
pub struct Qexed {
    pub version: i32,
    pub update_check:bool,
}
impl Default for Qexed {
    fn default() -> Self {
        Self { version: Default::default(), update_check: true }
    }
}
impl AppConfigTrait for Qexed {
    const PATH: &'static str = "./config/";

    const NAME: &'static str = "qexed";
}
