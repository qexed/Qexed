use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use bytes::Bytes;
use qexed_nbt::Tag;
use qexed_packet::PacketCodec;
use qexed_player::Player;
use qexed_protocol::{
    to_client::play::system_chat::SystemChat,
    to_server::play::{keep_alive::KeepAlive, pong::Pong},
};
use qexed_task::{
    event::task::TaskEvent,
    message::{
        MessageSender, MessageType, return_message::ReturnMessage,
        unreturn_message::UnReturnMessage,
    },
};
use qexed_tcp_connect::PacketSend;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use uuid::Uuid;

use crate::message::{ManagerMessage, TaskMessage};

#[derive(Debug)]
pub struct ChatActor {
    uuid: Uuid,
    name: String,
    packet_write: Option<UnboundedSender<Bytes>>,
    config: qexed_config::app::qexed_chat::ChatConfig,
}
impl ChatActor {
    pub fn new(uuid: Uuid, config: qexed_config::app::qexed_chat::ChatConfig) -> Self {
        Self {
            uuid,
            name: "".to_string(),
            packet_write: None,
            config: config,
        }
    }
}
#[async_trait]
impl TaskEvent<UnReturnMessage<TaskMessage>, ReturnMessage<ManagerMessage>> for ChatActor {
    async fn event(
        &mut self,
        _api: &MessageSender<UnReturnMessage<TaskMessage>>,
        manage_api: &MessageSender<ReturnMessage<ManagerMessage>>,
        mut data: UnReturnMessage<TaskMessage>,
    ) -> anyhow::Result<bool> {
        match data.data {
            TaskMessage::Start(name, mut unbounded_sender) => {
                // 玩家进入了服务器
                self.name = name;
                self.packet_write = unbounded_sender.take();
                let _packet_write = match self.packet_write.clone() {
                    Some(p) => p,
                    None => {
                        return Ok(false);
                    }
                };

                return Ok(false);
            }
            TaskMessage::ChatEvent(chat_message) => {
                log::info!("<{}> {}", &self.name, chat_message.message);
                ReturnMessage::build(ManagerMessage::BroadCastEvent(
                    self.uuid,
                    build_chat(&self.name, chat_message.message),
                ))
                .get(manage_api)
                .await?;
                return Ok(false);
            }
            TaskMessage::SendMessage(system_chat) => {
                if let Some(packet_write) = &self.packet_write {
                    packet_write.send(PacketSend::build_send_packet(system_chat).await?)?;
                    // Test 给泥土
                    packet_write.send(PacketSend::build_send_packet(build_item()?).await?)?;
                    // 初始化配方
                    // packet_write.send(
                    //     PacketSend::build_send_packet(build_dirt_from_4_stones_recipe())
                    //     .await?,
                    // )?;
                    // packet_write.send(
                    //     PacketSend::build_send_packet(build_dirt_from_4_stones_recipe())
                    //     .await?,
                    // )?;
                }
                return Ok(false);
            }
            TaskMessage::SystemEvent(system_event) => match system_event {
                crate::message::SystemEvent::PlayerJoin => {
                    log::info!("{} 进入了服务器", &self.name);
                    ReturnMessage::build(ManagerMessage::BroadCastEvent(
                        self.uuid,
                        build_system_message(format!("§e{}§e 进入了服务器", &self.name)),
                    ))
                    .get(manage_api)
                    .await?;
                    return Ok(false);
                }
                crate::message::SystemEvent::PlayerLevel => {
                    log::info!("{} 退出了服务器", &self.name);
                    ReturnMessage::build(ManagerMessage::BroadCastEvent(
                        self.uuid,
                        build_system_message(format!("§e{}§e 退出了服务器", &self.name)),
                    ))
                    .get(manage_api)
                    .await?;
                    return Ok(false);
                }
            },
            TaskMessage::Close => {
                // 向父级发送关闭消息
                ReturnMessage::build(ManagerMessage::PlayerClose(self.uuid))
                    .get(manage_api)
                    .await?;
                return Ok(true);
            }
        }
    }
}
// async fn config
fn build_chat(player: &str, message: String) -> SystemChat {
    // 1. 创建文本组件的 Compound
    let mut chat_component = HashMap::new();
    // Minecraft 文本组件的基础格式：{"text": "实际内容"}
    chat_component.insert(
        "text".to_string(),
        Tag::String(format!("<{}> {}", player.clone(), message).into()), // 使用 `into()` 转为 Arc<str>
    );

    // 2. 可选：添加样式（例如颜色）
    // chat_component.insert("color".to_string(), Tag::String("red".into()));

    // 3. 将 HashMap 包装为 Tag::Compound
    let content_nbt = Tag::Compound(Arc::new(chat_component));

    // 4. 构建 SystemChat（overlay = false 表示显示在普通聊天框）
    SystemChat {
        content: content_nbt,
        overlay: false,
    }
}
fn build_system_message(message: String) -> SystemChat {
    // 1. 创建文本组件的 Compound
    let mut chat_component = HashMap::new();
    // Minecraft 文本组件的基础格式：{"text": "实际内容"}
    chat_component.insert(
        "text".to_string(),
        Tag::String(message.into()), // 使用 `into()` 转为 Arc<str>
    );

    // 2. 可选：添加样式（例如颜色）
    // chat_component.insert("color".to_string(), Tag::String("red".into()));

    // 3. 将 HashMap 包装为 Tag::Compound
    let content_nbt = Tag::Compound(Arc::new(chat_component));

    // 4. 构建 SystemChat（overlay = false 表示显示在普通聊天框）
    SystemChat {
        content: content_nbt,
        overlay: false,
    }
}
pub fn build_dirt_from_4_stones_recipe()
-> qexed_protocol::to_client::play::recipe_book_add::RecipeBookAdd {
    // 假设的物品ID
    let stone_id = qexed_packet::net_types::VarInt(qexed_block::BlockId::Stone as i32); // 石头
    let dirt_id = qexed_packet::net_types::VarInt(qexed_block::BlockId::Dirt as i32); // 泥土

    // 创建无序合成 (2x2背包合成)
    let crafting_shapeless = qexed_protocol::types::minecraft::CraftingShapeless {
        ingredients: vec![qexed_protocol::types::SlotDisplay::Item(
            qexed_protocol::types::slot_display_types::minecraft::Item {
                item_type: stone_id.clone(),
            },
        )],

        // 合成结果：1个泥土
        result: qexed_protocol::types::SlotDisplay::Item(
            qexed_protocol::types::slot_display_types::minecraft::Item {
                item_type: dirt_id.clone(),
            },
        ),

        crafting_station: qexed_protocol::types::SlotDisplay::Empty,
    };

    qexed_protocol::to_client::play::recipe_book_add::RecipeBookAdd {
        entries: vec![qexed_protocol::to_client::play::recipe_book_add::Recipes {
            // 配方ID
            recipe: qexed_packet::net_types::VarInt(1002), // 注意：使用不同的ID

            // 显示信息 - 使用无序合成
            display: qexed_protocol::types::RecipeDisplay::MinecraftCraftingShapeless(
                crafting_shapeless,
            ),

            // 配方分组
            group: qexed_packet::net_types::VarInt(0),

            // 配方类别 - 无序合成
            category: qexed_packet::net_types::VarInt(0), // 合成类别

            ingredients: Some(vec![
                qexed_protocol::types::IDSet {
                    r#type: qexed_packet::net_types::VarInt(2), // 物品类型
                    tag_name: None,
                    ids: Some(vec![stone_id.clone()]), // 一个石头
                },
                qexed_protocol::types::IDSet {
                    r#type: qexed_packet::net_types::VarInt(2),
                    tag_name: None,
                    ids: Some(vec![stone_id.clone()]),
                },
                qexed_protocol::types::IDSet {
                    r#type: qexed_packet::net_types::VarInt(2),
                    tag_name: None,
                    ids: Some(vec![stone_id.clone()]),
                },
                qexed_protocol::types::IDSet {
                    r#type: qexed_packet::net_types::VarInt(2),
                    tag_name: None,
                    ids: Some(vec![stone_id.clone()]),
                },
            ]),

            // 标志位
            flags: 0x2, // 已解锁
        }],
        replace: false,
    }
}

