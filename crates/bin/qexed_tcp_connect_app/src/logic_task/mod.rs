use qexed_task::{event::task::TaskEvent, message::{MessageSender, return_message::ReturnMessage, unreturn_message::UnReturnMessage}};

use crate::messages::{LogicCommand, ManagerCommand};

pub mod read_task;
pub mod write_task;

#[derive(Debug)]
pub struct LogicTask {

}
impl LogicTask {
    pub fn new(stream:tokio::net::TcpStream,addr:std::net::SocketAddr)->Self{
        Self{}
    }
    
}

#[async_trait::async_trait]
impl TaskEvent<UnReturnMessage<LogicCommand>,ReturnMessage<ManagerCommand>> for LogicTask {
    async fn event(
        &mut self,
        api: &MessageSender<ReturnMessage<LogicCommand>>,
        manage_api: &MessageSender<ReturnMessage<ManagerCommand>>,
        mut data: ReturnMessage<LogicCommand>,
    ) -> anyhow::Result<bool> {
        
        Ok(false)
    }
}

