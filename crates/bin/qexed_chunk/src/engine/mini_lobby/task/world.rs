use async_trait::async_trait;
use dashmap::DashMap;
use qexed_task::{
    event::task_manage::TaskManageEvent,
    message::{MessageSender, MessageType, unreturn_message::UnReturnMessage},
};
use qexed_tcp_connect::PacketSend;
use tokio::sync::oneshot;

use crate::{
    engine::mini_lobby::event::world::WorldManage,
    message::{
        region::{RegionCommand, RegionCommandResult},
        world::WorldCommand,
    },
};

#[async_trait]
impl TaskManageEvent<[i64; 2], UnReturnMessage<WorldCommand>, UnReturnMessage<RegionCommand>>
    for WorldManage
{
    async fn event(
        &mut self,
        api: &MessageSender<UnReturnMessage<WorldCommand>>,
        task_map: &DashMap<[i64; 2], MessageSender<UnReturnMessage<RegionCommand>>>,
        data: UnReturnMessage<WorldCommand>,
    ) -> anyhow::Result<bool> {
        match data.data {
            WorldCommand::Init=>{
                self.init(api,task_map).await?;
            }
            WorldCommand::PlayerJoin { pos, packet_send, uuid }=>{
                let pos = self.config.join_pos;// 无视任务端传递的存档的坐标
                // 所有区块均已加载，直接发送即可、
                let join_pos_chunk = self.player_pos_to_chunk_pos([pos[0] as i32,pos[1] as i32,pos[2] as i32]);
                // 构建 SetChunkCacheCenter 数据包
                packet_send.send(PacketSend::build_send_packet(qexed_protocol::to_client::play::update_view_position::UpdateViewPosition{
                    chunk_x:qexed_packet::net_types::VarInt(join_pos_chunk[0]),
                    chunk_z:qexed_packet::net_types::VarInt(join_pos_chunk[1]),
                }).await?)?;
                for i in task_map{
                    let _ = i.send(qexed_task::message::unreturn_message::UnReturnMessage { data: RegionCommand::PlayerJoin { pos, packet_send:packet_send.clone() ,uuid} });
                }
            }
            WorldCommand::GetRegionApi { pos, result } => {
                if let Some(region) = task_map.get(&pos) {
                    let _ = result.send(RegionCommandResult::GetRegionApiResult {
                        success: true,
                        api: Some(region.clone()),
                    });
                } else {
                    let _ = result.send(RegionCommandResult::GetRegionApiResult {
                        success: false,
                        api: None,
                    });
                }
            }
            WorldCommand::CreateRegion { pos, result } => {
                if let Some(region) = task_map.get(&pos) {
                    let _ = result.send(RegionCommandResult::CreateRegionResult {
                        success: true,
                        api: Some(region.clone()),
                    });
                } else {
                    // 暂时不创建区域，因为RegionManage的trait实现有问题
                    // 先返回失败，等修复trait实现后再实现创建逻辑
                    let _ = result.send(RegionCommandResult::CreateRegionResult {
                        success: false,
                        api: None,
                    });
                }
            }
            WorldCommand::GetOtherWorldRegionApi { pos, world, result } => {}
            WorldCommand::CreateOtherWorldRegion { pos, world, result } => {}
            WorldCommand::GetChunkApi { pos, result } => {
                // 计算所属区域位置
                let region_pos = self.calc_region_pos_for_world(pos);

                if let Some(region) = task_map.get(&region_pos) {
                    // 转发到区域
                    let _ = region.send(UnReturnMessage::build(RegionCommand::GetChunkApi {
                        pos,
                        result,
                    }));
                } else {
                    let _ = result.send(RegionCommandResult::GetChunkApiResult {
                        success: false,
                        api: None,
                    });
                }
            }
            WorldCommand::SendChunkCommand { pos, event } => {
                // 计算所属区域位置
                let region_pos = self.calc_region_pos_for_world(pos);

                if let Some(region) = task_map.get(&region_pos) {
                    // 转发到区域
                    let _ = region.send(UnReturnMessage::build(RegionCommand::SendChunkCommand {
                        pos,
                        event,
                    }));
                }
                // 如果没有区域，命令被丢弃
            }
            WorldCommand::SendChunkNeedReturnCommand { pos, event, result } => {
                // 计算所属区域位置
                let region_pos = self.calc_region_pos_for_world(pos);

                if let Some(region) = task_map.get(&region_pos) {
                    // 转发到区域
                    let _ = region.send(UnReturnMessage::build(
                        RegionCommand::SendChunkNeedReturnCommand { pos, event, result },
                    ));
                } else {
                    let _ = result.send(false);
                }
            }
            WorldCommand::CreateChunk { pos, result } => {
                // 计算所属区域位置
                let region_pos = self.calc_region_pos_for_world(pos);

                if let Some(region) = task_map.get(&region_pos) {
                    // 转发到区域
                    let _ = region.send(UnReturnMessage::build(RegionCommand::CreateChunk {
                        pos,
                        result,
                    }));
                } else {
                    let _ = result.send(RegionCommandResult::CreateChunkResult {
                        success: false,
                        api: None,
                    });
                }
            }
            WorldCommand::GetOtherWorldChunkApi { pos, world, result } => {}
            WorldCommand::SendOtherWorldChunkCommand { pos, world, event } => {}
            WorldCommand::SendOtherWorldChunkNeedReturnCommand {
                pos,
                world,
                event,
                result,
            } => {}
            WorldCommand::CreateOtherWorldChunk { pos, world, result } => {}
            WorldCommand::RegionCloseEvent { pos } => {
                // 只从世界的任务映射中移除区域
                // 区域会自行通知相邻区域，世界不需要处理
                task_map.remove(&pos);
            }
            WorldCommand::WorldCloseCommand { result } => {
                // 关闭所有区域
                let mut close_tasks: Vec<oneshot::Receiver<_>> = Vec::new();

                for entry in task_map.iter() {
                    let pos = *entry.key();
                    let region_sender: tokio::sync::mpsc::UnboundedSender<
                        UnReturnMessage<RegionCommand>,
                    > = entry.value().clone();

                    let (tx, rx) = oneshot::channel();

                    // 发送关闭命令
                    if region_sender
                        .send(UnReturnMessage::build(RegionCommand::RegionCloseCommand {
                            result: tx,
                        }))
                        .is_ok()
                    {
                        close_tasks.push(rx);
                    }
                }

                // 等待所有区域关闭
                for rx in close_tasks {
                    let _ = rx.await;
                }

                // 清空task_map
                task_map.clear();
                result.send(());
                return Ok(true); // 世界关闭完成
            }
            WorldCommand::CommandSeed(command_data) => {}
        }

        Ok(false)
    }
}
