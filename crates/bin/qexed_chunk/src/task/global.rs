use async_trait::async_trait;
use dashmap::DashMap;
use qexed_task::{
    event::task_manage::TaskManageEvent,
    message::{MessageSender, MessageType, unreturn_message::UnReturnMessage},
};
use tokio::sync::oneshot;
use uuid::Uuid;
use std::path::PathBuf;

use crate::{
    event::global::GlobalManage,
    message::{
        global::GlobalCommand, 
        region::RegionCommandResult,
        world::WorldCommand
    },
};

#[async_trait]
impl TaskManageEvent<uuid::Uuid, UnReturnMessage<GlobalCommand>, UnReturnMessage<WorldCommand>>
    for GlobalManage
{
    async fn event(
        &mut self,
        api: &MessageSender<UnReturnMessage<GlobalCommand>>,
        task_map: &DashMap<uuid::Uuid, MessageSender<UnReturnMessage<WorldCommand>>>,
        data: UnReturnMessage<GlobalCommand>,
    ) -> anyhow::Result<bool> {
        match data.data {
            GlobalCommand::Init => {
                // 初始化全局管理器
                // 可以在这里加载配置、初始化数据库等
                println!("GlobalManager initialized");
            }
            GlobalCommand::LoadWorld { uuid, result } => {
                // 加载世界
                if let Some(world_sender) = task_map.get(&uuid) {
                    // 世界已加载
                    let _ = result.send(true);
                } else {
                    if let Some(world_info) = self.config.world.get(&uuid).cloned(){
                        
                        let (world_task, world_sender) = qexed_task::task::task_manage::TaskManage::new(
                            crate::event::world::WorldManage::new(world_info,self.worlds_root.clone(),uuid.clone(),api.clone())
                        );
                        task_map.insert(uuid, world_sender.clone());
                        let _ = result.send(true);
                        return Ok(false);
                    };
                    let _ = result.send(false);                    
                    
                }
            }
            GlobalCommand::UnloadWorld { uuid, result } => {
                // 卸载世界
                if let Some((_, world_sender)) = task_map.remove(&uuid) {
                    // 发送世界关闭命令
                    let (tx, rx) = oneshot::channel();
                    let _ = world_sender.send(UnReturnMessage::build(
                        WorldCommand::WorldCloseCommand { result: tx }
                    ));
                    
                    // 异步等待世界关闭
                    tokio::spawn(async move {
                        match rx.await {
                            Ok(_) => {
                                let _ = result.send(true);
                            }
                            Err(_) => {
                                let _ = result.send(false);
                            }
                        }
                    });
                } else {
                    let _ = result.send(false);
                }
            }
            GlobalCommand::WorldCloseEvent { uuid } => {
                // 从任务映射中移除已关闭的世界
                task_map.remove(&uuid);
            }
            // 玩家相关命令暂时不需要实现
            GlobalCommand::PlayerJoin { player_uuid, world, pos, view_distance } => {
                // 暂不实现
                println!("PlayerJoin: player={}, world={}", player_uuid, world);
            }
            GlobalCommand::PlayerLeave { player_uuid, world, pos, view_distance } => {
                // 暂不实现
                println!("PlayerLeave: player={}, world={}", player_uuid, world);
            }
            GlobalCommand::PlayerChangeWorld { player_uuid, from_world, to_world, pos } => {
                // 暂不实现
                println!("PlayerChangeWorld: player={}, from={}, to={}", player_uuid, from_world, to_world);
            }
            // 跨世界操作 - 转发到对应世界
            GlobalCommand::GetOtherWorldRegionApi { pos, world, result } => {
                if let Some(world_sender) = task_map.get(&world) {
                    // 转发到目标世界
                    let _ = world_sender.send(UnReturnMessage::build(
                        WorldCommand::GetRegionApi { pos, result }
                    ));
                } else {
                    // 世界未加载
                    let _ = result.send(RegionCommandResult::GetRegionApiResult {
                        success: false,
                        api: None,
                    });
                }
            }
            GlobalCommand::CreateOtherWorldRegion { pos, world, result } => {
                if let Some(world_sender) = task_map.get(&world) {
                    // 转发到目标世界
                    let _ = world_sender.send(UnReturnMessage::build(
                        WorldCommand::CreateRegion { pos, result }
                    ));
                } else {
                    // 世界未加载
                    let _ = result.send(RegionCommandResult::CreateRegionResult {
                        success: false,
                        api: None,
                    });
                }
            }
            GlobalCommand::GetOtherWorldChunkApi { pos, world, result } => {
                if let Some(world_sender) = task_map.get(&world) {
                    // 转发到目标世界
                    let _ = world_sender.send(UnReturnMessage::build(
                        WorldCommand::GetChunkApi { pos, result }
                    ));
                } else {
                    // 世界未加载
                    let _ = result.send(RegionCommandResult::GetChunkApiResult {
                        success: false,
                        api: None,
                    });
                }
            }
            GlobalCommand::SendOtherWorldChunkCommand { pos, world, event } => {
                if let Some(world_sender) = task_map.get(&world) {
                    // 转发到目标世界
                    let _ = world_sender.send(UnReturnMessage::build(
                        WorldCommand::SendChunkCommand { pos, event }
                    ));
                }
                // 如果世界未加载，命令被丢弃
            }
            GlobalCommand::SendOtherWorldChunkNeedReturnCommand { pos, world, event, result } => {
                if let Some(world_sender) = task_map.get(&world) {
                    // 转发到目标世界
                    let _ = world_sender.send(UnReturnMessage::build(
                        WorldCommand::SendChunkNeedReturnCommand { pos, event, result }
                    ));
                } else {
                    // 世界未加载，直接返回失败
                    let _ = result.send(false);
                }
            }
            GlobalCommand::CreateOtherWorldChunk { pos, world, result } => {
                if let Some(world_sender) = task_map.get(&world) {
                    // 转发到目标世界
                    let _ = world_sender.send(UnReturnMessage::build(
                        WorldCommand::CreateChunk { pos, result }
                    ));
                } else {
                    // 世界未加载
                    let _ = result.send(RegionCommandResult::CreateChunkResult {
                        success: false,
                        api: None,
                    });
                }
            }
            GlobalCommand::GetWorldApi { world, result } => {
                if let Some(world_sender) = task_map.get(&world) {
                    let _ = result.send(Some(world_sender.clone()));
                } else {
                    let _ = result.send(None);
                }
            }
            GlobalCommand::GetWorldsStatus { result } => {
                // 收集所有世界的状态
                let mut status = Vec::new();
                
                for entry in task_map.iter() {
                    let world_uuid = *entry.key();
                    // 这里可以添加更多状态信息
                    status.push((world_uuid, "loaded".to_string()));
                }
                
                let _ = result.send(status);
            }
            GlobalCommand::CommandSeed(cmd)=>{
                for i in &self.config.world{
                    cmd.send_chat_message(&format!("§e[世界§7:§3{}§e]§r 种子:[§2{}§r]",i.1.name,i.1.seed)).await?;
                }
            }
        }
        Ok(false)
    }
}