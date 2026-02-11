pub mod message;
rust_i18n::i18n!("../../locales");
use std::sync::{Arc, atomic::AtomicI32};

use crate::message::Message;
pub async fn new(
    max_players: i32,
) -> anyhow::Result<tokio::sync::mpsc::UnboundedSender<message::Message>> {
    let (s, r) = tokio::sync::mpsc::unbounded_channel();
    Server::new(r, max_players);
    Ok(s)
}
pub struct Server {
    players:Arc<AtomicI32>,
    max_players: i32 ,
}
impl Server {
    pub fn new(
        r: tokio::sync::mpsc::UnboundedReceiver<Message>,
        max_players: i32,
    ) -> tokio::task::JoinHandle<()> {
        let server = Self {
            players: Arc::new(AtomicI32::new(0)),
            max_players: max_players,
        };
        tokio::spawn(server.listen(r))
    }
    pub async fn listen(self, mut r: tokio::sync::mpsc::UnboundedReceiver<Message>) {
        while let Some(event) = r.recv().await {
            match event {
                Message::GetPlayer(api) => {
                    match api.send(self.players.clone()){
                        Ok(_) => {},
                        Err(e) => {
                            log::error!("Failed to send player count: {:?}", e);
                        }
                    };
                }
            };
        }
    }
}
