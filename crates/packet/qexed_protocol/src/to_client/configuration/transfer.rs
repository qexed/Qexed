#[qexed_packet_macros::packet(id = 0x0b)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Transfer {
}
impl Transfer {
    pub fn new() -> Self {
        Transfer {
        }
    }
}
