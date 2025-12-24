use async_trait::async_trait;
use bytes::Bytes;
use dashmap::DashMap;
use qexed_packet::{PacketCodec, net_types::{Bitset, VarInt}};
use qexed_player::Player;
use qexed_protocol::{
    to_server::configuration::{
        cookie_response::CookieResponse, custom_click_action::CustomClickAction,
        custom_payload::CustomPayload, finish_configuration::FinishConfiguration,
        keep_alive::KeepAlive, pong::Pong, resource_pack_receive::ResourcePackReceive,
    },
    types::KnownPacks,
};
use qexed_task::{
    event::task::TaskEvent,
    message::{
        MessageSender, MessageType, return_message::ReturnMessage,
        unreturn_message::UnReturnMessage,
    },
};
use qexed_tcp_connect::PacketSend;
use rsa::pkcs8::der::asn1::Null;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use uuid::Uuid;

use crate::message::{ManagerMessage, TaskMessage};

#[derive(Debug)]
pub struct GameLogicActor {
    uuid: Uuid,
    player: Option<Player>,
    packet_read: Option<UnboundedReceiver<Vec<u8>>>,
    packet_write: Option<UnboundedSender<Bytes>>,
    qexed_ping_api: Option<UnboundedSender<UnReturnMessage<qexed_ping::message::TaskCommand>>>,
    qexed_heartbeat_api:
        Option<UnboundedSender<UnReturnMessage<qexed_heartbeat::message::TaskCommand>>>,
    qexed_packet_split_api:
        Option<UnboundedSender<ReturnMessage<qexed_packet_split::message::TaskMessage>>>,
    qexed_chat_api:Option<UnboundedSender<UnReturnMessage<qexed_chat::message::TaskMessage>>>,
    qexed_command_api:Option<UnboundedSender<UnReturnMessage<qexed_command::message::TaskCommand>>>,
    qexed_player_list_api:Option<UnboundedSender<ReturnMessage<qexed_player_list::Message>>>,
}
impl GameLogicActor {
    pub fn new(uuid: Uuid) -> Self {
        Self {
            uuid,
            player: None,
            packet_read: None,
            packet_write: None,
            qexed_ping_api: None,
            qexed_heartbeat_api: None,
            qexed_packet_split_api: None,
            qexed_chat_api:None,
            qexed_command_api:None,
            qexed_player_list_api:None,
        }
    }
}
#[async_trait]
impl TaskEvent<ReturnMessage<TaskMessage>, ReturnMessage<ManagerMessage>> for GameLogicActor {
    async fn event(
        &mut self,
        api: &MessageSender<ReturnMessage<TaskMessage>>,
        manage_api: &MessageSender<ReturnMessage<ManagerMessage>>,
        mut data: ReturnMessage<TaskMessage>,
    ) -> anyhow::Result<bool> {
        match data.data {
            TaskMessage::Start(
                ref player,
                ref mut unbounded_receiver,
                ref mut unbounded_sender,
            ) => {
                // 玩家进入了服务器
                self.player = Some(player.clone());
                self.packet_read = unbounded_receiver.take();
                self.packet_write = unbounded_sender.take();
                let packet_write = match self.packet_write.clone() {
                    Some(p) => p,
                    None => {
                        if let Some(send) = data.get_return_send().await? {
                            let _ = send.send(data.data);
                        }
                        return Ok(false);
                    }
                };

                // 获取目标对象并初始化
                if let ManagerMessage::GetPlayerPing(Some(
                    qexed_ping::message::ManagerCommand::NewPlayerConnect(
                        _uuid,
                        is_true,
                        err,
                        api_ping,
                        _ps,
                    ),
                )) = ReturnMessage::build(ManagerMessage::GetPlayerPing(Some(
                    qexed_ping::message::ManagerCommand::NewPlayerConnect(
                        self.uuid.clone(),
                        false,
                        None,
                        None,
                        Some(packet_write.clone()),
                    ),
                )))
                .get(&manage_api)
                .await?
                {
                    self.qexed_ping_api = api_ping;
                }
                // 获取目标对象并初始化
                if let ManagerMessage::GetPlayerHeartbeat(Some(
                    qexed_heartbeat::message::ManagerCommand::NewPlayerConnect(
                        _uuid,
                        is_true,
                        err,
                        heartbeat_api,
                        _ps,
                    ),
                )) = ReturnMessage::build(ManagerMessage::GetPlayerHeartbeat(Some(
                    qexed_heartbeat::message::ManagerCommand::NewPlayerConnect(
                        self.uuid.clone(),
                        false,
                        None,
                        None,
                        Some(packet_write.clone()),
                    ),
                )))
                .get(&manage_api)
                .await?
                {
                    self.qexed_heartbeat_api = heartbeat_api;
                }
                // 获取目标对象并初始化
                if let ManagerMessage::GetPlayerPacketSplit(Some(
                    qexed_packet_split::message::ManagerMessage::NewPlayerConnect(
                        _uuid,
                        is_true,
                        err,
                        packet_split_api,
                    ),
                )) = ReturnMessage::build(ManagerMessage::GetPlayerPacketSplit(Some(
                    qexed_packet_split::message::ManagerMessage::NewPlayerConnect(
                        self.uuid.clone(),
                        false,
                        None,
                        None,
                    ),
                )))
                .get(&manage_api)
                .await?
                {
                    self.qexed_packet_split_api = packet_split_api;
                }

                if let Some(send) = data.get_return_send().await? {
                    let _ = send.send(data.data);
                }
                return Ok(false);
            }
            TaskMessage::Configuration(ref mut is_true) => {
                // 配置阶段
                let mut packet_read = match self.packet_read.take() {
                    Some(p) => p,
                    None => {
                        if let Some(send) = data.get_return_send().await? {
                            let _ = send.send(data.data);
                        }
                        return Ok(false);
                    }
                };
                let mut packet_write = match &self.packet_write.clone() {
                    Some(p) => p.clone(),
                    None => {
                        if let Some(send) = data.get_return_send().await? {
                            let _ = send.send(data.data);
                        }
                        return Ok(false);
                    }
                };
                let mut player_name: String = "无名".to_string().to_owned();
                let mut player_uuid: uuid::Uuid = uuid::Uuid::nil();
                let mut locale: String = "zh_cn".to_owned();
                let mut view_distance: i8 = 12;
                // if let Some(api_ping) = &self.qexed_ping_api {
                //     UnReturnMessage::build(qexed_ping::message::TaskCommand::UpdatePart(
                //         qexed_ping::message::Part::Configuration,
                //     ))
                //     .post(&api_ping)
                //     .await?;
                //     // UnReturnMessage::build(qexed_ping::message::TaskCommand::Start).post(&api_ping).await?;
                // }
                while let Some(raw_data) = packet_read.recv().await {
                    let mut buf: bytes::BytesMut = bytes::BytesMut::new();
                    buf.extend_from_slice(&raw_data);
                    let mut reader = qexed_packet::PacketReader::new(Box::new(&mut buf));
                    let mut id: qexed_packet::net_types::VarInt = Default::default();
                    id.deserialize(&mut reader)?;
                    match id.0 {
                        0x00 => {
                            let pk = qexed_tcp_connect::decode_packet::<
                                qexed_protocol::to_server::configuration::settings::Settings,
                            >(&mut reader)?;
                            locale = pk.locale;
                            view_distance = pk.view_distance;
                            packet_write.send(
                                PacketSend::build_send_packet(qexed_protocol::to_client::configuration::select_known_packs::SelectKnownPacks {
                                    known_packs: vec![KnownPacks {
                                        namespace: "minecraft".to_string(),
                                        id: "core".to_string(),
                                        version: "1.21.8".to_string(),
                                    }],
                                })
                                .await?,
                            )?;
                        }
                        0x01 => {
                            let pk =
                                qexed_tcp_connect::decode_packet::<CookieResponse>(&mut reader)?;
                        }
                        0x02 => {
                            let pk =
                                qexed_tcp_connect::decode_packet::<CustomPayload>(&mut reader)?;
                        }
                        0x03 => {
                            let pk = qexed_tcp_connect::decode_packet::<FinishConfiguration>(
                                &mut reader,
                            )?;
                            *is_true = true;
                            // if let Some(api_ping) =&self.qexed_ping_api{
                            //     let (w,r) = tokio::sync::oneshot::channel();
                            //     UnReturnMessage::build(qexed_ping::message::TaskCommand::Await(w)).post(&api_ping).await?;
                            //     r.await?;
                            //     UnReturnMessage::build(qexed_ping::message::TaskCommand::Stop).post(&api_ping).await?;
                            // }
                            break;
                        }
                        0x04 => {
                            let pk = qexed_tcp_connect::decode_packet::<KeepAlive>(&mut reader)?;
                        }
                        0x05 => {
                            let pk = qexed_tcp_connect::decode_packet::<Pong>(&mut reader)?;
                            if let Some(api_ping) = &self.qexed_ping_api {
                                let _ = UnReturnMessage::build(
                                    qexed_ping::message::TaskCommand::Pong(pk.id),
                                )
                                .post(&api_ping)
                                .await;
                            }
                        }
                        0x06 => {
                            let pk = qexed_tcp_connect::decode_packet::<ResourcePackReceive>(
                                &mut reader,
                            )?;
                        }
                        0x07 => {
                            let pk =
                                qexed_tcp_connect::decode_packet::<qexed_protocol::to_server::configuration::select_known_packs::SelectKnownPacks>(&mut reader)?;
                            if let ManagerMessage::Registry(d1, d2) =
                                ReturnMessage::build(ManagerMessage::Registry(None, None))
                                    .get(&manage_api)
                                    .await?
                            {
                                if let Some(d1) = d1 {
                                    for p in d1 {
                                        packet_write
                                            .send(PacketSend::build_send_packet(p).await?)?;
                                    }
                                }
                                if let Some(d2) = d2 {
                                    packet_write.send(PacketSend::build_send_packet(d2).await?)?;
                                }
                            }
                            packet_write.send(
                                PacketSend::build_send_packet(FinishConfiguration {}).await?,
                            )?;
                        }
                        0x08 => {
                            let pk =
                                qexed_tcp_connect::decode_packet::<CustomClickAction>(&mut reader)?;
                        }
                        _ => {}
                    }
                }
                self.packet_read = Some(packet_read);

                if let Some(send) = data.get_return_send().await? {
                    let _ = send.send(data.data);
                }
                return Ok(false);
            }
            TaskMessage::Play => {
                let mut player = match self.player.clone() {
                    Some(p) => p,
                    None => {
                        if let Some(send) = data.get_return_send().await? {
                            let _ = send.send(data.data);
                        }
                        return Ok(false);
                    }
                };
                let mut packet_read = match self.packet_read.take() {
                    Some(p) => p,
                    None => {
                        if let Some(send) = data.get_return_send().await? {
                            let _ = send.send(data.data);
                        }
                        return Ok(false);
                    }
                };
                let mut packet_write = match &self.packet_write.clone() {
                    Some(p) => p.clone(),
                    None => {
                        if let Some(send) = data.get_return_send().await? {
                            let _ = send.send(data.data);
                        }
                        return Ok(false);
                    }
                };
                packet_write.send(
                    PacketSend::build_send_packet(qexed_protocol::to_client::play::login::Login {
                        entity_id: 1,
                        is_hardcore: false,
                        dimension_names: vec![
                            "minecraft:overworld".to_string(),
                            "minecraft:the_end".to_string(),
                            "minecraft:the_nether".to_string(),
                        ],
                        max_player: qexed_packet::net_types::VarInt(20),
                        view_distance: qexed_packet::net_types::VarInt(12),
                        simulation_distance: qexed_packet::net_types::VarInt(12),
                        reduced_debug_info: false,
                        enable_respawn_screen: false,
                        do_limited_crafting: false,
                        dimension_type: qexed_packet::net_types::VarInt(0),
                        dimension_name: "minecraft:overworld".to_string(),
                        hashed_seed: 114514,
                        game_mode: 0,
                        previous_game_mode: -1,
                        is_debug: false,
                        is_flat: false,
                        has_death_location: false,
                        death_dimension_name: None,
                        death_position: None,
                        portal_cooldown: qexed_packet::net_types::VarInt(0),
                        sea_level: qexed_packet::net_types::VarInt(63),
                        enforces_secure_chat: false,
                    })
                    .await?,
                )?;
                // 初始化成就
                packet_write.send(
                    PacketSend::build_send_packet(create_multiple_advancements_packet())
                    .await?,
                )?;

                let heartbeat_api = match &self.qexed_heartbeat_api {
                    Some(p) => p,
                    None => {
                        if let Some(send) = data.get_return_send().await? {
                            let _ = send.send(data.data);
                        }
                        return Ok(false);
                    }
                };
                // 获取目标对象并初始化
                // 聊天
                if let ManagerMessage::GetPlayerChat(Some(
                    qexed_chat::message::ManagerMessage::NewPlayerConnect(
                        _uuid,
                        is_true,
                        err,
                        chat_api,
                    ),
                )) = ReturnMessage::build(ManagerMessage::GetPlayerChat(Some(
                    qexed_chat::message::ManagerMessage::NewPlayerConnect(
                        self.uuid.clone(),
                        false,
                        None,
                        None,
                    ),
                )))
                .get(&manage_api)
                .await?
                {
                    self.qexed_chat_api = chat_api;
                }
                let chat_api = match &self.qexed_chat_api {
                    Some(p) => p,
                    None => {
                        if let Some(send) = data.get_return_send().await? {
                            let _ = send.send(data.data);
                        }
                        return Ok(false);
                    }
                };
                // 指令
                if let ManagerMessage::GetCommand(Some(
                    qexed_command::message::ManagerCommand::NewPlayerConnect(
                        _uuid,
                        _username,
                        is_true,
                        err,
                        command_api,
                        _,
                    ),
                )) = ReturnMessage::build(ManagerMessage::GetCommand(Some(
                    qexed_command::message::ManagerCommand::NewPlayerConnect(
                        self.uuid.clone(),
                        player.username.clone(),
                        false,
                        None,
                        None,
                        Some(packet_write.clone()),
                    ),
                )))
                .get(&manage_api)
                .await?
                {
                    self.qexed_command_api = command_api;
                }
                // 玩家列表
                if let ManagerMessage::GetPlayerListApi(player_list_api) = ReturnMessage::build(ManagerMessage::GetPlayerListApi(None)).get(&manage_api).await?{
                    self.qexed_player_list_api = player_list_api;
                }
                let chat_api = match &self.qexed_chat_api {
                    Some(p) => p,
                    None => {
                        if let Some(send) = data.get_return_send().await? {
                            let _ = send.send(data.data);
                        }
                        return Ok(false);
                    }
                };
                let command_api = match &self.qexed_command_api {
                    Some(p) => p,
                    None => {
                        if let Some(send) = data.get_return_send().await? {
                            let _ = send.send(data.data);
                        }
                        return Ok(false);
                    }
                };
                let player_list_api = match &self.qexed_player_list_api {
                    Some(p) => p,
                    None => {
                        if let Some(send) = data.get_return_send().await? {
                            let _ = send.send(data.data);
                        }
                        return Ok(false);
                    }
                };
                UnReturnMessage::build(qexed_command::message::TaskCommand::InitCommandPacket).post(&command_api)
                    .await?;
                UnReturnMessage::build(qexed_chat::message::TaskMessage::Start(player.username.clone(),Some(packet_write.clone())))
                    .post(&chat_api)
                    .await?;
                ReturnMessage::build(qexed_player_list::Message::PlayerJoin(player.uuid.clone(),player.username.clone())).get(&player_list_api).await?;
                // 区块初始化:
                packet_write.send(
                    PacketSend::build_send_packet(qexed_protocol::to_client::play::game_state_change::GameStateChange{reason:13,game_mode:0.0}).await?)?;
                // 构建 SetChunkCacheCenter 数据包
                packet_write.send(PacketSend::build_send_packet(qexed_protocol::to_client::play::update_view_position::UpdateViewPosition::default()).await?)?;
                // 发送玩家附近区块

                let radius = { 12 as i32};
                let mut chunks_pos: Vec<(i32, i32)> = vec![];
                for x in -radius..=radius {
                    for z in -radius..=radius {
                        chunks_pos.push((x, z));
                        // 创建空区块
                        let p_q = qexed_protocol::to_client::play::map_chunk::MapChunk {
                            chunk_x: x,
                            chunk_z: z,
                            data: qexed_protocol::to_client::play::map_chunk::Chunk {
                                // 高度图 - 使用修复后的高度图
                                heightmaps: create_heightmaps(),
                                // 空的区块数据 - 使用修复后的编码函数
                                data: encode_empty_chunk_data_1_21(),
                                // 无方块实体
                                block_entities: vec![],
                            },
                            light: create_light_data_for_all_sections(),
                        };
                        packet_write.send(PacketSend::build_send_packet(p_q).await?)?;
                    }
                }
                UnReturnMessage::build(qexed_heartbeat::message::TaskCommand::Start)
                    .post(&heartbeat_api)
                    .await?;
                // Test:Set Title
                packet_write.send(PacketSend::build_send_packet(qexed_protocol::to_client::play::set_title_text::SetTitleText{
                    text:create_text_nbt("测试title"),
                }).await?)?;
                
                // let api_ping = match &self.qexed_ping_api {
                //     Some(p) => p,
                //     None => {
                //         if let Some(send) = data.get_return_send().await? {
                //             let _ = send.send(data.data);
                //         }
                //         return Ok(false);
                //     }
                // };
                let packet_split_api = match &self.qexed_packet_split_api {
                    Some(p) => p,
                    None => {
                        if let Some(send) = data.get_return_send().await? {
                            let _ = send.send(data.data);
                        }
                        return Ok(false);
                    }
                };
                ReturnMessage::build(qexed_packet_split::message::TaskMessage::Start(
                    player,
                    Some(packet_read),
                    Some(packet_write.clone()),
                    // Some(api_ping.clone()),
                    Some(heartbeat_api.clone()),
                    Some(chat_api.clone()),
                    Some(command_api.clone()),
                ))
                .get(&packet_split_api)
                .await?;
                ReturnMessage::build(qexed_packet_split::message::TaskMessage::Run)
                    .get(&packet_split_api)
                    .await?;

                return Ok(false);
            }
            TaskMessage::Close => {
                if let Some(api_ping) = &self.qexed_ping_api {
                    let _ = UnReturnMessage::build(qexed_ping::message::TaskCommand::Close)
                        .post(&api_ping)
                        .await;
                }
                if let Some(api_ping) = &self.qexed_heartbeat_api {
                    let _ = UnReturnMessage::build(qexed_heartbeat::message::TaskCommand::Close)
                        .post(&api_ping)
                        .await;
                }
                if let Some(api_ping) = &self.qexed_packet_split_api {
                    let _ = ReturnMessage::build(qexed_packet_split::message::TaskMessage::Close)
                        .post(&api_ping)
                        .await;
                }
                if let Some(api_ping) = &self.qexed_chat_api {
                    let _ = UnReturnMessage::build(qexed_chat::message::TaskMessage::Close)
                        .post(&api_ping)
                        .await;
                }
                if let Some(api_ping) = &self.qexed_command_api {
                    let _ = UnReturnMessage::build(qexed_command::message::TaskCommand::Close)
                        .post(&api_ping)
                        .await;
                }
                if let Some(api_ping) = &self.qexed_player_list_api {
                    let _ = ReturnMessage::build(qexed_player_list::Message::PlayerLeft(self.uuid.clone()))
                        .post(&api_ping)
                        .await;
                }
                // 向父级发送关闭消息
                ReturnMessage::build(ManagerMessage::PlayerClose(self.uuid))
                    .get(manage_api)
                    .await?;
                if let Some(send) = data.get_return_send().await? {
                    let _ = send.send(data.data);
                }
                return Ok(true);
            }
        }
        Ok(false)
    }
}
// async fn config

