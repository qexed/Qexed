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
    qexed_title_api:Option<UnboundedSender<UnReturnMessage<qexed_title::message::TaskMessage>>>,
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
            qexed_title_api:None,
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
                // title
                if let ManagerMessage::GetTitle(Some(
                    qexed_title::message::ManagerMessage::NewPlayerConnect(
                        _uuid,
                        is_true,
                        err,
                        chat_api,
                    ),
                )) = ReturnMessage::build(ManagerMessage::GetTitle(Some(
                    qexed_title::message::ManagerMessage::NewPlayerConnect(
                        self.uuid.clone(),
                        false,
                        None,
                        None,
                    ),
                )))
                .get(&manage_api)
                .await?
                {
                    self.qexed_title_api = chat_api;
                }
                let title_api = match &self.qexed_title_api {
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
                UnReturnMessage::build(qexed_title::message::TaskMessage::Start(player.username.clone(),Some(packet_write.clone())))
                    .post(&title_api)
                    .await?;
                ReturnMessage::build(qexed_player_list::Message::PlayerJoin(player.uuid.clone(),player.username.clone())).get(&player_list_api).await?;
                // 区块初始化:
                packet_write.send(
                    PacketSend::build_send_packet(qexed_protocol::to_client::play::game_state_change::GameStateChange{reason:13,game_mode:0.0}).await?)?;
                // 发送玩家附近区块
                ReturnMessage::build(ManagerMessage::GetWorld(Some(qexed_chunk::message::world::WorldCommand::PlayerJoin {
                    uuid: player.uuid.clone(),
                    pos: [0,0,0],// 后续玩家模块完善后传递坐标
                    packet_send: packet_write.clone(),
                }))).get(&manage_api).await?;
                // let radius = { 12 as i32};
                // let mut chunks_pos: Vec<(i32, i32)> = vec![];
                // for x in -radius..=radius {
                //     for z in -radius..=radius {
                //         chunks_pos.push((x, z));
                //         // 创建空区块
                //         let p_q = qexed_protocol::to_client::play::map_chunk::MapChunk {
                //             chunk_x: x,
                //             chunk_z: z,
                //             data: qexed_protocol::to_client::play::map_chunk::Chunk {
                //                 // 高度图 - 使用修复后的高度图
                //                 heightmaps: create_heightmaps(),
                //                 // 空的区块数据 - 使用修复后的编码函数
                //                 data: encode_empty_chunk_data_1_21(),
                //                 // 无方块实体
                //                 block_entities: vec![],
                //             },
                //             light: create_light_data_for_all_sections(),
                //         };
                //         packet_write.send(PacketSend::build_send_packet(p_q).await?)?;
                //     }
                // }
                UnReturnMessage::build(qexed_heartbeat::message::TaskCommand::Start)
                    .post(&heartbeat_api)
                    .await?;
                // // Test:Set Title
                // packet_write.send(PacketSend::build_send_packet(qexed_protocol::to_client::play::set_title_text::SetTitleText{
                //     text:create_text_nbt("测试title"),
                // }).await?)?;
                
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
                    let _ = UnReturnMessage::build(qexed_chat::message::TaskMessage::SystemEvent(qexed_chat::message::SystemEvent::PlayerJoin))
                        .post(&chat_api)
                        .await;                
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
                    let _ = UnReturnMessage::build(qexed_chat::message::TaskMessage::SystemEvent(qexed_chat::message::SystemEvent::PlayerLevel))
                        .post(&api_ping)
                        .await;
                    let _ = UnReturnMessage::build(qexed_chat::message::TaskMessage::Close)
                        .post(&api_ping)
                        .await;
                }
                if let Some(api_ping) = &self.qexed_title_api {
                    let _ = UnReturnMessage::build(qexed_title::message::TaskMessage::Close)
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
