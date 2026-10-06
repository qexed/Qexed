#[qexed_packet_macros::packet(id = 0x6)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ResetChat {}
impl ResetChat {
    pub fn new() -> Self {
        ResetChat {}
    }
}