fn encode_empty_chunk_data_1_21() -> Vec<u8> {
    let mut data = Vec::new();

    // 1.21.8 使用 24 个区块段落 (从 y=-64 到 y=319)
    for d in 0..24 {
        // 段落非空气方块数量为 0
        // 段落非空气方块数量为 0
        if d == 0 {
            data.extend_from_slice(&256i16.to_be_bytes());
        } else {
            data.extend_from_slice(&0i16.to_be_bytes());
        }       

        // 方块状态
        if d == 0 {
            // 第一个段落有基岩和空气两种方块
            let bits_per_block = 4; // 需要至少4位来表示0-15的索引
            data.push(bits_per_block as u8);

            // 调色板长度 - 使用 VarInt 编码
            data.extend(encode_var_int(16)); // 需要定义16个调色板条目

            // 定义所有可能的调色板条目（0-15）
            for i in 0..16 {
                if i == 1 {
                    // 索引1对应基岩
                    data.extend(encode_var_int(1));
                } else {
                    // 其他索引对应空气
                    data.extend(encode_var_int(0));
                }
            }
        } else {
            // 其他段落只有空气方块
            let bits_per_block = 1; // 只需要1位，因为只有空气
            data.push(bits_per_block as u8);

            // 调色板长度 - 使用 VarInt 编码
            data.extend(encode_var_int(1));

            // 空气方块的 ID
            data.extend(encode_var_int(0));
        }       

        // 计算需要多少个 long 来存储 4096 个方块
        let bits_per_block = if d == 0 { 4 } else { 1 };
        let blocks_per_long = 64 / bits_per_block;
        let num_longs = (4096 + blocks_per_long - 1) / blocks_per_long;     

        // 设置方块数据
        if d == 0 {
            // 第一个段落: 最底层是基岩 (索引1)，其余是空气 (索引0)
            for i in 0..num_longs {
                let mut long_value = 0i64;

                // 每个long包含多个方块
                for j in 0..blocks_per_long {
                    let block_index = i * blocks_per_long + j;

                    // 检查这个方块是否在最底层 (y=-64)
                    if block_index < 256 {
                        // 最底层方块是基岩 (调色板索引1)
                        long_value |= 1 << (j * bits_per_block);
                    }
                    // 其他方块保持为0 (空气，调色板索引0)
                }

                data.extend_from_slice(&long_value.to_be_bytes());
            }
        } else {
            // 其他段落: 所有方块都是空气 (调色板索引 0)
            for _ in 0..num_longs {
                data.extend_from_slice(&0i64.to_be_bytes());
            }
        }


        // 生物群系数据
        // 使用调色板模式，只有一个生物群系
        let bits_per_biome = 1; // 只需要 1 位，因为只有一种生物群系
        data.push(bits_per_biome as u8);

        // 生物群系调色板长度 - 使用 VarInt 编码
        data.extend(encode_var_int(1));

        // 平原生物群系的 ID
        data.extend(encode_var_int(1));

        // 计算需要多少个 long 来存储 64 个生物群系 (4x4x4)
        let biomes_per_long = 64 / bits_per_biome;
        let num_biome_longs = (64 + biomes_per_long - 1) / biomes_per_long;

        // 所有生物群系都是平原 (调色板索引 0)
        for _ in 0..num_biome_longs {
            data.extend_from_slice(&0i64.to_be_bytes());
        }
    }
    data
}

