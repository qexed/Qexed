use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeutralMob {
    #[serde(flatten)]
    pub angry_at:Vec<i32>,
    pub anger_end_time:i64,
}