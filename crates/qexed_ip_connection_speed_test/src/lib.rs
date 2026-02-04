pub mod message;
rust_i18n::i18n!("../../locales");
use rust_i18n::t;
pub async fn new()->anyhow::Result<tokio::sync::mpsc::UnboundedSender<message::Message>>{
    log::info!("Test");
    let (s,r) = tokio::sync::mpsc::unbounded_channel();
    Ok(s)
}