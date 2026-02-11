use qexed_protocol::to_server::handshaking::set_protocol::SetProtocol;

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
            tokio::spawn(Handshaking::task(socket, socket_addr,self.status_r.clone()));
        }
    }
    pub async fn task(
        socket: tokio::net::TcpStream,
        _socket_addr: std::net::SocketAddr,
        status_r: tokio::sync::mpsc::UnboundedSender<(
            qexed_tcp_connect::PacketRead,
            qexed_tcp_connect::PacketSend,
        )>,
    )->anyhow::Result<()> {
        let (mut r, s) = qexed_tcp_connect::PacketListener::from_socket(socket, 0).split();
        let set_protocol = qexed_tcp_connect::read_one_packet::<SetProtocol>(&mut r).await?;
        if set_protocol.next_state.0 == 1 {
            status_r.send((r, s)).unwrap();
        }
        Ok(())
    }
}
