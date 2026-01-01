use qexed_command::message::CommandData;

#[derive(Debug)]
pub enum ManagerMessage {
    Command(CommandData),
}
