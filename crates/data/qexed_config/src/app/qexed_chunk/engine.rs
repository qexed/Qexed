use qexed_config_macros::AutoEnum;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, AutoEnum)]
pub enum Engine{
    Original,// 原版
}