fn create_heightmaps() -> Vec<qexed_protocol::to_client::play::map_chunk::Heightmaps> {
    vec![
        qexed_protocol::to_client::play::map_chunk::Heightmaps {
            type_id: VarInt(0), // MOTION_BLOCKING
            // 高度图应该包含 256 个值（16x16），每个值是一个 VarLong
            // 对于空区块，所有高度都是世界底部（-64）
            data: vec![0; 37], // 这个大小可能需要调整
        },
        qexed_protocol::to_client::play::map_chunk::Heightmaps {
            type_id: VarInt(1), // WORLD_SURFACE
            data: vec![0; 37],  // 这个大小可能需要调整
        },
    ]
}
fn encode_var_int(value: i32) -> Vec<u8> {
    let mut value = value as u32;
    let mut buf = Vec::new();
    loop {
        if value & !0x7F == 0 {
            buf.push(value as u8);
            break;
        } else {
            buf.push((value as u8 & 0x7F) | 0x80);
            value >>= 7;
        }
    }
    buf
}
fn create_light_data_for_all_sections() -> qexed_protocol::to_client::play::map_chunk::Light {
    let total_sections = 24; // 从 y=-64 到 y=319

    // 1. 设置光照掩码 - 所有段落都需要更新光照
    let mut sky_light_mask = Bitset(vec![0; (total_sections + 63) / 64]);
    let mut block_light_mask = Bitset(vec![0; (total_sections + 63) / 64]);

    // 设置所有段落
    for i in 0..total_sections {
        let index = i / 64;
        let bit = i % 64;
        sky_light_mask.0[index] |= 1 << bit;
        block_light_mask.0[index] |= 1 << bit;
    }

    // 2. 空光照掩码设置为空（没有段落被标记为空）
    let empty_sky_light_mask = Bitset(vec![0; (total_sections + 63) / 64]);
    let empty_block_light_mask = Bitset(vec![0; (total_sections + 63) / 64]);

    // 3. 创建光照数据 - 为每个段落创建光照数据
    let mut sky_light_arrays = Vec::new();
    let mut block_light_arrays = Vec::new();

    for _ in 0..total_sections {
        let mut sky_light_data = vec![0u8; 2048];
        let mut block_light_data = vec![0u8; 2048];

        // 设置全部方块为最大方块光照 (15)
        for i in 0..2048 {
            block_light_data[i] = 0xFF; // 每个字节存储两个15值 (0xF = 15)
            // 同时设置天空光照为最大值
            sky_light_data[i] = 0xFF;
        }

        sky_light_arrays.push(sky_light_data);
        block_light_arrays.push(block_light_data);
    }

    // 4. 返回Light结构体
    qexed_protocol::to_client::play::map_chunk::Light {
        sky_light_mask,
        block_light_mask,
        empty_sky_light_mask,
        empty_block_light_mask,
        sky_light_arrays,
        block_light_arrays,
    }
}
// 创建一个简单的 /qexed 命令


