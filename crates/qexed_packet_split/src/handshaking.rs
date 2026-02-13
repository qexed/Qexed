use qexed_protocol::to_server::handshaking::set_protocol::SetProtocol;
use rust_i18n::t;

pub struct Handshaking {
    status_r: tokio::sync::mpsc::UnboundedSender<(
        qexed_tcp_connect::PacketRead,
        qexed_tcp_connect::PacketSend,
    )>,
}
impl Handshaking {
    pub fn new(
        r: tokio::sync::mpsc::UnboundedReceiver<(tokio::net::TcpStream, std::net::SocketAddr)>,
        status_r: tokio::sync::mpsc::UnboundedSender<(
            qexed_tcp_connect::PacketRead,
            qexed_tcp_connect::PacketSend,
        )>,
    ) -> tokio::task::JoinHandle<()> {
        let server = Self { status_r: status_r };
        tokio::spawn(server.listen(r))
    }
    pub async fn listen(
        self,
        mut r: tokio::sync::mpsc::UnboundedReceiver<(tokio::net::TcpStream, std::net::SocketAddr)>,
    ) {
        while let Some((socket, socket_addr)) = r.recv().await {
            tokio::spawn(Handshaking::task(
                socket,
                socket_addr,
                self.status_r.clone(),
            ));
        }
    }
    pub async fn task(
        socket: tokio::net::TcpStream,
        _socket_addr: std::net::SocketAddr,
        status_r: tokio::sync::mpsc::UnboundedSender<(
            qexed_tcp_connect::PacketRead,
            qexed_tcp_connect::PacketSend,
        )>,
    ) -> anyhow::Result<()> {
        let (mut r, mut s) = qexed_tcp_connect::PacketListener::from_socket(socket, 0).split();
        let set_protocol = qexed_tcp_connect::read_one_packet::<SetProtocol>(&mut r).await?;
        if set_protocol.next_state.0 == 1 {
            status_r.send((r, s)).unwrap();
            return Ok(());
        } else if set_protocol.next_state.0 != 2 {
            log::error!(
                "{}",
                t!(
                    "qexed_packet_split.unknown_next_state",
                    state = set_protocol.next_state.0
                )
            );
            return Ok(());
        }
        if set_protocol.protocol_version.0 != qexed_config::PROTOCOL_VERSION {
            // 下个阶段:但是服务端没写完
            let server_info = qexed_protocol::to_client::login::disconnect::Disconnect {
                reason: serde_json::json!({
                    "text": t!("qexed_packet_split.incompatible_game_version",mc_server= qexed_config::MC_VERSION),
                    "color": "red",
                    "bold": true
                }),
            };
            s.send(server_info).await?;
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            return Ok(());
        }
        Ok(())
    }
}
