pub mod handshaking;
pub mod message;
pub mod status;
rust_i18n::i18n!("../../locales");
use tokio::sync::mpsc::UnboundedSender;

pub async fn new(
    players_api: UnboundedSender<qexed_players::message::Message>,
    max_players: i32,
    motd: Vec<String>,
    favicon: String,
) -> anyhow::Result<(
    UnboundedSender<(tokio::net::TcpStream, std::net::SocketAddr)>,
    UnboundedSender<(qexed_tcp_connect::PacketRead, qexed_tcp_connect::PacketSend)>,
)> {
    let (sh, rh) = tokio::sync::mpsc::unbounded_channel();
    let (ss, rs) = tokio::sync::mpsc::unbounded_channel();
    handshaking::Handshaking::new(rh, ss.clone());
    let (s, r) = tokio::sync::oneshot::channel();
    players_api.send(qexed_players::message::Message::GetPlayer(s))?;
    status::Status::new(rs, r.await?, max_players,motd,favicon);
    Ok((sh, ss))
}