// 创建文本组件的NBT标签
fn create_text_nbt(text: &str) -> qexed_nbt::Tag {
    let mut map = std::collections::HashMap::new();
    map.insert("text".to_string(), qexed_nbt::Tag::String(text.to_string().into()));
    qexed_nbt::Tag::Compound(std::sync::Arc::new(map))
}

// 构建成就显示信息
fn create_advancement_display() -> qexed_protocol::to_client::play::update_advancements::AdvancementDisplay {
    // 创建NBT标签表示文本组件
    // 标题：欢迎来到Qexed的世界
    let title_nbt = create_text_nbt("欢迎来到Qexed的世界");
    
    // 描述：获得泥土
    let description_nbt = create_text_nbt("获得泥土");
    
    qexed_protocol::to_client::play::update_advancements::AdvancementDisplay {
        title: title_nbt,
        description: description_nbt,
        icon: create_dirt_slot(),
        frame_type: VarInt(0),  // task类型
        flags: 0x01,  // 有背景纹理，不显示toast（因为根成就通常不会触发toast，除非完成）
        background_texture: Some("minecraft:gui/advancements/backgrounds/end".to_string()),
        x_coord: 0.0,  // 成就界面中的X坐标
        y_coord: 0.0,  // 成就界面中的Y坐标
    }
}

