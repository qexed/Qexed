use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize,Default)]
pub struct BlockEntityData {
    pub id:String,
    // TODO:后面再完善,无限套娃下去我肯定做不完
    // 开源后继续完善
}