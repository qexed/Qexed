//! 指令链路探针：最小离线协议客户端，实测 /help 是否收到 SystemChat 回复。
//!
//! 用法: probe_commands.exe [addr] [username]
//! 默认 127.0.0.1:25578 Probe
//!
//! 流程：握手 → LoginStart →（正版认证跳过：服务器需 online_mode=false）
//!       → 配置协商 → Play → 等待 Commands/Position → 发送 /help、/ping
//!       → 收集 SystemChat 输出并打印所有 play 包 id。

use bytes::BytesMut;
use qexed_packet::net_types::VarInt;
use qexed_packet::{Packet, PacketCodec};
use qexed_protocol::to_client;
use qexed_protocol::to_server;
use qexed_tcp_connect::{PacketSink, PacketStream};
use tokio::time::Duration;

fn de<T: Packet + Default>(payload: &mut BytesMut) -> T {
    let mut packet = T::default();
    packet
        .deserialize(&mut qexed_packet::PacketReader::new(payload))
        .expect("decode");
    packet
}

fn read_id(payload: &mut BytesMut) -> i32 {
    let mut id = VarInt::default();
    id.deserialize(&mut qexed_packet::PacketReader::new(payload))
        .expect("packet id");
    id.0
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:25578".into());
    let username = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "Probe".into());
    let port: u16 = addr.rsplit(':').next().unwrap().parse().unwrap();

    let stream = tokio::net::TcpStream::connect(&addr).await.expect("connect");
    let (rd, wr) = tokio::io::split(stream);
    let mut packets = PacketStream::new(rd);
    let mut sink = PacketSink::new(wr);
    println!("[probe] connected to {addr}");

    // ── 握手 + LoginStart ──
    sink.send(to_server::handshaking::set_protocol::SetProtocol {
        protocol_version: VarInt(qexed_mojang_data::PROTOCOL_VERSION),
        server_host: "127.0.0.1".into(),
        server_port: port,
        next_state: VarInt(2),
    })
    .await
    .unwrap();
    sink.send(to_server::login::login_start::LoginStart {
        username: username.clone(),
        player_uuid: uuid::Uuid::from_u128(0x0099_8877_6655_4433_2211_0000_0000_0001),
    })
    .await
    .unwrap();
    sink.flush().await.unwrap();

    let extra_cmds: Vec<String> = std::env::args().skip(3).collect();

    // ── login/configuration 阶段推进 ──
    let mut in_play = false;
    let mut login_done = false;
    let mut saw_commands = false;
    let mut joined = false;
    let mut commands_sent = 0usize;
    let mut chat_lines: Vec<String> = Vec::new();
    let mut play_ids: Vec<String> = Vec::new();
    // 出生流送计时
    let mut t_join: Option<std::time::Instant> = None;
    let mut t_last_chunk: Option<std::time::Instant> = None;
    let mut map_chunks = 0usize;
    let mut chunk_batches = 0usize;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(300);

    loop {
        if tokio::time::Instant::now() >= deadline {
            println!("[probe] global deadline hit");
            break;
        }
        let wait = if in_play && commands_sent >= 2 {
            Duration::from_millis(1200)
        } else {
            Duration::from_secs(10)
        };
        let maybe = match tokio::time::timeout(wait, packets.read_packet()).await {
            Ok(r) => r.expect("read"),
            Err(_elapsed) => {
                if in_play && commands_sent == 0 && saw_commands && joined {
                    println!("[probe] >> /help");
                    sink.send(to_server::play::chat_command::ChatCommand {
                        command: "help".into(),
                    })
                    .await
                    .unwrap();
                    sink.flush().await.unwrap();
                    commands_sent = 1;
                } else if in_play && commands_sent >= 2 {
                    println!("[probe] collect window done");
                    break;
                } else {
                    println!(
                        "[probe] idle timeout: in_play={in_play} saw_commands={saw_commands} joined={joined}"
                    );
                    break;
                }
                continue;
            }
        };
        let Some(mut payload) = maybe else {
            println!("[probe] server closed connection");
            break;
        };

        if !in_play {
            let id = read_id(&mut payload);
            // login 与 configuration 的包 id 空间不同：0x03 在 login 是
            // LoginCompression,在 configuration 是 FinishConfiguration——
            // 用 login_done 区分阶段,避免误解析成错误的包结构。
            if id == to_client::login::compress::Compress::ID && !login_done {
                let c: to_client::login::compress::Compress = de(&mut payload);
                println!("[probe] login compression: threshold={}", c.threshold.0);
                // 真实客户端语义：此包之后两侧按 threshold 压缩（>=threshold 才 zlib）
                packets.set_compression_threshold(c.threshold.0);
                sink.set_compression_threshold(c.threshold.0);
            } else if id == to_client::login::success::Success::ID && !login_done {
                let ok: to_client::login::success::Success = de(&mut payload);
                println!("[probe] login success: {}", ok.game_profile.username);
                login_done = true;
                sink.send(to_server::login::login_acknowledged::LoginAcknowledged {})
                    .await
                    .unwrap();
                sink.flush().await.unwrap();
            } else if id == to_client::login::disconnect::Disconnect::ID && !login_done {
                let d: to_client::login::disconnect::Disconnect = de(&mut payload);
                println!("[probe] login kick: {:?}", d.reason);
                break;
            } else if id == to_client::configuration::select_known_packs::SelectKnownPacks::ID {
                let ask: to_client::configuration::select_known_packs::SelectKnownPacks =
                    de(&mut payload);
                println!("[probe] cfg: known packs ask ({} packs)", ask.known_packs.len());
                sink.send(to_server::configuration::select_known_packs::SelectKnownPacks {
                    entries: ask.known_packs,
                })
                .await
                .unwrap();
                sink.flush().await.unwrap();
            } else if id == to_client::configuration::keep_alive::KeepAlive::ID {
                let ka: to_client::configuration::keep_alive::KeepAlive = de(&mut payload);
                sink.send(to_server::configuration::keep_alive::KeepAlive {
                    keep_alive_id: ka.keep_alive_id,
                })
                .await
                .unwrap();
                sink.flush().await.unwrap();
            } else if id == to_client::configuration::finish_configuration::FinishConfiguration::ID
            {
                println!("[probe] cfg finished -> play");
                sink.send(to_server::configuration::finish_configuration::FinishConfiguration {})
                    .await
                    .unwrap();
                sink.flush().await.unwrap();
                in_play = true;
            } else {
                // registry / tags / feature flags / brand 等一律跳过
            }
            continue;
        }

        // ── PLAY 阶段 ──
        let id = read_id(&mut payload);
        let id_name = format!("{id:#04x}");
        if !play_ids.last().is_some_and(|last| *last == id_name) {
            play_ids.push(id_name);
        }
        match id {
            0x31 => {
                joined = true;
                t_join = Some(std::time::Instant::now());
                println!("[probe] play: join game received");
            }
            0x2d => {
                map_chunks += 1;
                t_last_chunk = Some(std::time::Instant::now());
            }
            0x10 => {
                saw_commands = true;
                let cmds: to_client::play::commands::Commands = de(&mut payload);
                println!(
                    "[probe] play: commands packet received ({} nodes, root {})",
                    cmds.nodes.len(),
                    cmds.root_index.0
                );
            }
            0x48 => {
                let pos: to_client::play::position::Position = de(&mut payload);
                println!(
                    "[probe] play: position sync (teleport {}, y={:.2})",
                    pos.teleport_id.0, pos.y
                );
                sink.send(to_server::play::accept_teleportation::AcceptTeleportation {
                    teleport_id: pos.teleport_id,
                })
                .await
                .unwrap();
                sink.send(to_server::play::move_player_pos::MovePlayerPos {
                    x: pos.x,
                    y: pos.y,
                    z: pos.z,
                    flags: 0,
                })
                .await
                .unwrap();
                sink.send(to_server::play::player_loaded::PlayerLoaded)
                    .await
                    .unwrap();
                sink.flush().await.unwrap();
            }
            0x0b => {
                let done: to_client::play::chunk_batch_finished::ChunkBatchFinished =
                    de(&mut payload);
                chunk_batches += 1;
                sink.send(to_server::play::chunk_batch_received::ChunkBatchReceived {
                    desired_chunks_per_tick: 500.0,
                })
                .await
                .unwrap();
                sink.flush().await.unwrap();
                let _ = done;
            }
            0x2c => {
                let ka: to_client::play::keep_alive::KeepAlive = de(&mut payload);
                sink.send(to_server::play::keep_alive::KeepAlive {
                    keep_alive_id: ka.keep_alive_id,
                })
                .await
                .unwrap();
                sink.flush().await.unwrap();
            }
            0x3d => {
                let ping: to_client::play::ping::Ping = de(&mut payload);
                sink.send(to_server::play::pong::Pong { id: ping.id })
                    .await
                    .unwrap();
                sink.flush().await.unwrap();
            }
            0x79 => {
                let chat: to_client::play::system_chat::SystemChat = de(&mut payload);
                let text = format!("[overlay={}] {:?}", chat.overlay, chat.content);
                println!("[probe] chat << {text}");
                chat_lines.push(text);
            }
            0x20 => {
                let d: to_client::play::disconnect::Disconnect = de(&mut payload);
                println!("[probe] play kick: {d:?}");
                break;
            }
            _ => {}
        }

        // 主循环内：入世界且见到 commands 后,逐条发指令
        // 队列 = extra_cmds(自定义) 或 默认 [help, ping]
        // 触发条件:首条需 joined;后续每收到一条聊天回复发下一条
        let queue: Vec<String> = if extra_cmds.is_empty() {
            vec!["help".into(), "ping".into()]
        } else {
            extra_cmds.iter().map(|c| c.trim_start_matches('/').to_string()).collect()
        };
        if in_play && saw_commands && commands_sent == 0 && joined {
            println!("[probe] >> /{}", queue[0]);
            sink.send(to_server::play::chat_command::ChatCommand {
                command: queue[0].clone(),
            })
            .await
            .unwrap();
            sink.flush().await.unwrap();
            commands_sent = 1;
        } else if in_play && commands_sent >= 1 && !chat_lines.is_empty() {
            let last_reply = chat_lines.len();
            let sent = commands_sent as usize;
            if sent < queue.len() && last_reply >= sent {
                println!("[probe] >> /{}", queue[sent]);
                sink.send(to_server::play::chat_command::ChatCommand {
                    command: queue[sent].clone(),
                })
                .await
                .unwrap();
                sink.flush().await.unwrap();
                commands_sent += 1;
            }
        }
    }

    println!("[probe] ===== summary =====");
    println!("[probe] joined={joined} commands_packet={saw_commands}");
    println!("[probe] chat lines: {}", chat_lines.len());
    for line in &chat_lines {
        println!("[probe]   {line}");
    }
    if let (Some(t0), Some(t1)) = (t_join, t_last_chunk) {
        println!(
            "[probe] spawn stream: {map_chunks} MapChunks in {chunk_batches} batches, \
             {} ms (join -> last chunk)",
            (t1 - t0).as_millis()
        );
    }
    println!("[probe] play ids seen: {}", play_ids.join(" "));
}
