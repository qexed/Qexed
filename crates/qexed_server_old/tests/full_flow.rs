//! 端到端：真实 TCP 上跑完整 状态ping / 登录 / 配置 / 游玩 流程。

use qexed_packet::Packet;
use qexed_protocol::to_client as s2c;
use qexed_protocol::to_server::handshaking::set_protocol::SetProtocol;
use qexed_protocol::to_server::login::login_acknowledged::LoginAcknowledged;
use qexed_protocol::to_server::login::login_start::LoginStart;
use qexed_protocol::to_server::status::ping_start::PingStart;
use qexed_server::transport::Connection;

type C = Connection<tokio::net::tcp::OwnedReadHalf, tokio::net::tcp::OwnedWriteHalf>;

async fn read_packet<T: Packet>(conn: &mut C, what: &str) -> T {
    let opt = conn.reader.read_packet().await.unwrap();
    let mut payload = match opt {
        Some(p) => p,
        None => panic!("closed at {}", what),
    };
    let id = qexed_server::transport::read_packet_id(&mut payload).unwrap();
    assert_eq!(id, T::ID, "packet id at {}", what);
    qexed_server::transport::decode_payload(&mut payload).unwrap()
}

#[tokio::test]
async fn full_login_flow_over_tcp() {
    let config = qexed_server::ServerConfig {
        bind: "127.0.0.1:0".to_string(),
        compression_threshold: 16,
        ..Default::default()
    };
    let handle = qexed_server::serve(config).await.unwrap();
    let addr = handle.local_addr;

    let socket = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (rh, wh) = socket.into_split();
    let mut conn = Connection::new(rh, wh);

    conn.send(&SetProtocol {
        protocol_version: qexed_packet::net_types::VarInt(775),
        server_host: "localhost".to_string(),
        server_port: 25565,
        next_state: qexed_packet::net_types::VarInt(2),
    })
    .await
    .unwrap();

    conn.send(&LoginStart { username: "tester".to_string(), player_uuid: uuid::Uuid::nil() }).await.unwrap();

    let compress: s2c::login::compress::Compress = read_packet(&mut conn, "compress").await;
    assert_eq!(compress.threshold.0, 16);
    conn.set_compression(16);

    let success: s2c::login::success::Success = read_packet(&mut conn, "success").await;
    assert_eq!(success.game_profile.username, "tester");

    conn.send(&LoginAcknowledged::default()).await.unwrap();

    let brand: s2c::configuration::custom_payload::CustomPayload = read_packet(&mut conn, "brand").await;
    assert_eq!(brand.channel, "minecraft:brand");

    let _packs: s2c::configuration::select_known_packs::SelectKnownPacks = read_packet(&mut conn, "known packs").await;
    conn.send(&qexed_protocol::to_server::configuration::select_known_packs::SelectKnownPacks {
        entries: Vec::new(),
    })
    .await
    .unwrap();

    let _finish: s2c::configuration::finish_configuration::FinishConfiguration = read_packet(&mut conn, "config finish").await;
    conn.send(&qexed_protocol::to_server::configuration::finish_configuration::FinishConfiguration::default())
        .await
        .unwrap();

    let join: s2c::play::login::Login = read_packet(&mut conn, "join").await;
    assert_eq!(join.game_mode, 1);

    let _pos: s2c::play::position::Position = read_packet(&mut conn, "position").await;

    handle.shutdown();
}

#[tokio::test]
async fn status_ping_flow() {
    let config = qexed_server::ServerConfig {
        bind: "127.0.0.1:0".to_string(),
        ..Default::default()
    };
    let handle = qexed_server::serve(config).await.unwrap();
    let socket = tokio::net::TcpStream::connect(handle.local_addr).await.unwrap();
    let (rh, wh) = socket.into_split();
    let mut conn = Connection::new(rh, wh);

    conn.send(&SetProtocol {
        protocol_version: qexed_packet::net_types::VarInt(775),
        server_host: "localhost".to_string(),
        server_port: 25565,
        next_state: qexed_packet::net_types::VarInt(1),
    })
    .await
    .unwrap();

    conn.send(&PingStart::default()).await.unwrap();
    let info: s2c::status::server_info::ServerInfo = read_packet(&mut conn, "server info").await;
    let json = info.response.0;
    assert_eq!(json["version"]["protocol"], 775);
    assert!(json["description"]["text"].as_str().unwrap().contains("qexed"));

    handle.shutdown();
}