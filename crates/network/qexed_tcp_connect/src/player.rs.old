/// 玩家信息
#[derive(Debug,Clone)]
pub struct Player {
    /// 玩家名字
    pub name:String,
    /// 玩家uuid
    pub uuids:uuid::Uuid,
    /// 玩家客户端类型
    pub client_type:String,
    /// 玩家语言
    pub locale: String,
    /// 视野距离
    pub view_distance:i8,
}

impl Player {
    pub fn new()->Self{
        Player {
            name:"".to_owned(),
            uuids:uuid::Uuid::nil(),
            client_type:"".to_owned(),
            locale: "zh_cn".to_owned(),
            view_distance:21,
        }
    }
}