// 创建一个更完整的示例，包含多个成就
pub fn create_multiple_advancements_packet() -> qexed_protocol::to_client::play::update_advancements::UpdateAdvancements {
    // 第一个成就：根成就
    let root_advancement = qexed_protocol::to_client::play::update_advancements::AdvancementMapping {
        key: "qexed:story/root".to_string(),
        value: qexed_protocol::to_client::play::update_advancements::Advancement {
            parentid: None,
            display_data: Some(create_advancement_display()),
            nested_requirements: vec![
                vec![
                    vec!["craft_workbench".to_string()]  // 需要制作工作台
                ]
            ].into_iter().flatten().collect(),
            sends_telemetry_data: false,
        },
    };
    
    // 创建木头物品的Slot
    let wood_slot = qexed_protocol::types::Slot {
        item_count: VarInt(1),
        item_id: Some(VarInt(17)),  // 木头的物品ID
        number_of_components_to_add: Some(VarInt(0)),
        number_of_components_to_remove: Some(VarInt(0)),
        components_to_add: Some(Vec::new()),
        components_to_remove: Some(Vec::new()),
    };
    
    // 第二个成就：获得木头
    let get_wood_advancement = qexed_protocol::to_client::play::update_advancements::AdvancementMapping {
        key: "qexed:story/get_wood".to_string(),
        value: qexed_protocol::to_client::play::update_advancements::Advancement {
            parentid: Some("qexed:story/root".to_string()),  // 父成就是根成就
            display_data: Some(qexed_protocol::to_client::play::update_advancements::AdvancementDisplay {
                title: create_text_nbt("获得木头"),
                description: create_text_nbt("砍伐树木获得原木"),
                icon: wood_slot,
                frame_type: VarInt(0),  // task
                flags: 0x02,  // 显示toast
                background_texture: None,
                x_coord: 1.0,  // 在成就界面中的位置
                y_coord: 0.0,
            }),

            nested_requirements: vec![
                vec![
                    vec!["get_oak_log".to_string()]  // 需要制作工作台
                ]
            ].into_iter().flatten().collect(),
            sends_telemetry_data: false,
        },
    };
    
    qexed_protocol::to_client::play::update_advancements::UpdateAdvancements {
        reset_or_clear: true,
        advancement_mapping: vec![root_advancement, get_wood_advancement],
        identifiers: Vec::new(),
        progress_mapping: Vec::new(),  // 玩家还没有完成任何成就
        show_advancements: true,
    }
}
fn create_dirt_slot() -> qexed_protocol::types::Slot {
    qexed_protocol::types::Slot {
        item_count: VarInt(1),  // 数量为1
        item_id: Some(VarInt(3)),  // 泥土的物品ID，假设为3（需要确认实际ID）
        number_of_components_to_add: Some(VarInt(0)),
        number_of_components_to_remove: Some(VarInt(0)),
        components_to_add: Some(vec![]),
        components_to_remove: Some(vec![]),
    }
}