pub fn build_item()
-> anyhow::Result<qexed_protocol::to_client::play::container_set_content::ContainerSetContent> {
    let total_slots = 46;
    let mut slot_data = vec![qexed_protocol::types::Slot::default(); total_slots];
    let stone_id = qexed_packet::net_types::VarInt(qexed_item::ItemId::Stone as i32); // 石头
    let dirt_id = qexed_packet::net_types::VarInt(qexed_item::ItemId::Dirt as i32); // 泥土
    slot_data[0] = create_slot(&stone_id, 16)?; // 16个石头

    // // 槽位1：泥土
    //slot_data[14] = create_slot(&dirt_id, 10)?; // 10个泥土

    // 在背包（9-35）的前4个格子放入石头
    for i in 9..13 {
        slot_data[i] = create_slot(&stone_id, 1)?; // 每个格子1个石头
    }
    Ok(
        qexed_protocol::to_client::play::container_set_content::ContainerSetContent {
            window_id: qexed_packet::net_types::VarInt(0),
            state_id: qexed_packet::net_types::VarInt(0),
            slot_data: slot_data,
            carried_item: qexed_protocol::types::Slot {
                item_count: qexed_packet::net_types::VarInt(0), // 数量为1
                item_id: None,
                number_of_components_to_add: None,
                number_of_components_to_remove: None,
                components_to_add: None,
                components_to_remove: None,
            },
        },
    )
}
/// 创建一个物品槽位
fn create_slot(
    item_id: &qexed_packet::net_types::VarInt,
    count: i8,
) -> anyhow::Result<qexed_protocol::types::Slot> {
    if count < 0 || count > 64 {
        return Err(anyhow::anyhow!("物品数量必须在0-64之间: {}", count));
    }

    Ok(qexed_protocol::types::Slot {
        item_count: qexed_packet::net_types::VarInt(count as i32), // 数量为1
        item_id: Some(item_id.clone()), // 泥土的物品ID，假设为3（需要确认实际ID）
        number_of_components_to_add: Some(qexed_packet::net_types::VarInt(0)),
        number_of_components_to_remove: Some(qexed_packet::net_types::VarInt(0)),
        components_to_add: Some(vec![]),
        components_to_remove: Some(vec![]),
    })
}
