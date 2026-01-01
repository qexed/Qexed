#[derive(Debug,Clone,Default)]
pub struct Player{
    pub uuid:uuid::Uuid,
    pub username:String,
    pub properties:Vec<qexed_protocol::to_client::login::success::Properties>,
    pub data:Option<qexed_data_serde::entity::living_entity::avatar::player::Player>,
}