// TEST
pub struct Task<ManageMessageType> {
    api: MessageSender<ReturnMessage<TaskMessage>>,
    manage_api: MessageSender<ManageMessageType>,
    other: GameLogicActor,
    receiver: Option<UnboundedReceiver<ReturnMessage<TaskMessage>>>,
}
impl<ManageMessageType> Task<ManageMessageType>
where
    ManageMessageType: Send + 'static + std::fmt::Debug + Unpin,
    GameLogicActor: Send + 'static + std::fmt::Debug + Unpin + TaskEvent<ReturnMessage<TaskMessage>,ManageMessageType>, // 添加 Send
    
{
    pub fn new(
        manage_api: MessageSender<ManageMessageType>,
        data: GameLogicActor,
    ) -> (Self, MessageSender<ReturnMessage<TaskMessage>>) {
        let (w, r) = tokio::sync::mpsc::unbounded_channel();
        (
            Self {
                api: w.clone(),
                manage_api: manage_api,
                other: data,
                receiver: Some(r),
            },
            w,
        )
    }
    // 请注意:下面的所有权转移并不是失误,是刻意的设计
    pub async fn run(self) -> anyhow::Result<()> {
        tokio::spawn(self.listen());
        Ok(())
    }
    async fn listen(mut self) -> anyhow::Result<()> {
        let mut receiver = self
            .receiver
            .take()
            .ok_or_else(|| anyhow::anyhow!("接收管道不存在"))?;
        let api = self.api;
        let manage_api = self.manage_api;
        while let Some(data) = receiver.recv().await {
            // 这里我们后面修改来实现具体业务逻辑
            match self.other.event(&api, &manage_api, data).await {
                Ok(v)=>{
                    if v{
                        receiver.close();
                    }
                }
                Err(e)=>{
                    self.other.event(&api, &manage_api, ReturnMessage::build(TaskMessage::Close)).await?;
                    receiver.close();
                }
            }
        }
        Ok(())
    }
}
