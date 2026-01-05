use tokio::sync::{mpsc::UnboundedSender, oneshot};


#[derive(Debug, Clone)]
pub enum ManagerCommand {
    Start,
    Register(uuid::Uuid,UnboundedSender<TaskCommand>),
    GetEntity(uuid::Uuid,Option<UnboundedSender<TaskCommand>>),
    RegisterFinish,
    SpiltIDTaskClose,
    TaskClose(uuid::Uuid,i32),
}
#[derive(Debug)]
pub enum TaskCommand {
    Start,
    GetentityID(oneshot::Sender<i32>),
    AssignID(i32),
    GetAddEntityPacket(oneshot::Sender<qexed_protocol::to_client::play::add_entity::AddEntity>),
    Close,
}

#[derive(Debug)]
pub enum SpiltIDCommand {
    Start,
    New(oneshot::Sender<i32>),
    Recycled(i32)
}