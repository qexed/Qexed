pub mod message;
pub mod handshaking;
pub mod status;
rust_i18n::i18n!("../../locales");
use tokio::sync::mpsc::UnboundedSender;

pub async fn new() -> anyhow::Result<(
    UnboundedSender<(tokio::net::TcpStream,std::net::SocketAddr)>,
    UnboundedSender<(qexed_tcp_connect::PacketRead, qexed_tcp_connect::PacketSend)>,
)> {
    let (sh, rh) = tokio::sync::mpsc::unbounded_channel();
    let (ss, rs) = tokio::sync::mpsc::unbounded_channel();
    handshaking::Handshaking::new(rh,ss.clone());
    status::Status::new(rs);
    Ok(
        (
            sh,
            ss
        )
    )
}
