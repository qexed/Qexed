use std::sync::{Arc, atomic::AtomicI32};

use tokio::sync::oneshot;

pub enum Message {
    GetPlayer(oneshot::Sender<Arc<AtomicI32>>),
}